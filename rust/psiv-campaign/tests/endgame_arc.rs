//! The seams the route past Dark Force 2 leans on, driven with pads from
//! hand-built saves: the field loader's map redirects
//! (`GameMode_LoadFieldMap`, `psiv_runtime::adjust_field_load`) in the
//! runtime and in the planner, `plan --from-save`, and a `go_to_map` whose
//! target's load starts a scene that carries the party off it
//! (`arrival_scene`). They need the repository pack and skip with a message
//! without one.

mod common;

use std::path::PathBuf;
use std::process::Command;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_campaign::exec::{Memory, execute};
use psiv_campaign::halt::{HaltKind, Res};
use psiv_campaign::route::Objective;
use psiv_campaign::{MapGraph, Position, Target};
use psiv_core::{Cell, CharId, Direction, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const DEZOLIS: u16 = 0x001;
const MOTA_SPACEPORT: u16 = 0x0BF;
const DEZO_SPACEPORT: u16 = 0x0D4;
const RESHEL_1: u16 = 0x14D;
const GUMBIOUS_ENTRANCE: u16 = 0x160;
const GUMBIOUS_B2: u16 = 0x164;

/// `EventFlag_DezoSpaceport` (`$82`, the spaceport's warp is open),
/// `DarkForce2`, `Elsydeon` and `Reunion` (`ps4.constants.asm:1572`, `:1597`,
/// `:1645`, `:1646`).
const SPACEPORT_OPEN: u16 = 0x82;
const DARK_FORCE_2: u16 = 0x9E;
const ELSYDEON: u16 = 0xD9;
const REUNION: u16 = 0xDA;

/// `ItemID_IceDigger`.
const ITEM_ICE_DIGGER: u8 = 0x97;

/// Below Dezolis warp 11, the Gumbious entrance's footprint (124..127,68..71).
const BELOW_GUMBIOUS: (u16, u16) = (125, 73);
/// Below Dezolis warp 0, Reshel's footprint (126..127,44..45).
const BELOW_RESHEL: (u16, u16) = (126, 47);
/// West of Dezolis warp 9, the Dezo spaceport's door (12..13,72..73).
const BESIDE_SPACEPORT: (u16, u16) = (9, 74);

/// A Chaz/Rika/Rune/Wren party on Dezolis at `cell` with the story flags the
/// late arc keeps (so no earlier trigger fires on the first frame) and
/// `extra`.
fn party_at(cell: (u16, u16), extra: &[u16]) -> Option<Session> {
    party_with(cell, extra, false)
}

/// As [`party_at`], with the Ice Digger in the pack.
fn party_at_with_digger(cell: (u16, u16), extra: &[u16]) -> Option<Session> {
    party_with(cell, extra, true)
}

fn party_with(cell: (u16, u16), extra: &[u16], ice_digger: bool) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(5)),
        Some(CharId(3)),
        Some(CharId(7)),
        None,
    ]);
    for flag in [
        0x08, 0x66, 0x68, 0x70, 0x71, 0x72, 0x80, 0x81, 0x84, 0x85, 0x88,
    ]
    .iter()
    .chain(extra)
    {
        game.set(Flag::event(*flag)).expect("flag in range");
    }
    if ice_digger {
        game.inventory_mut()
            .add(ITEM_ICE_DIGGER)
            .expect("room for the Ice Digger");
    }
    Some(
        Session::start(set.data.clone())
            .with_battles(set.battle.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 1,
                    map_index_2: 0,
                    map_index: DEZOLIS,
                    char_x: cell.0 * 16,
                    char_y: cell.1 * 16,
                },
            })
            .expect("the map loads"),
    )
}

/// Runs one route objective written as the route file writes it.
fn objective(driver: &mut Driver, json: &str) -> Res {
    let objective: Objective = serde_json::from_str(json).expect("the objective parses");
    execute(driver, &mut Memory::default(), &objective)
}

fn flag(driver: &Driver, id: u16) -> bool {
    driver.runtime().game().is_set(Flag::event(id))
}

fn at(map: u16, (x, y): (u16, u16)) -> Position {
    Position {
        map,
        cell: Cell::new(x, y),
    }
}

