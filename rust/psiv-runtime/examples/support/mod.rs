//! Development route driver: one new game, ordinary movement, ordinary
//! dialogue.
//!
//! The route drives the shipped game's own frame — `Session::frame` — and hands
//! it pads, the way the Godot shell does: walking presses a direction, talking
//! presses Speak, and whatever dialogue opens is pressed through with Speak.
//! This remains traversal evidence, not a Godot presentation or full-campaign
//! acceptance test.
use psiv_core::battle::{BattleEvent, Command, Outcome, RoundOrders, Side};
use psiv_core::{Cell, Direction};
use psiv_runtime::{
    Button, DialogueSignal, NpcDialogueOpen, Pad, Runtime, RuntimeEvent, SceneDialogueOpen, Session,
};
use std::collections::{BTreeMap, VecDeque};

/// Frames between the route's dialogue presses: a fresh press every four
/// frames, with the three released frames in between, which is the shape the
/// oracle tapes use (a held button never advances a finished page).
const PRESS_PERIOD: u64 = 4;

pub struct Walk {
    pub session: Session,
    pub ticks: u64,
    pub battles: usize,
    pub heal_in_battle: bool,
}

impl Walk {
    /// The runtime, for the route's own assertions and for the modes this
    /// harness still drives itself (battles, camp) until the campaign runner
    /// owns them.
    pub fn runtime(&self) -> &Runtime {
        self.session.runtime()
    }

    /// The runtime, mutably, for those same harness-driven modes.
    pub fn runtime_mut(&mut self) -> &mut Runtime {
        self.session.runtime_mut()
    }

    /// One frame, in the shipped shell's order: the session's frame with this
    /// frame's pad, the events a shell would route, then the window's own half.
    pub fn tick(&mut self, pad: Pad) {
        self.ticks += 1;
        assert!(self.ticks < 100_000, "route exceeded its tick budget");
        let pad = self.with_dialogue_press(pad);
        let frame = self.session.frame(pad);
        self.present(frame.signals);
        if let Some(start) = frame.scene_started {
            println!(
                "{} dialogue fires {:#x} (scene started: {})",
                self.ticks, start.event, start.started
            );
        }
        for event in frame.events {
            match event {
                RuntimeEvent::SceneDialogue { entry } => {
                    println!("{} scene dialogue entry {entry:#04x}", self.ticks);
                    // The runtime resolves the entry against whichever tree
                    // the scene selected, so the headless route needs no scene
                    // presentation state of its own.
                    match self.session.runtime_mut().open_scene_dialogue(entry, false) {
                        SceneDialogueOpen::Empty => self.session.runtime_mut().dialogue_closed(),
                        SceneDialogueOpen::UnknownTree => {
                            panic!("scene dialogue tree is absent from the pack")
                        }
                        SceneDialogueOpen::Opened => {}
                    }
                }
                RuntimeEvent::SceneDialogueResume => {
                    println!("{} {event:?}", self.ticks);
                    if !self.session.runtime_mut().resume_scene_dialogue(false) {
                        self.session.runtime_mut().dialogue_closed();
                    }
                }
                RuntimeEvent::SceneChoiceRequested => {
                    println!("{} scene awaits a choice", self.ticks);
                    self.session.runtime_mut().open_scene_choice();
                }
                RuntimeEvent::Interact { npc_index, .. } => self.talk(npc_index),
                RuntimeEvent::SceneFaulted { .. }
                | RuntimeEvent::SceneMissing { .. }
                | RuntimeEvent::SceneBattleFailed { .. }
                | RuntimeEvent::MapRefreshFailed { .. }
                | RuntimeEvent::UnpackedTarget { .. }
                | RuntimeEvent::WarpUnmapped { .. } => panic!("{event:?}"),
                RuntimeEvent::EncounterRolled { formation } => {
                    let party = self.session.runtime().battle_party();
                    self.session
                        .runtime_mut()
                        .start_battle_timeline(formation, party)
                        .unwrap();
                    self.fight();
                }
                RuntimeEvent::SceneBattleStarted { .. } => {
                    self.fight();
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
        // The window's own half of the same frame, after the events above may
        // have opened it.
        let signals = self.session.window_tick();
        self.present(signals);
    }

    fn fight(&mut self) {
        self.battles += 1;
        let purse = self.session.runtime().game().money();
        println!(
            "BATTLE {}: {:?}",
            self.battles,
            self.session
                .runtime()
                .battle_roster()
                .unwrap()
                .living(Side::Enemy)
                .map(|f| (&f.name, f.stats.curr_hp))
                .collect::<Vec<_>>()
        );
        for round in 1..=100 {
            let roster = self.session.runtime().battle_roster().unwrap();
            let target = roster
                .living(Side::Enemy)
                .max_by_key(|f| f.stats.curr_hp)
                .unwrap()
                .id;
            let injured = roster
                .living(Side::Party)
                .filter(|f| u32::from(f.stats.curr_hp) * 3 < u32::from(f.stats.max_hp))
                .min_by_key(|f| f.stats.curr_hp)
                .map(|f| f.id);
            let mut healer_assigned = false;
            let commands = roster
                .side(Side::Party)
                .map(|fighter| {
                    if self.heal_in_battle
                        && !healer_assigned
                        && injured.is_some()
                        && fighter.is_alive()
                        && fighter.stats.can_act()
                        && fighter.stats.status & psiv_core::battle::status::TECH_SEALED == 0
                        && fighter.stats.techniques.contains(&24)
                        && fighter.stats.curr_tp >= 3
                    {
                        healer_assigned = true;
                        Command::Technique {
                            technique: 24,
                            target: injured,
                        }
                    } else {
                        Command::AttackTarget(target)
                    }
                })
                .collect();
            let events = self
                .session
                .runtime_mut()
                .battle_round(&RoundOrders::Commands(commands))
                .unwrap();
            let mut reward = 0;
            let mut meseta = 0;
            let mut outcome = None;
            for event in events {
                println!("B{} R{round} {event:?}", self.battles);
                match event {
                    BattleEvent::UnsupportedAbility { .. } => {
                        panic!("unimplemented enemy ability: {event:?}")
                    }
                    BattleEvent::Rewarded {
                        experience_each,
                        meseta: paid,
                        ..
                    } => {
                        reward = experience_each;
                        meseta = paid;
                    }
                    BattleEvent::Ended { outcome: result } => outcome = Some(result),
                    _ => {}
                }
            }
            if let Some(outcome) = outcome {
                assert_eq!(outcome, Outcome::Victory, "route lost a battle");
                for event in self
                    .session
                    .runtime_mut()
                    .finish_battle_for_outcome(outcome, reward)
                {
                    println!("B{} {event:?}", self.battles);
                }
                assert_eq!(
                    self.session.runtime().game().money(),
                    purse + u32::from(meseta)
                );
                return;
            }
        }
        panic!("battle exceeded 100 rounds");
    }

    /// Talks to an object: the runtime resolves the map's tree and the
    /// object's dialogue id, and the route presses through whatever opens.
    fn talk(&mut self, npc: usize) {
        match self.session.runtime_mut().open_npc_dialogue(npc) {
            NpcDialogueOpen::Opened => {}
            NpcDialogueOpen::NoBinding => {
                println!("{} NPC {npc} has no dialogue binding", self.ticks)
            }
            NpcDialogueOpen::Nothing => println!("{} NPC {npc} has nothing to say", self.ticks),
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

    pub fn settle(&mut self) {
        let mut idle = 0;
        for _ in 0..20_000 {
            self.tick(Pad::NEUTRAL);
            if !self.runtime().scene_active() && !self.runtime().state().is_stepping() {
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
