//! Development route driver: one new game, ordinary movement, ordinary
//! dialogue.
//!
//! The route drives the shipped game's own frame — `Session::frame` — and hands
//! it pads, the way the Godot shell does: walking presses a direction, talking
//! presses Speak, and whatever dialogue opens is pressed through with Speak.
//! This remains traversal evidence, not a Godot presentation or full-campaign
//! acceptance test.
use psiv_core::battle::Side;
use psiv_core::{Cell, Direction};
use psiv_runtime::{
    Button, CampAbilityKind, CampPage, CampView, CommandMenuView, DialogueSignal, MenuView,
    NpcDialogueOpen, Pad, Routed, Runtime, RuntimeEvent, SaveStore, SceneDialogueOpen, Session,
};
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

/// Frames between the route's dialogue presses: a fresh press every four
/// frames, with the three released frames in between, which is the shape the
/// oracle tapes use (a held button never advances a finished page).
const PRESS_PERIOD: u64 = 4;

/// The technique the route's policy fights with when it heals: RES, the first
/// cure the Academy party knows.
const HEAL_TECHNIQUE: u8 = 24;

pub struct Walk {
    pub session: Session,
    pub ticks: u64,
    pub battles: usize,
    /// Whether the battle policy spends the first eligible actor's turn on
    /// [`HEAL_TECHNIQUE`] when somebody is hurt.
    pub heal_in_battle: bool,
    /// The last battle view the session produced: what the policy navigates.
    battle_view: Option<psiv_runtime::BattleView>,
    /// Set while the policy's previous press is still held. A held button is
    /// not a second press, so the policy releases before it presses again.
    held_press: bool,
    /// The actor that healed this round, so one healer answers per round.
    healer: Option<u8>,
    /// The actor of the previous command window, for the round boundary.
    previous_actor: Option<u8>,
}

impl Walk {
    /// A walk over `session`, with the battle policy's healing turned on or
    /// off. The policy's own state is the harness's; every press it makes goes
    /// through the session's battle menu.
    pub fn new(session: Session, heal_in_battle: bool) -> Walk {
        Walk {
            session,
            ticks: 0,
            battles: 0,
            heal_in_battle,
            battle_view: None,
            held_press: false,
            healer: None,
            previous_actor: None,
        }
    }

    /// The runtime, for the route's own assertions and for the modes this
    /// harness asserts on; every mode it plays goes through the session.
    pub fn runtime(&self) -> &Runtime {
        self.session.runtime()
    }

    /// Writes the walk's own state through the session's store, pointed at
    /// `directory`: the session's own save path, not a second one.
    ///
    /// # Errors
    ///
    /// Whatever the store's write rejects.
    pub fn save_slot(
        &mut self,
        directory: &Path,
        slot: usize,
    ) -> Result<PathBuf, psiv_runtime::SessionSaveError> {
        self.session.set_save_store(SaveStore::new(directory));
        self.session.save_slot(slot)
    }

    /// One frame, in the shipped shell's order: the session's frame with this
    /// frame's pad, the events a shell would route, then the window's own half.
    pub fn tick(&mut self, pad: Pad) {
        self.ticks += 1;
        assert!(self.ticks < 100_000, "route exceeded its tick budget");
        // A battle is the session's now. While one owns the frame the route's
        // battle policy is the only thing pressing buttons, exactly as a player
        // would: the caller's pad describes the field, not the fight.
        let pad = if self.session.battle_active() {
            self.battle_pad()
        } else {
            pad
        };
        let pad = self.with_dialogue_press(pad);
        let frame = self.session.frame(pad);
        self.battle_view = frame.battle.and_then(|battle| battle.view);
        self.present(frame.signals);
        if let Some(start) = frame.scene_started {
            println!(
                "{} dialogue fires {:#x} (scene started: {})",
                self.ticks, start.event, start.started
            );
        }
        for routed in &frame.routed {
            match routed {
                Routed::SceneDialogue {
                    entry,
                    outcome: SceneDialogueOpen::UnknownTree,
                } => panic!("scene dialogue entry {entry:#04x}: tree is absent from the pack"),
                Routed::SceneDialogue { entry, .. } => {
                    println!("{} scene dialogue entry {entry:#04x}", self.ticks);
                }
                Routed::SceneChoice => println!("{} scene awaits a choice", self.ticks),
                Routed::Talk {
                    npc_index, outcome, ..
                } => match outcome {
                    NpcDialogueOpen::Opened => {}
                    NpcDialogueOpen::NoBinding => {
                        println!("{} NPC {npc_index} has no dialogue binding", self.ticks)
                    }
                    NpcDialogueOpen::Nothing => {
                        println!("{} NPC {npc_index} has nothing to say", self.ticks);
                    }
                },
                other => println!("{} {other:?}", self.ticks),
            }
        }
        for event in frame.events {
            match event {
                RuntimeEvent::SceneFaulted { .. }
                | RuntimeEvent::SceneMissing { .. }
                | RuntimeEvent::SceneBattleFailed { .. }
                | RuntimeEvent::MapRefreshFailed { .. }
                | RuntimeEvent::UnpackedTarget { .. }
                | RuntimeEvent::WarpUnmapped { .. } => panic!("{event:?}"),
                RuntimeEvent::EncounterRolled { formation } => {
                    self.battles += 1;
                    println!("BATTLE {}: encounter {formation:#05x}", self.battles);
                }
                RuntimeEvent::SceneBattleStarted { index, .. } => {
                    self.battles += 1;
                    println!("BATTLE {}: event battle {index}", self.battles);
                }
                RuntimeEvent::MapChanged { .. }
                | RuntimeEvent::SceneStarted { .. }
                | RuntimeEvent::SceneEnded
                | RuntimeEvent::SceneStartedFromInteraction { .. } => {
                    println!("{} {event:?}", self.ticks);
                }
                _ => {}
            }
        }
        self.present(frame.window_signals);
    }