/// `AdjustGumbiousMap` (`ps4.asm:111750-111772`) in the planner: with Dark
/// Force 2 fought, Dezolis warp 11 lands in `Gumbious_B2` at `($3C,$44)` =
/// (30,35) facing left; without it, the warp's own record (the entrance,
/// (31,39) facing up), and `B2` is out of reach.
#[test]
fn the_planner_follows_the_gumbious_redirect_only_after_dark_force_2() {
    let Some(pack) = pack() else { return };
    let flags = [Flag::event(DARK_FORCE_2)];
    let after = MapGraph::new(&pack.data, &flags)
        .unwrap()
        .plan(at(DEZOLIS, BELOW_GUMBIOUS), Target::Map(GUMBIOUS_B2))
        .expect("one warp, redirected");
    assert_eq!(after.legs.len(), 1);
    let hop = &after.legs[0].hop;
    assert_eq!(hop.record_offset, "0x10FF4A", "Dezolis warp 11");
    assert_eq!(
        (hop.target_map, hop.arrival, hop.facing),
        (GUMBIOUS_B2, Cell::new(30, 35), Direction::Left)
    );

    let mut before = MapGraph::new(&pack.data, &[]).unwrap();
    let plain = before
        .plan(at(DEZOLIS, BELOW_GUMBIOUS), Target::Map(GUMBIOUS_ENTRANCE))
        .expect("the entrance itself");
    assert_eq!(
        (plain.legs[0].hop.arrival, plain.legs[0].hop.facing),
        (Cell::new(31, 39), Direction::Up)
    );
    assert!(
        before
            .plan(at(DEZOLIS, BELOW_GUMBIOUS), Target::Map(GUMBIOUS_B2))
            .is_err(),
        "B2 has no warp of its own from Dezolis"
    );
}

