//! Per-scene coverage for the retail scene arc after Piata.
//!
//! Each scene is started directly through `Runtime::start_event` at its own
//! retail map, so this target proves the scene reaches its return edge, that
//! the follow-up chains write their retail flags, that NPC motion and map
//! recasts land in the live `FieldMap`, and that the walks between the arc's
//! maps keep working. The campaign chain is in `next_arc.rs`; the scripted
//! battles are in `next_arc_battles.rs`.

use std::path::Path;

use crate::{Runtime, RuntimeEvent};
use psiv_core::battle::{BattleEvent, Outcome, RoundOrders};
use psiv_core::{
    Cell, CharId, Direction, Flag, GameState, Input, RetailLocation, RetailSave, SceneOp,
    StepFrames,
};
use psiv_data::{BattleFiles, GameData};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn runtime_at(map: u16) -> Runtime {
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    Runtime::new(
        data,
        map,
        Cell::new(1, 1),
        Direction::Down,
        StepFrames::default(),
    )
    .expect("runtime starts")
}

fn runtime_with_four_party_at(map: u16) -> Runtime {
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let mut game = GameState::new();
    let fourth = if map == 0xD8 { CharId(3) } else { CharId(4) };
    for (slot, id) in [CharId(0), CharId(1), CharId(2), fourth]
        .into_iter()
        .enumerate()
    {
        game.set_party_slot(slot, Some(id))
            .expect("party slot exists");
    }
    Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: map,
                char_x: 16,
                char_y: 16,
            },
        },
        StepFrames::default(),
    )
    .expect("runtime with seeded party starts")
}

fn relocate_with_state(runtime: &Runtime, map: u16) -> Runtime {
    Runtime::from_save(
        runtime.data().clone(),
        RetailSave {
            snapshot: runtime.game().snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: map,
                char_x: 0x1E0,
                char_y: 0x120,
            },
        },
        StepFrames::default(),
    )
    .expect("runtime relocates with persistent state")
}

fn relocate_with_edit<F>(runtime: &Runtime, map: u16, edit: F) -> Runtime
where
    F: FnOnce(&mut GameState),
{
    let mut game = GameState::from_snapshot(&runtime.game().snapshot());
    edit(&mut game);
    Runtime::from_save(
        runtime.data().clone(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: map,
                char_x: 0x1E0,
                char_y: 0x120,
            },
        },
        StepFrames::default(),
    )
    .expect("runtime relocates with edited persistent state")
}

fn relocate_with_battles(runtime: &Runtime, map: u16) -> Runtime {
    let mut relocated = relocate_with_state(runtime, map);
    let files = BattleFiles::load(Path::new(PACK)).expect("battle files load");
    relocated
        .enable_battles(&files)
        .expect("battles enable after relocation");
    relocated
}

fn drive_scene(runtime: &mut Runtime, event: u16) -> Vec<RuntimeEvent> {
    assert!(runtime.start_event(event), "event {event:#x} starts");
    let mut log = Vec::new();
    for _ in 0..50_000 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
            assert!(
                !matches!(item, RuntimeEvent::SceneFaulted { .. }),
                "event {event:#x} faulted: {item:?}; party={:?}; actors={:?}",
                runtime.game().party_members(),
                runtime.scene_actors()
            );
            if matches!(item, RuntimeEvent::SceneChoiceRequested) {
                runtime.dialogue_choice(true);
            }
            if matches!(
                item,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
            if matches!(
                item,
                RuntimeEvent::ScenePresentation {
                    op: SceneOp::WaitForStart
                }
            ) {
                runtime.ending_continue();
            }
            assert!(
                !matches!(item, RuntimeEvent::SceneBattleStarted { .. }),
                "event {event:#x} unexpectedly owns a battle"
            );
        }
        log.extend(events);
        if !runtime.scene_active() {
            return log;
        }
    }
    panic!(
        "event {event:#x} did not finish; tail: {:?}",
        &log[log.len().saturating_sub(8)..]
    );
}

