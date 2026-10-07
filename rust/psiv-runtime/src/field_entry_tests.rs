//! Map-entry checks use the loaded map before movement or encounter rolls.
use super::*;
use psiv_core::{Input, MapId};
use psiv_data::BattleFiles;
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn runtime(map: u16, cell: Cell) -> Option<Runtime> {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return None;
    }
    Some(
        Runtime::new(
            GameData::load(pack).unwrap(),
            map,
            cell,
            Direction::Up,
            StepFrames::default(),
        )
        .unwrap(),
    )
}

#[test]
fn zema_scene_despawns_last_until_full_object_reload() {
    let Some(mut rt) = runtime(0x24, Cell::new(31, 49)) else {
        return;
    };
    assert!((0..7).all(|i| rt.map.npcs().get(i).unwrap().active));
    assert!(rt.start_event(0x8005));
    let mut despawned = false;
    for _ in 0..10_000 {
        let events = rt.tick(Input::Neutral);
        if events.iter().any(|e| {
            matches!(
                e,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            )
        }) {
            rt.dialogue_closed();
        }
        if events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::NpcsDespawned { first: 0, count: 7 }))
        {
            despawned = true;
            break;
        }
    }
    assert!(
        despawned,
        "Alshline must remove the seven townspeople while staging the monster"
    );
    assert!((0..7).all(|i| !rt.map.npcs().get(i).unwrap().active));
    rt.refresh_field_after_battle().unwrap();
    assert!(
        (0..7).all(|i| !rt.map.npcs().get(i).unwrap().active),
        "battle return retains the live cast"
    );
    rt.game.set(Flag::event(0x33)).unwrap();
    rt.change_map_from(MapId(0x24), Cell::new(30, 17), Direction::Up, 0xFFFF)
        .unwrap();
    assert!(
        (0..7).all(|i| rt.map.npcs().get(i).unwrap().active),
        "a full map load restores the rescued townspeople"
    );
    assert!(
        (7..10).all(|i| !rt.map.npcs().get(i).unwrap().active),
        "original flag-gated rock despawns still apply"
    );
}

#[test]
fn entering_the_valley_with_rune_starts_his_scene_before_another_step() {
    let Some(mut rt) = runtime(0, Cell::new(128, 122)) else {
        return;
    };
    rt.game.set(Flag::event(0x11)).unwrap();
    rt.game.set(Flag::event(0x0C)).unwrap();
    let mut entered = false;
    for _ in 0..32 {
        let events = rt.tick(Input::Direction(Direction::Up));
        if events.iter().any(|e| {
            matches!(
                e,
                RuntimeEvent::MapChanged {
                    map: MapId(0xD8),
                    ..
                }
            )
        }) {
            entered = true;
            break;
        }
    }
    assert!(
        entered,
        "ordinary doorway input must enter the mountain pass"
    );
    let at = rt.state().cell();
    let events = rt.tick(Input::Direction(Direction::Down));
    assert_eq!(events, [RuntimeEvent::SceneStarted { trigger: 0x30 }]);
    assert_eq!(
        rt.state().cell(),
        at,
        "the new scene owns input immediately"
    );
    assert!(rt.scene_active());
}

