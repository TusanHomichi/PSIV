//! Development route driver: one new game, ordinary movement, ordinary
//! dialogue.
//!
//! The dialogue is the shipped runtime's: the route builds a pad, hands it to
//! `Runtime::dialogue_frame` once per frame and presses Speak through the
//! pages the way a player does, so a route's dialogue is the game's dialogue
//! and not a second implementation of it. This remains traversal evidence,
//! not a Godot presentation or full-campaign acceptance test.
use psiv_core::battle::{BattleEvent, Command, Outcome, RoundOrders, Side};
use psiv_core::{Cell, Direction, Input};
use psiv_runtime::{
    Button, DialogueSignal, NpcDialogueOpen, Pad, Runtime, RuntimeEvent, SceneDialogueOpen,
};
use std::collections::{BTreeMap, VecDeque};

/// Frames between the route's dialogue presses: a fresh press every four
/// frames, with the three released frames in between, which is the shape the
/// oracle tapes use (a held button never advances a finished page).
const PRESS_PERIOD: u64 = 4;

pub struct Walk {
    pub rt: Runtime,
    pub ticks: u64,
    pub battles: usize,
    pub heal_in_battle: bool,
}

impl Walk {
    /// One frame, in the shipped shell's order: the dialogue's input half with
    /// this frame's pad, the field tick (neutral while a window is up), then
    /// the dialogue's own half.
    pub fn tick(&mut self, input: Input) {
        self.ticks += 1;
        assert!(self.ticks < 100_000, "route exceeded its tick budget");
        let dialogue_open = self.rt.dialogue_open();
        let pad = self.pad(dialogue_open);
        let signals = self.rt.dialogue_frame(pad);
        self.present(signals);
        if let Some(event) = self.rt.take_dialogue_event() {
            println!("{} dialogue fires {event:#x}", self.ticks);
            self.rt.start_event(event);
        }
        // A window is up: the field is suspended and gets a neutral input, as
        // the cartridge's interaction mode does. With no window the route
        // releases the suspension itself, exactly where the shipped shell
        // does — a leftover suspension would starve the field's trigger
        // checks (`RunEvents` runs only on an unsuspended tick).
        let events = if dialogue_open {
            self.rt.tick(Input::Neutral)
        } else {
            self.rt.set_field_suspended(false);
            self.rt.tick(input)
        };
        let signals = self.rt.dialogue_tick();
        self.present(signals);
        for event in events {
            match event {
                RuntimeEvent::SceneDialogue { entry } => {
                    println!("{} scene dialogue entry {entry:#04x}", self.ticks);
                    // The runtime resolves the entry against whichever tree
                    // the scene selected, so the headless route needs no scene
                    // presentation state of its own.
                    match self.rt.open_scene_dialogue(entry, false) {
                        SceneDialogueOpen::Empty => self.rt.dialogue_closed(),
                        SceneDialogueOpen::UnknownTree => {
                            panic!("scene dialogue tree is absent from the pack")
                        }
                        SceneDialogueOpen::Opened => {}
                    }
                }
                RuntimeEvent::SceneDialogueResume => {
                    println!("{} {event:?}", self.ticks);
                    if !self.rt.resume_scene_dialogue(false) {
                        self.rt.dialogue_closed();
                    }
                }
                RuntimeEvent::SceneChoiceRequested => {
                    println!("{} scene awaits a choice", self.ticks);
                    self.rt.open_scene_choice();
                }
                RuntimeEvent::Interact { npc_index, .. } => self.talk(npc_index),
                RuntimeEvent::SceneFaulted { .. }
                | RuntimeEvent::SceneMissing { .. }
                | RuntimeEvent::SceneBattleFailed { .. }
                | RuntimeEvent::MapRefreshFailed { .. }
                | RuntimeEvent::UnpackedTarget { .. }
                | RuntimeEvent::WarpUnmapped { .. } => panic!("{event:?}"),
                RuntimeEvent::EncounterRolled { formation } => {
                    self.rt
                        .start_battle_timeline(formation, self.rt.battle_party())
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
    }

    fn fight(&mut self) {
        self.battles += 1;
        let purse = self.rt.game().money();
        println!(
            "BATTLE {}: {:?}",
            self.battles,
            self.rt
                .battle_roster()
                .unwrap()
                .living(Side::Enemy)
                .map(|f| (&f.name, f.stats.curr_hp))
                .collect::<Vec<_>>()
        );
        for round in 1..=100 {
            let roster = self.rt.battle_roster().unwrap();
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
                .rt
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
                for event in self.rt.finish_battle_for_outcome(outcome, reward) {
                    println!("B{} {event:?}", self.battles);
                }
                assert_eq!(self.rt.game().money(), purse + u32::from(meseta));
                return;
            }
        }
        panic!("battle exceeded 100 rounds");
    }

    /// Talks to an object: the runtime resolves the map's tree and the
    /// object's dialogue id, and the route presses through whatever opens.
    fn talk(&mut self, npc: usize) {
        match self.rt.open_npc_dialogue(npc) {
            NpcDialogueOpen::Opened => {}
            NpcDialogueOpen::NoBinding => {
                println!("{} NPC {npc} has no dialogue binding", self.ticks)
            }
            NpcDialogueOpen::Nothing => println!("{} NPC {npc} has nothing to say", self.ticks),
        }
    }

    /// This frame's pad: Speak on every `PRESS_PERIOD`-th frame while the
    /// window is ready for it, and nothing otherwise — the released frames
    /// between presses are what make each press fresh. A prompt is answered
    /// YES; a route that needs NO would press Cancel instead.
    fn pad(&self, dialogue_open: bool) -> Pad {
        let ready = dialogue_open
            && self.rt.dialogue_view().is_some_and(|view| {
                view.dismissable || view.choice.is_some_and(|choice| choice.ready)
            });
        if ready && self.ticks.is_multiple_of(PRESS_PERIOD) {
            Pad::new(Button::Speak)
        } else {
            Pad::NEUTRAL
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
            self.tick(Input::Neutral);
            if !self.rt.scene_active() && !self.rt.state().is_stepping() {
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
        let map = self.rt.map();
        let start = self.rt.state().cell();
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
        let map = self.rt.map_id();
        for _ in 0..10_000 {
            if self.rt.map_id() != map {
                self.settle();
                return;
            }
            if self.rt.scene_active() || self.rt.state().is_stepping() {
                self.tick(Input::Neutral);
            } else if self.rt.state().cell() == target {
                self.settle();
                return;
            } else if let Some(direction) = self.first_step(target) {
                self.tick(Input::Direction(direction));
            } else {
                panic!(
                    "no walking path on {:?} from {:?} to {target:?}",
                    map,
                    self.rt.state().cell()
                );
            }
        }
        panic!(
            "walk did not reach {target:?}; at {:?}",
            self.rt.state().cell()
        );
    }

    pub fn checkpoint(&self, name: &str) {
        println!(
            "CHECKPOINT {name}: tick {}, map {:?}, cell {:?}, party {:?}, money {}",
            self.ticks,
            self.rt.map_id(),
            self.rt.state().cell(),
            self.rt.game().party_members(),
            self.rt.game().money()
        );
    }
}