fn drive_until_battle(runtime: &mut Runtime, event: u16) -> Vec<RuntimeEvent> {
    assert!(runtime.start_event(event), "event {event:#x} starts");
    let mut log = Vec::new();
    for _ in 0..20_000 {
        let events = runtime.tick(Input::Neutral);
        let battle_started = events
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { .. }));
        for item in &events {
            assert!(
                !matches!(item, RuntimeEvent::SceneFaulted { .. }),
                "event {event:#x} faulted: {item:?}"
            );
            if matches!(item, RuntimeEvent::SceneChoiceRequested) {
                runtime.dialogue_choice(true);
            }
            if matches!(
                item,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        log.extend(events);
        if battle_started {
            return log;
        }
    }
    panic!("event {event:#x} never requested its battle");
}

fn resolve_scene_battle(runtime: &mut Runtime) -> Outcome {
    resolve_scene_battle_with_limit(runtime, 400)
}

fn resolve_scene_battle_with_limit(runtime: &mut Runtime, max_rounds: usize) -> Outcome {
    let mut outcome = None;
    let mut last_events = Vec::new();
    for _ in 0..max_rounds {
        let events = runtime
            .battle_round(&RoundOrders::attack_all())
            .expect("battle round resolves");
        last_events = events.clone();
        for event in events {
            if let BattleEvent::Ended { outcome: result } = event {
                outcome = Some(result);
            }
        }
        if outcome.is_some() {
            break;
        }
    }
    let outcome = outcome.unwrap_or_else(|| {
        panic!(
            "scene battle ends; active={} party_len={} last={:?}",
            runtime.battle_active(),
            runtime.battle_party().len(),
            last_events
        )
    });
    runtime.finish_battle_for_outcome(outcome, 0);

    for _ in 0..100 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
            if matches!(item, RuntimeEvent::SceneChoiceRequested) {
                runtime.dialogue_choice(true);
            }
            if matches!(
                item,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        if !runtime.scene_active() {
            return outcome;
        }
    }
    panic!("scene did not consume BattleFinished");
}

#[test]
fn every_next_arc_scene_reaches_its_return_edge() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }

    // One entry per registered scene which does not own a battle. The map is
    // the retail map that supplies the scene's NPC indices when it needs one.
    for (name, map, event) in [
        ("ProfHolt", 0x2B, 0x8002),
        ("MeetingRune", 0x41, 0x8003),
        ("MeetingDorin", 0x43, 0x0032),
        ("Dorin", 0x43, 0x8004),
        ("RuneFlaeli", 0xD8, 0x0027),
        ("AlshlineFound", 0x4A, 0x0028),
        ("ZemaIgglanovaDefeated", 0x24, 0x8006),
        ("ZemaOldMan", 0x24, 0x008B),
        ("ZemaOldManAfterMission", 0x24, 0x008C),
        ("MeetingSaya", 0x3A, 0x000D),
        ("TonoeBasementDoor", 0x42, 0x0033),
        ("BioPlantAlarm", 0xA3, 0x0012),
        ("GirlsSneakingOut", 0x63, 0x0023),
        ("ChazHouse", 0x5E, 0x003B),
        ("LeavingChazHouse", 0x54, 0x003C),
        ("MeetingRika", 0xAC, 0x8007),
    ] {
        let mut runtime = if event == 0x0032 {
            let rt = runtime_with_four_party_at(map);
            relocate_with_edit(&rt, map, |game| {
                game.set_party([
                    Some(CharId(0)),
                    Some(CharId(1)),
                    Some(CharId(2)),
                    Some(CharId(3)),
                    None,
                ]);
            })
        } else if event == 0x8007 || event == 0x0027 {
            runtime_with_four_party_at(map)
        } else {
            runtime_at(map)
        };
        let log = drive_scene(&mut runtime, event);
        assert!(
            log.iter()
                .any(|item| matches!(item, RuntimeEvent::SceneEnded)),
            "{name} emits SceneEnded"
        );
    }
}

#[test]
fn the_followup_chain_carries_alarm_through_rika_recruitment() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }

    // The alarm fires in BioPlant Part2 and the Rika trigger is in its later
    // B4 Part2 room. The map traversal between them is player-controlled, so
    // this single runtime starts at the latter room after the retail walk.
    let mut runtime = runtime_with_four_party_at(0xAC);
    let initial_inventory = *runtime.game().inventory().slots();
    let _ = drive_scene(&mut runtime, 0x0012);
    assert!(runtime.game().is_set(Flag::temp(0x08)));
    assert!(runtime.game().is_clear(Flag::event(0x34)));

    let log = drive_scene(&mut runtime, 0x8007);
    assert_eq!(
        runtime.map_id().0,
        0x00,
        "Rika tail: {:?}",
        &log[log.len().saturating_sub(12)..]
    );
    assert!(runtime.game().is_set(Flag::event(0x34)));
    assert!(runtime.game().is_set(Flag::event(0x35)));
    assert_eq!(runtime.game().party_slot(4).map(|id| id.0), Some(5));
    assert_eq!(*runtime.game().inventory().slots(), initial_inventory);
}