    /// One frame of the battle policy's pad: the buttons a player would press
    /// in the session's own battle menu.
    fn battle_pad(&mut self) -> Pad {
        // Release the last press first: holding a button is not a second
        // press, and every accept and every cursor step is an edge.
        if std::mem::take(&mut self.held_press) {
            return Pad::NEUTRAL;
        }
        let Some(button) = self.battle_button() else {
            return Pad::NEUTRAL;
        };
        self.held_press = true;
        Pad::new(button)
    }

    /// The button this frame's menu state asks for.
    fn battle_button(&mut self) -> Option<Button> {
        let view = self.battle_view.clone()?;
        if !view.ready {
            // A beat owns the frame. The victory and level-up pages take any
            // face button (`Battle_VictoryMessage`, `ps4.asm:4706`).
            if view.finishing || view.current.is_some() {
                return Some(Button::Speak);
            }
            return None;
        }
        match view.menu? {
            // The main options: row 0 is COMD for a party and ATTAC for a
            // mounted surface; both lead to an ordinary attack round.
            MenuView::Top { cursor } => match cursor {
                0 => Some(Button::Speak),
                _ => Some(Button::Up),
            },
            // The mounted surface's own window: the first slot with uses left.
            MenuView::VehicleSkills { cursor, slots } => {
                let wanted = slots.iter().position(|slot| slot.current > 0)?;
                move_cursor(cursor, wanted)
            }
            MenuView::Commands(menu) => self.command_button(&menu),
        }
    }

    /// The button the per-character command window asks for.
    fn command_button(&mut self, menu: &CommandMenuView) -> Option<Button> {
        let actor = menu.actor?;
        if self.previous_actor != Some(actor) && (actor == 1 || self.previous_actor.is_none()) {
            // The actors answer from the first slot again: a new round.
            self.healer = None;
        }
        self.previous_actor = Some(actor);
        let healing = self.wants_to_heal(menu, actor);
        match menu.page {
            psiv_runtime::MenuPage::Actions => {
                let wanted = usize::from(healing);
                move_cursor(menu.cursor, wanted)
            }
            psiv_runtime::MenuPage::Techniques => {
                let wanted = menu
                    .techniques
                    .iter()
                    .position(|technique| technique.id == HEAL_TECHNIQUE && technique.available)?;
                move_cursor(menu.cursor, wanted)
            }
            psiv_runtime::MenuPage::Targets(_) => {
                let wanted = if healing {
                    let injured = self
                        .session
                        .runtime()
                        .battle_roster()?
                        .living(Side::Party)
                        .filter(|fighter| fighter.stats.curr_hp * 3 < fighter.stats.max_hp)
                        .min_by_key(|fighter| fighter.stats.curr_hp)?;
                    menu.targets.iter().position(|id| *id == injured.id.get())?
                } else {
                    // The strongest enemy, the target the auto-battle policy
                    // always took.
                    let enemy = self
                        .session
                        .runtime()
                        .battle_roster()?
                        .living(Side::Enemy)
                        .max_by_key(|fighter| fighter.stats.curr_hp)?;
                    menu.targets.iter().position(|id| *id == enemy.id.get())?
                };
                move_cursor(menu.cursor, wanted)
            }
            // The policy fights with ATTACK and one cure; anything else is a
            // page it backs out of.
            _ => Some(Button::Cancel),
        }
    }

