//! Development route driver: one new game, ordinary movement/interactions.
//! Dialogue is acknowledged headlessly. This is traversal evidence, not a
//! Godot presentation or full-campaign acceptance test.
use psiv_core::battle::{BattleEvent, Command, Outcome, RoundOrders, Side};
use psiv_core::{Cell, Direction, Flag, Input};
use psiv_data::{Ctrl, DialogueSet, Segment};
use psiv_runtime::{Runtime, RuntimeEvent};
use std::collections::{BTreeMap, VecDeque};

pub struct Walk {
    pub rt: Runtime,
    pub dialogue: DialogueSet,
    pub ticks: u64,
    pub battles: usize,
    pub heal_in_battle: bool,
}

impl Walk {
    pub fn tick(&mut self, input: Input) {
        self.ticks += 1;
        assert!(self.ticks < 100_000, "route exceeded its tick budget");
        for event in self.rt.tick(input) {
            match event {
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume => {
                    println!("{} {event:?}", self.ticks);
                    self.rt.dialogue_closed();
                }
                RuntimeEvent::SceneChoiceRequested => panic!("route needs an explicit answer"),
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

    fn talk(&mut self, npc: usize) {
        let tree = self.rt.map_record().unwrap().dialogue_tree;
        let mut id = self.rt.npc_dialogue_id(npc).unwrap();
        for _ in 0..16 {
            let entry = self.dialogue.entry(tree, id).unwrap();
            let mut jumped = false;
            for segment in &entry.segments {
                match segment {
                    Segment::Control(Ctrl::FlagCheck {
                        flag, then_entry, ..
                    }) => {
                        if self.rt.game().is_set(Flag::event(u16::from(*flag))) {
                            id += then_entry;
                            jumped = true;
                            break;
                        }
                    }
                    Segment::Control(Ctrl::Event { id: event, .. }) => {
                        println!(
                            "{} NPC {npc}, tree {tree} entry {id} starts {event:#x}",
                            self.ticks
                        );
                        assert!(self.rt.start_event(*event));
                        return;
                    }
                    _ => {
                        println!(
                            "{} NPC {npc}, tree {tree} entry {id}: ordinary dialogue",
                            self.ticks
                        );
                        return;
                    }
                }
            }
            if !jumped {
                return;
            }
        }
        panic!("NPC preamble did not resolve");
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