#[test]
fn the_shop_and_house_followups_write_their_retail_flags() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }

    let mut girls = runtime_at(0x63);
    let girls_inventory = *girls.game().inventory().slots();
    let _ = drive_scene(&mut girls, 0x0023);
    assert!(girls.game().is_set(Flag::event(0x46)));
    assert_eq!(*girls.game().inventory().slots(), girls_inventory);

    let mut house = runtime_at(0x5E);
    let house_inventory = *house.game().inventory().slots();
    let _ = drive_scene(&mut house, 0x003B);
    assert_eq!(house.map_id().0, 0x5E);
    assert!(house.game().is_set(Flag::temp(0x18)));
    assert_eq!(*house.game().inventory().slots(), house_inventory);

    let _ = drive_scene(&mut house, 0x003C);
    assert!(house.game().is_clear(Flag::temp(0x18)));
    assert_eq!(*house.game().inventory().slots(), house_inventory);
}

#[test]
fn dorin_motion_lands_in_the_live_field_map() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut runtime = runtime_at(0x43);
    let _ = drive_scene(&mut runtime, 0x8004);

    assert_eq!(runtime.map_id().0, 0x43, "Dorin refreshes TonoeGryzHouse");
    assert_eq!(runtime.map().npcs()[6].cell, Cell::new(0x1F, 0x31));
    assert_eq!(runtime.map().npcs()[0].cell, Cell::new(0x1E, 0x31));
    assert!(runtime.game().is_set(Flag::event(0x30)));
}

#[test]
fn the_arc_walks_from_holt_through_tonoe_to_birth_valley() {
    if !Path::new(PACK).join("battle").is_dir() {
        eprintln!("pack battle section not present; skipping");
        return;
    }
    let mut runtime = runtime_with_four_party_at(0x2B);
    runtime
        .enable_battles(&BattleFiles::load(Path::new(PACK)).unwrap())
        .unwrap();

    let _ = drive_scene(&mut runtime, 0x8002);
    assert_eq!(runtime.map_id().0, 0x24);
    assert!(runtime.game().is_set(Flag::event(0x10)));

    runtime = relocate_with_battles(&runtime, 0x40);
    let _ = drive_scene(&mut runtime, 0x8003);
    assert!(runtime.game().is_set(Flag::event(0x11)));

    runtime = relocate_with_battles(&runtime, 0xD8);
    let _ = drive_scene(&mut runtime, 0x0027);
    assert!(runtime.game().is_set(Flag::event(0x13)));
    assert!(runtime.map().is_walkable(Cell::new(31, 31)));
    runtime = relocate_with_battles(&runtime, 0x43);
    let _ = drive_scene(&mut runtime, 0x0032);
    assert!(runtime.game().is_set(Flag::event(0x36)));

    let _ = drive_scene(&mut runtime, 0x8004);
    assert_eq!(runtime.map_id().0, 0x43);
    assert!(runtime.game().is_set(Flag::event(0x30)));

    let _ = drive_scene(&mut runtime, 0x0028);
    assert!(runtime.game().is_set(Flag::event(0x32)));

    let _ = drive_until_battle(&mut runtime, 0x8005);
    assert_eq!(
        resolve_scene_battle(&mut runtime),
        Outcome::Victory,
        "headless arc wins its battle"
    );
    assert!(runtime.game().is_set(Flag::event(0x33)));

    let _ = drive_scene(&mut runtime, 0x8006);
    assert_eq!(runtime.map_id().0, 0x24);
    assert!(runtime.game().is_set(Flag::event(0x37)));
}