    /// Whether this actor should spend the round healing, and whether it is
    /// the one healer of the round.
    fn wants_to_heal(&mut self, menu: &CommandMenuView, actor: u8) -> bool {
        if !self.heal_in_battle || self.healer.is_some() {
            return false;
        }
        let Some(roster) = self.session.runtime().battle_roster() else {
            return false;
        };
        let hurt = roster
            .living(Side::Party)
            .any(|fighter| fighter.stats.curr_hp * 3 < fighter.stats.max_hp);
        let knows = menu
            .techniques
            .iter()
            .any(|technique| technique.id == HEAL_TECHNIQUE && technique.available);
        if hurt && knows {
            self.healer = Some(actor);
            true
        } else {
            false
        }
    }

    /// A frame's pad: the caller's buttons plus the route's own dialogue
    /// press — Speak on every `PRESS_PERIOD`-th frame while the window is ready
    /// for one, and nothing otherwise. The released frames between presses are
    /// what make each press fresh. A prompt is answered YES; a route that needs
    /// NO would press Cancel instead.
    fn with_dialogue_press(&self, pad: Pad) -> Pad {
        let ready =
            self.session.runtime().dialogue_view().is_some_and(|view| {
                view.dismissable || view.choice.is_some_and(|choice| choice.ready)
            });
        if ready && self.ticks.is_multiple_of(PRESS_PERIOD) {
            pad.with(Button::Speak)
        } else {
            pad
        }
    }

    /// Reports what the dialogue did. A fault is a route failure: the shipped
    /// runtime could not present a line.
    fn present(&mut self, signals: Vec<DialogueSignal>) {
        for signal in signals {
            match signal {
                DialogueSignal::Fault(line) => panic!("dialogue fault: {line}"),
                DialogueSignal::Log(line) => println!("{} dialogue: {line}", self.ticks),
                DialogueSignal::Closed { suspended } => {
                    println!("{} dialogue closed (suspended {suspended})", self.ticks);
                }
                DialogueSignal::ChoiceAnswered(yes) => {
                    println!(
                        "{} dialogue choice: {}",
                        self.ticks,
                        if yes { "YES" } else { "NO" }
                    );
                }
                DialogueSignal::Action(_) => {}
            }
        }
    }

    // Each example is its own crate and uses a subset of this shared module.
    #[allow(dead_code)]
    /// One fresh press of `button`: down for a frame, released for the next,
    /// the shape the oracle tapes use.
    pub fn press_button(&mut self, button: Button) {
        self.tick(Pad::new(button));
        self.tick(Pad::NEUTRAL);
    }

    /// The camp menu, while it is up.
    #[allow(dead_code)]
    fn camp(&self) -> &CampView {
        self.session.camp_view().expect("the camp menu is up")
    }

    #[allow(dead_code)]
    /// Moves a menu cursor onto `target` with Down presses; menus wrap, so it
    /// always arrives. `cursor` reads the cursor out of the current view.
    fn move_camp_cursor(&mut self, cursor: impl Fn(&CampView) -> usize, target: usize) {
        for _ in 0..64 {
            if cursor(self.camp()) == target {
                return;
            }
            self.press_button(Button::Down);
        }
        panic!("the camp cursor never reached {target}");
    }