#[test]
fn loaded_save_checks_entry_once_when_the_field_is_released() {
    let Some(mut rt) = runtime(0xD8, Cell::new(31, 35)) else {
        return;
    };
    rt.game.set(Flag::event(0x11)).unwrap();
    let dir = std::env::temp_dir().join(format!("psiv-entry-save-{}", std::process::id()));
    rt.save_slot(&dir, 0).unwrap();
    let mut loaded = Runtime::load_slot(rt.data.clone(), &dir, 0, StepFrames::default()).unwrap();
    loaded.set_field_suspended(true);
    assert!(loaded.tick(Input::Neutral).is_empty());
    assert!(!loaded.scene_active());
    loaded.set_field_suspended(false);
    assert_eq!(
        loaded.tick(Input::Neutral),
        [RuntimeEvent::SceneStarted { trigger: 0x30 }]
    );
    assert!(
        loaded.tick(Input::Neutral).is_empty(),
        "the next tick enters the running scene"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn map_load_rearms_nine_free_encounter_steps_and_the_tenth_roll() {
    let Some(mut rt) = runtime(0x2B, Cell::new(17, 52)) else {
        return;
    };
    rt.enable_battles(&BattleFiles::load(Path::new(PACK)).unwrap())
        .unwrap();
    for _ in 0..20 {
        rt.battles.as_mut().unwrap().clock.step();
    }
    assert!(rt.battles.as_mut().unwrap().clock.step());
    rt.change_map(MapId(0x2C), Cell::new(24, 49), Direction::Up)
        .unwrap();
    for step in 1..10 {
        assert!(!rt.battles.as_mut().unwrap().clock.step(), "step {step}");
    }
    assert!(rt.battles.as_mut().unwrap().clock.step());
}

#[test]
fn rune_opens_the_live_rock_before_the_story_flag_and_keeps_the_party_staged() {
    let Some(mut rt) = runtime(0xD8, Cell::new(31, 35)) else {
        return;
    };
    rt.game.set_party([
        Some(CharId(1)),
        Some(CharId(0)),
        Some(CharId(2)),
        Some(CharId(3)),
        None,
    ]);
    rt.resize_party();
    rt.game.set(Flag::event(0x11)).unwrap();
    assert!(!rt.map.is_walkable(Cell::new(31, 31)));
    let mut opened = 0;
    for _ in 0..1500 {
        for event in rt.tick(Input::Neutral) {
            match event {
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume => {
                    rt.dialogue_closed()
                }
                RuntimeEvent::MapRefreshed => {
                    opened += 1;
                    assert!(
                        rt.game.is_clear(Flag::event(0x13)),
                        "layout changes before the final flag write"
                    );
                    assert!(rt.map.is_walkable(Cell::new(31, 31)));
                    assert!(rt.effects.patch_blits.is_empty());
                    let rune = rt.scene_party_actor(3).unwrap();
                    assert_eq!(rune.cell, Cell::new(31, 37));
                    assert_ne!(rt.scene_party_actor(0).unwrap().cell, rune.cell);
                }
                RuntimeEvent::SceneFaulted { .. } | RuntimeEvent::MapRefreshFailed { .. } => {
                    panic!("{event:?}")
                }
                _ => {}
            }
        }
        if rt.game.is_set(Flag::event(0x13)) && !rt.scene_active() {
            break;
        }
    }
    assert_eq!(opened, 1);
    assert!(rt.game.is_set(Flag::event(0x13)));
    assert_eq!(rt.state().cell(), Cell::new(31, 34));
    assert!(rt.map.is_walkable(Cell::new(31, 31)));
    rt.change_map(MapId(0xD8), Cell::new(31, 34), Direction::Up)
        .unwrap();
    assert!(
        rt.map.is_walkable(Cell::new(31, 31)),
        "flag preserves the opening on the next load"
    );
}

/// Both of the cartridge's dismounts rebuild every character object on
/// `Character_1`, the machine's body, with its position and facing
/// (`Event_GettingOffVehicle`, `loc_6BF4E`, `ps4.asm:145362-145380`; the
/// mounted branch of `Event_EclipseTorchUsed`, `loc_701DE`, `:149559-149577`).
/// The party objects stayed where the party boarded; a dismount stands them on
/// the machine.
#[test]
fn a_dismount_stands_the_party_on_the_machine() {
    // One step of the Ice Digger from the cell above Meese's door where the
    // route boards it, in the first direction with open ground, from a fresh
    // game each time (a step into a town would park it).
    let mut found = None;
    let mut tried = Vec::new();
    for direction in [
        Direction::Right,
        Direction::Left,
        Direction::Up,
        Direction::Down,
    ] {
        let Some(mut rt) = runtime(1, Cell::new(146, 50)) else {
            return;
        };
        // EventFlag_Snowstorm ($80): past the Raja temple, so the overworld's
        // entry trigger ($31, `Event_OutsideRajaTemple`) stays quiet.
        rt.game.set(Flag::event(0x80)).unwrap();
        let boarded = rt.party.leader().cell();
        rt.set_vehicle_index(2).unwrap();
        let mut last = None;
        let mut stepped = None;
        for _ in 0..120 {
            let events = rt.tick(Input::Direction(direction));
            let Some(vehicle) = rt.vehicle_state() else {
                last = Some(format!("unmounted after {events:?}"));
                break;
            };
            last = Some(format!(
                "{:?} moving {}",
                vehicle.cell(),
                vehicle.is_moving()
            ));
            if vehicle.cell() != boarded && !vehicle.is_moving() {
                stepped = Some(vehicle);
                break;
            }
        }
        if let Some(vehicle) = stepped {
            found = Some((rt, boarded, vehicle));
            break;
        }
        tried.push((
            direction,
            last,
            psiv_core::can_enter(2, &rt.map, boarded, direction),
            rt.vehicle_state().and_then(|v| v.step_timing()),
        ));
    }
    let (mut rt, boarded, vehicle) =
        found.unwrap_or_else(|| panic!("the Ice Digger never drove off its cell: {tried:?}"));
    assert_eq!(rt.party.leader().cell(), boarded, "the party stayed behind");
    rt.set_vehicle_index(0).unwrap();
    assert_eq!(rt.vehicle_index(), None);
    assert_eq!(rt.party.leader().cell(), vehicle.cell());
    assert_eq!(rt.party.leader().facing(), vehicle.facing());
    for member in rt.party.members() {
        assert_eq!(member.cell, vehicle.cell(), "every object stacked");
    }
}

/// `Map_Load_Flags` is the one-shot byte both load routines read. A warp is
/// `GameMode_LoadFieldMap`, whose bits 0 and 2 spare the field objects — and
/// with them `Vehicle_Index` — from `clr.w (Vehicle_Index).w`
/// (`ps4.asm:107507-107517`); a scene's own `LoadMap` is `RefreshMap`, whose
/// keep bit is 3 (`:121769-121780`). Each then consumes what it read
/// (`andi.b #$80` in the field load's tail, `:107594`; zero for the refresh,
/// `:121835`).
#[test]
fn the_load_flags_decide_whether_a_map_load_parks_the_vehicle() {
    let Some(mut rt) = runtime(0, Cell::new(114, 177)) else {
        return;
    };
    // An ordinary warp: no bit set, so the machine stays parked behind.
    rt.set_vehicle_index(2).unwrap();
    rt.change_map_from(MapId(0x39), Cell::new(31, 36), Direction::Up, 0)
        .unwrap();
    assert_eq!(
        rt.vehicle_index(),
        None,
        "GameMode_LoadFieldMap clears the selector"
    );

    // A scene's refresh with bit 3 set keeps it, and zeroes the byte.
    rt.set_vehicle_index(2).unwrap();
    rt.map_load_flags = 0b0000_1000;
    rt.change_map_refresh(MapId(0), Cell::new(114, 177), Direction::Up, 0x39, 0)
        .unwrap();
    assert_eq!(rt.vehicle_index(), Some(2), "RefreshMap's keep bit");
    assert_eq!(rt.map_load_flags, 0, "RefreshMap consumes the flags");

    // The same refresh with the scene's own `bclr #3` parks it.
    rt.change_map_refresh(MapId(0), Cell::new(114, 177), Direction::Up, 0, 0x08)
        .unwrap();
    assert_eq!(rt.vehicle_index(), None);

    // A field load's keep bits are 0 and 2: bit 2 (a cutscene) spares it…
    rt.set_vehicle_index(1).unwrap();
    rt.map_load_flags = 0b0000_0100;
    rt.change_map_from(MapId(0x39), Cell::new(31, 36), Direction::Up, 0)
        .unwrap();
    assert_eq!(rt.vehicle_index(), Some(1), "bit 2 spares the machine");
    assert_eq!(
        rt.map_load_flags & !0b1000_0000,
        0,
        "the field load keeps bit 7 only"
    );

    // …and bit 3 does not: the two routines read different bits of one byte.
    rt.map_load_flags = 0b0000_1000;
    rt.change_map_from(MapId(0), Cell::new(114, 177), Direction::Up, 0x39)
        .unwrap();
    assert_eq!(
        rt.vehicle_index(),
        None,
        "a warp reads bits 0 and 2, never bit 3"
    );
}