/// `Event_ZioFortBarrier` (`$30`) against Zio Fort `$82`: the ring walk, the
/// two dialogues, and the map state the event leaves behind.
///
/// This is the H18 acceptance. The party is the campaign's post-Psycho-Wand
/// roster (Chaz, Gryz, Rika, Demi and Rune), and `$63` — After Alys Death —
/// must be set for the event's main path; with it clear the cartridge plays
/// dialogue `$4A` and stops.
#[test]
fn the_zio_fort_barrier_walks_its_ring_and_opens_the_courtyard() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }

    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let mut game = GameState::new();
    // The campaign's own roster order at this point: Gryz leads, and Rune
    // has joined from the Ladea Tower pair.
    for (slot, id) in [CharId(4), CharId(0), CharId(5), CharId(6), CharId(3)]
        .into_iter()
        .enumerate()
    {
        game.set_party_slot(slot, Some(id))
            .expect("party slot exists");
    }
    game.set(Flag::event(0x63)).expect("flag exists");
    let mut runtime = Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x82,
                char_x: 47 * 16,
                char_y: 56 * 16,
            },
        },
        StepFrames::default(),
    )
    .expect("runtime at Zio Fort");

    // The barrier's eight objects stand before the event and are gone after.
    for index in 4..12 {
        assert!(
            runtime.map.npcs()[index].active,
            "object {index} is a barrier blocker before the scene"
        );
    }

    // Drive the scene by hand: the ring is complete when dialogue `$45`
    // opens, and the event's last moves come after it.
    assert!(runtime.start_event(0x0030), "the barrier event starts");
    let mut log = Vec::new();
    let mut ring = None;
    let mut entries = Vec::new();
    for _ in 0..50_000 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
            assert!(
                !matches!(item, RuntimeEvent::SceneFaulted { .. }),
                "the barrier event faulted: {item:?}"
            );
            if let RuntimeEvent::SceneDialogue { entry } = item {
                entries.push(*entry);
                if *entry == 0x45 {
                    // The scene's own cast, not the field party: a running
                    // scene owns the cast's positions until it hands them back
                    // on its last tick.
                    ring = Some(
                        runtime
                            .scene_actors()
                            .iter()
                            .filter_map(|actor| match actor.actor {
                                psiv_core::ActorRef::PartyMember(slot) => Some((slot, actor.cell)),
                                _ => None,
                            })
                            .collect::<Vec<_>>(),
                    );
                }
            }
            if matches!(
                item,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        log.extend(events);
        if !runtime.scene_active() {
            break;
        }
    }
    assert_eq!(entries, vec![0x44, 0x45], "both barrier dialogues open");

    // The ring, complete when `$45` opens: each destination is that
    // character's own `move.w #x,$38(a4) / move.w #y,$3A(a4)` pair, and the
    // standing cell is the pixel target's cell plus the one-row shift every
    // field walk uses.
    let mut ring = ring.expect("dialogue $45 opens after the ring walk");
    ring.sort_unstable();
    for (slot, who, x, y, cell) in [
        (0, "Gryz", 0x310, 0x380, Cell::new(49, 57)),
        (1, "Chaz", 0x2F0, 0x380, Cell::new(47, 57)),
        (2, "Rika", 0x2E0, 0x380, Cell::new(46, 57)),
        (3, "Demi", 0x300, 0x370, Cell::new(48, 56)),
        (4, "Rune", 0x2F0, 0x370, Cell::new(47, 56)),
    ] {
        assert_eq!(
            ring[slot].1, cell,
            "{who} should stand at its retail destination (${x:X},${y:X}); ring {ring:?}"
        );
    }

    // The event's last move walks the leader — `Character_1` — to
    // ($2F0,$350), cell (47,54), with the follow chain restored, so the rest
    // of the ring trails him.
    assert_eq!(runtime.state().cell(), Cell::new(47, 54));
    assert!(runtime.game().is_set(Flag::event(0x64)));

    for index in 4..12 {
        assert!(
            !runtime.map.npcs()[index].active,
            "object {index} is cleared by the event"
        );
    }

    // The courtyard's warp 8 at (47,49) is the only way to Nurvus $D7.
    let mut arrived = None;
    for _ in 0..40 {
        for event in runtime.tick(Input::Direction(Direction::Up)) {
            if let RuntimeEvent::MapChanged { map, .. } = event {
                arrived = Some(map);
            }
        }
        if arrived.is_some() {
            break;
        }
    }
    assert_eq!(arrived, Some(psiv_core::MapId(0xD7)), "the courtyard opens");
    assert_eq!(runtime.state().cell(), Cell::new(32, 35));
}