/// The same redirect in the runtime's field loader: the walk the planner
/// makes lands where it said, on its first load.
#[test]
fn the_gumbious_entrance_loads_b2_after_dark_force_2() {
    let Some(session) = party_at(BELOW_GUMBIOUS, &[DARK_FORCE_2]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver
        .go_to_map(GUMBIOUS_B2, None)
        .expect("the redirected warp");
    let runtime = driver.runtime();
    assert_eq!(runtime.map_id().0, GUMBIOUS_B2);
    assert_eq!(runtime.state().cell(), Cell::new(30, 35));
    assert_eq!(runtime.state().facing(), Direction::Left);
    assert_eq!(
        driver.map_loads(GUMBIOUS_ENTRANCE),
        0,
        "the entrance never loaded"
    );
}

/// Mounted, the same warp goes through the vehicle's own map change: the Ice
/// Digger parks in `B2` and the load reports `B2`, never the entrance.
#[test]
fn the_ice_digger_also_lands_in_b2_after_dark_force_2() {
    let Some(session) = party_at_with_digger(BELOW_GUMBIOUS, &[DARK_FORCE_2, 0x89]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    objective(
        &mut driver,
        r#"{"do": "use_item", "item": "ICE-DIGGER", "target": "Chaz"}"#,
    )
    .expect("the Ice Digger boards");
    assert_eq!(driver.runtime().game().vehicle_index(), 2);
    objective(&mut driver, r#"{"do": "go_to_map", "map": 356}"#).expect("the redirected warp");
    assert_eq!(driver.runtime().map_id().0, GUMBIOUS_B2);
    assert_eq!(driver.runtime().state().cell(), Cell::new(30, 35));
    assert_eq!(
        driver.map_loads(GUMBIOUS_ENTRANCE),
        0,
        "the entrance never loaded"
    );
    assert_eq!(driver.map_loads(GUMBIOUS_B2), 1);
}

/// Negative control for the flag gate: before Dark Force 2 the same warp
/// loads the entrance.
#[test]
fn before_dark_force_2_the_entrance_loads_itself() {
    let Some(session) = party_at(BELOW_GUMBIOUS, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver
        .go_to_map(GUMBIOUS_ENTRANCE, None)
        .expect("the plain warp");
    assert_eq!(driver.runtime().map_id().0, GUMBIOUS_ENTRANCE);
    assert_eq!(driver.runtime().state().cell(), Cell::new(31, 39));
}

/// `AdjustReshelMap` (`ps4.asm:111733-111748`): with Dark Force 2 fought,
/// `Reshel1` loads its second state, at the warp's own cell.
#[test]
fn reshel_loads_its_second_state_after_dark_force_2() {
    let Some(session) = party_at(BELOW_RESHEL, &[DARK_FORCE_2]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver
        .go_to_map(RESHEL_1 + 1, None)
        .expect("the redirected warp");
    assert_eq!(driver.runtime().state().cell(), Cell::new(30, 49));
    assert_eq!(driver.map_loads(RESHEL_1), 0, "Reshel1 never loaded");
}

/// `RunEvent_Reunion` (`RunEventsJmpTbl[$50]`, `ps4.asm:116307`) fires on the
/// Dezo spaceport's load once Elsydeon (`$D9`) is set: `Cutscene_Reunion`
/// carries the party to the Mota spaceport and sets `$DA`. A `go_to_map`
/// marked `arrival_scene` ends there; the same objective unmarked halts
/// instead of walking back into a map whose scene has run. Both go through the
/// route file's own objective and `exec::execute`.
#[test]
fn an_arrival_scene_ends_the_go_to_map_and_an_unmarked_one_halts() {
    let flags = [SPACEPORT_OPEN, DARK_FORCE_2, ELSYDEON];
    let Some(session) = party_at(BESIDE_SPACEPORT, &flags) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    objective(
        &mut driver,
        r#"{"do": "go_to_map", "map": 212, "arrival_scene": true}"#,
    )
    .expect("the Reunion runs and the objective ends");
    assert_eq!(driver.runtime().map_id().0, MOTA_SPACEPORT);
    assert!(flag(&driver, REUNION), "$DA: the cutscene ran to its end");
    assert_eq!(driver.map_loads(DEZO_SPACEPORT), 1);

    let session = party_at(BESIDE_SPACEPORT, &flags).unwrap();
    let mut driver = Driver::new(session, None);
    let halt = objective(&mut driver, r#"{"do": "go_to_map", "map": 212}"#)
        .expect_err("an unmarked objective halts");
    assert_eq!(halt.kind, HaltKind::UnexpectedState);
    assert!(halt.detail.contains("arrival_scene"), "{}", halt.detail);
    assert_eq!(driver.runtime().map_id().0, MOTA_SPACEPORT);
}

/// Negative control for `arrival_scene`: without Elsydeon no scene runs, so
/// the marked objective is an ordinary arrival.
#[test]
fn without_a_scene_an_arrival_scene_objective_simply_arrives() {
    let Some(session) = party_at(BESIDE_SPACEPORT, &[SPACEPORT_OPEN, DARK_FORCE_2]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    objective(
        &mut driver,
        r#"{"do": "go_to_map", "map": 212, "arrival_scene": true}"#,
    )
    .expect("a plain arrival");
    assert_eq!(driver.runtime().map_id().0, DEZO_SPACEPORT);
    assert!(!flag(&driver, REUNION));
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_psiv-campaign"))
}

/// Writes a save of `session` as `<dir>/slot_1.sram`.
fn write_save(session: &Session, name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    session
        .runtime()
        .save_slot(&dir, 1)
        .expect("the slot writes")
}

/// `plan --from-save` plans from the save's position with the save's flags:
/// with Dark Force 2 in the save, B2 is one redirected warp away. The same
/// position named with `--from-map`/`--from-cell` and no flags has no plan
/// (the negative control: the flags came from the save), and `--from-save`
/// beside a half `--from-map` is a usage error.
#[test]
fn plan_from_save_reads_the_saves_position_and_flags() {
    let Some(session) = party_at(BELOW_GUMBIOUS, &[DARK_FORCE_2]) else {
        return;
    };
    let save = write_save(&session, "endgame-from-save");
    let out = bin()
        .arg("plan")
        .arg("--from-save")
        .arg(&save)
        .args(["--to-map", "0x164"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("from map 0x001 (125,73)"), "{text}");
    assert!(text.contains("arrive map 0x164 (30,35)"), "{text}");

    let bare = bin()
        .args([
            "plan",
            "--from-map",
            "1",
            "--from-cell",
            "125,73",
            "--to-map",
            "0x164",
        ])
        .output()
        .unwrap();
    assert_eq!(bare.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&bare.stdout).contains("no plan"));

    let mixed = bin()
        .arg("plan")
        .arg("--from-save")
        .arg(&save)
        .args(["--from-map", "1", "--to-map", "0x164"])
        .output()
        .unwrap();
    assert_eq!(mixed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&mixed.stderr).contains("--from-save alone"));
}