    #[allow(dead_code)]
    /// Uses a technique from the camp menu with pad presses only: Camp, TECH,
    /// the caster, the technique, the target, then back out to the field.
    /// Returns the result line the menu showed.
    pub fn camp_technique(
        &mut self,
        caster_slot: usize,
        technique: u8,
        target_slot: usize,
    ) -> String {
        self.press_button(Button::Camp);
        assert_eq!(self.camp().page, CampPage::Root, "Camp opens the menu");
        self.move_camp_cursor(|view| view.root_selection, 1);
        self.press_button(Button::Speak);
        assert_eq!(self.camp().page, CampPage::AbilityCharacters);
        assert_eq!(self.camp().ability_kind, CampAbilityKind::Technique);
        let caster = self
            .camp()
            .snapshot
            .party
            .iter()
            .position(|member| member.party_slot == caster_slot)
            .expect("the caster is in the party");
        self.move_camp_cursor(|view| view.ability_character_selection, caster);
        self.press_button(Button::Speak);
        assert_eq!(self.camp().page, CampPage::AbilityList);
        let ability = self
            .camp()
            .ability_options
            .iter()
            .position(|option| option.id == technique)
            .expect("the caster knows the technique");
        self.move_camp_cursor(|view| view.ability_selection, ability);
        self.press_button(Button::Speak);
        if self.camp().page == CampPage::AbilityTarget {
            let target = self
                .camp()
                .snapshot
                .party
                .iter()
                .position(|member| member.party_slot == target_slot)
                .expect("the target is in the party");
            self.move_camp_cursor(|view| view.target_selection, target);
            self.press_button(Button::Speak);
        }
        assert_eq!(self.camp().page, CampPage::AbilityResult);
        let line = self.camp().message.clone();
        for _ in 0..8 {
            if self.session.camp_view().is_none() {
                return line;
            }
            self.press_button(Button::Cancel);
        }
        panic!("the camp menu would not close");
    }

    pub fn settle(&mut self) {
        let mut idle = 0;
        for _ in 0..20_000 {
            self.tick(Pad::NEUTRAL);
            // A battle the route walked into is still the session's: the
            // policy keeps pressing until it releases the field.
            if !self.session.battle_active()
                && !self.runtime().scene_active()
                && !self.runtime().state().is_stepping()
            {
                idle += 1;
                if idle == 2 {
                    return;
                }
            } else {
                idle = 0;
            }
        }
        panic!("scene did not release control");
    }

    fn first_step(&self, target: Cell) -> Option<Direction> {
        let map = self.runtime().map();
        let start = self.runtime().state().cell();
        let mut visited = BTreeMap::from([(start, None)]);
        let mut queue = VecDeque::from([start]);
        while let Some(at) = queue.pop_front() {
            for direction in Direction::ALL {
                let Some(next) = map.neighbor(at, direction) else {
                    continue;
                };
                if visited.contains_key(&next) || !map.is_walkable(next) {
                    continue;
                }
                // Only the requested doorway can be crossed during this leg.
                if next != target
                    && map.warps().iter().any(|w| {
                        map.rect_contains(w.source, next) && !map.rect_contains(w.source, target)
                    })
                {
                    continue;
                }
                let first = visited[&at].unwrap_or(direction);
                if next == target {
                    return Some(first);
                }
                visited.insert(next, Some(first));
                queue.push_back(next);
            }
        }
        None
    }

    pub fn walk_to(&mut self, target: Cell) {
        self.settle();
        let map = self.runtime().map_id();
        for _ in 0..10_000 {
            if self.runtime().map_id() != map {
                self.settle();
                return;
            }
            if self.runtime().scene_active() || self.runtime().state().is_stepping() {
                self.tick(Pad::NEUTRAL);
            } else if self.runtime().state().cell() == target {
                self.settle();
                return;
            } else if let Some(direction) = self.first_step(target) {
                self.tick(press(direction));
            } else {
                panic!(
                    "no walking path on {:?} from {:?} to {target:?}",
                    map,
                    self.runtime().state().cell()
                );
            }
        }
        panic!(
            "walk did not reach {target:?}; at {:?}",
            self.runtime().state().cell()
        );
    }

    pub fn checkpoint(&self, name: &str) {
        println!(
            "CHECKPOINT {name}: tick {}, map {:?}, cell {:?}, party {:?}, money {}",
            self.ticks,
            self.runtime().map_id(),
            self.runtime().state().cell(),
            self.runtime().game().party_members(),
            self.runtime().game().money()
        );
    }
}

/// The button that steps this port's one-list windows from `current` to
/// `wanted`: up or down one row, or the accept when the cursor is there.
fn move_cursor(current: usize, wanted: usize) -> Option<Button> {
    match current.cmp(&wanted) {
        std::cmp::Ordering::Less => Some(Button::Down),
        std::cmp::Ordering::Greater => Some(Button::Up),
        std::cmp::Ordering::Equal => Some(Button::Speak),
    }
}

/// The pad that walks one direction: the d-pad bit the cartridge's own
/// `FieldObj_MovementsTbl` maps to that direction.
pub fn press(direction: Direction) -> Pad {
    Pad::new(match direction {
        Direction::Up => Button::Up,
        Direction::Down => Button::Down,
        Direction::Left => Button::Left,
        Direction::Right => Button::Right,
    })
}
