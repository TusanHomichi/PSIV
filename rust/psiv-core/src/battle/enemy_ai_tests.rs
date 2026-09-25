//! The AI instruction block's arms, one holds/does-not-hold pair each, and the
//! census that keeps a new condition id from slipping in unmodelled.
//!
//! The *fixture* replays (`replay`'s `every_fixture_replays_as_recorded`) are
//! the end-to-end check that an arm's fire in a real battle matches the
//! cartridge's log; these tests are the per-arm unit level the ledger's tables
//! are written from.

use super::*;
use crate::battle::fixtures;
use crate::battle::rng::SliceRolls;
use crate::battle::stats::status;
use crate::battle::{Roster, Stats};

fn id(n: u8) -> FighterId {
    FighterId::new(n).expect("a fighter id")
}

/// An enemy record standing in for a pack record: only the id and the AI block
/// matter to the dispatch.
fn record(enemy: u16, conditions: [u8; 4], abilities: [u8; 4]) -> EnemyRecord {
    EnemyRecord {
        id: enemy,
        name: format!("ENEMY{enemy}"),
        condition_ids: conditions,
        conditional_abilities: abilities,
        ..fixtures::zoran_bult()
    }
}

/// A roster with the named enemy slots: `(slot, enemy id, conditions, ability)`.
fn roster(rows: &[(u8, u16, [u8; 4], [u8; 4])]) -> Roster {
    let mut r = Roster::new();
    for (slot, enemy, conditions, abilities) in rows {
        r.add_enemy(*slot, &record(*enemy, *conditions, *abilities))
            .expect("a free enemy slot");
    }
    r
}

/// A record whose four slots all name `condition` for `ability`.
fn one_arm(condition: u8, ability: u8) -> EnemyRecord {
    record(0, [condition; 4], [ability; 4])
}

/// Runs the block on a fresh slice of draws and reports what it decided and how
/// many draws it took.
fn scan(roster: &mut Roster, actor: u8, record: &EnemyRecord, draws: &[u16]) -> (AiOutcome, usize) {
    let mut rolls = SliceRolls::new(draws);
    let outcome = instruction_block(roster, id(actor), record, &mut rolls).expect("a table id");
    (outcome, rolls.drawn())
}

/// What the engine makes of an outcome: the ability the actor runs.
fn ability_of(record: &EnemyRecord, outcome: AiOutcome, rolled: u8) -> u8 {
    match outcome {
        AiOutcome::Replaced { slot, .. } => record.conditional_abilities[slot],
        AiOutcome::Rolled => rolled,
        AiOutcome::Unsupported { condition } => condition.written_ability(record),
    }
}

/// The neighbour an outcome named, for the fission arm.
fn neighbour_of(outcome: AiOutcome) -> Option<FighterId> {
    match outcome {
        AiOutcome::Replaced { neighbour, .. } => neighbour,
        other => panic!("expected a replacement, got {other:?}"),
    }
}

/// `max_hp` and `curr_hp` on the fighter in `slot`.
fn hp(roster: &mut Roster, slot: u8, curr: u16, max: u16) {
    let fighter = roster.get_mut(id(slot)).expect("a seated enemy");
    fighter.stats.curr_hp = curr;
    fighter.stats.max_hp = max;
}

fn seat_party(roster: &mut Roster, members: usize) {
    let stats = Stats::from_character(&fixtures::chaz(), |item| {
        fixtures::items().into_iter().find(|i| i.id == item)
    });
    for member in 0..members {
        roster.add_party_member(member as u8, format!("P{member}"), stats.clone());
    }
}

// --- the table and the census ---------------------------------------------

#[test]
fn the_table_is_the_cartridges_twenty_entries() {
    assert_eq!(CONDITIONS.len(), 20, "twenty `dc.w` entries");
    assert_eq!(CONDITIONS[0], EnemyAiCondition::Nothing, "$00");
    assert_eq!(CONDITIONS[0x13], EnemyAiCondition::Nothing, "$13");
    assert_eq!(CONDITIONS[0x0F], EnemyAiCondition::HalfHpOrLowerAllEnemies);
    assert_eq!(CONDITIONS[0x10], EnemyAiCondition::Unknown);
    // Every entry round-trips through the id the data carries, and the two
    // `Nothing`s are the only pair that shares an arm.
    for (index, arm) in CONDITIONS.iter().enumerate() {
        assert!(
            arm.ids().any(|id| usize::from(id) == index),
            "entry {index:#04X} is reachable from its own id"
        );
    }
    assert_eq!(
        EnemyAiCondition::Nothing.ids().collect::<Vec<_>>(),
        vec![0x00, 0x13]
    );
    // The byte doubling drops the top bit before the table is read.
    assert_eq!(
        EnemyAiCondition::from_id(0x80),
        Some(EnemyAiCondition::Nothing)
    );
    assert_eq!(
        EnemyAiCondition::from_id(0x81),
        Some(EnemyAiCondition::EmptySpace)
    );
    // Past the table: retail's `adda.w` has no bound check and jumps anywhere.
    for id in [0x14u8, 0x1F, 0x7F] {
        assert_eq!(EnemyAiCondition::from_id(id), None, "{id:#04X}");
    }
}

#[test]
fn an_id_past_the_table_refuses_the_turn() {
    let mut r = roster(&[(1, 0, [0x14, 0, 0, 0], [0; 4])]);
    let mut rolls = SliceRolls::new(&[0]);
    assert_eq!(
        instruction_block(
            &mut r,
            id(6),
            &record(0, [0x14, 0, 0, 0], [0; 4]),
            &mut rolls
        ),
        Err(BattleDataError::UnknownAiCondition(0x14))
    );
}

#[test]
fn only_the_two_arms_without_a_d1_leave_the_scan_running() {
    for arm in CONDITIONS {
        let expected = !matches!(
            arm,
            EnemyAiCondition::Nothing | EnemyAiCondition::CRayTubeNearSatMinion
        );
        assert_eq!(arm.stops_the_scan(), expected, "{arm:?}");
    }
}

/// The pack's own records, where the whole 153-record space is on disk.
///
/// The pack is Sega-derived and never committed (`psiv-data`'s
/// `runtime_pack.rs` has the same gate), so a fresh clone skips this and the
/// committed fixture census below still runs. Build one with
/// `python -m psiv_tools pack <rom> runtime-pack/`, or point
/// `PSIV_RUNTIME_PACK` at an existing one.
#[test]
fn every_condition_id_the_pack_names_maps_to_an_arm() {
    let path = match std::env::var_os("PSIV_RUNTIME_PACK") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack"),
    }
    .join("battle/enemies.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipping: no pack at {}", path.display());
        return;
    };
    let parsed: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let records = parsed["enemies"].as_array().expect("an enemies array");
    assert!(records.len() > 100, "the pack holds every EnemyData record");
    let mut census: Vec<EnemyAiCondition> = Vec::new();
    for enemy in records {
        let symbol = enemy["symbol"].as_str().unwrap_or("?");
        let conditions: Vec<u8> = enemy["ai"]["condition_ids"]
            .as_array()
            .expect("four condition bytes")
            .iter()
            .map(|v| v.as_u64().expect("a byte") as u8)
            .collect();
        let abilities: Vec<u8> = enemy["ai"]["conditional_ability_ids"]
            .as_array()
            .expect("four conditional bytes")
            .iter()
            .map(|v| v.as_u64().expect("a byte") as u8)
            .collect();
        assert_eq!(conditions.len(), CONDITION_SLOTS, "{symbol}");
        let mut previous: Option<EnemyAiCondition> = None;
        for (&condition, &ability) in conditions.iter().zip(&abilities) {
            if condition == 0 {
                break;
            }
            let arm = EnemyAiCondition::from_id(condition).unwrap_or_else(|| {
                panic!(
                    "{symbol}: condition {condition:#04X} is not an EnemyAIInstructionsOffs entry"
                )
            });
            if !census.contains(&arm) {
                census.push(arm);
            }
            // An arm that writes no `d1` leaves the scan's `tst.b d1` reading
            // a register the frame's other code owns. A record may repeat it —
            // the ability it writes cannot change — but a *different* arm after
            // it would make the port's "keep scanning" a guess the log could
            // contradict.
            if let Some(previous) = previous
                && !previous.stops_the_scan()
            {
                assert_eq!(
                    previous, arm,
                    "{symbol}: {previous:?} writes no `d1`, so the scan cannot be trusted \
                     to reach {arm:?}"
                );
            }
            previous = Some(arm);
            // A slot that names a condition names an ability with it: the arm
            // writes `$3(a0)`.
            assert_ne!(
                ability, 0,
                "{symbol}: condition {condition:#04X} has no conditional ability"
            );
        }
    }
    // Eighteen of the nineteen dispatchable arms: the data never names `$10`
    // `Unknown` (nothing in the disassembly sets `$FFFFEEA4` bit 0), and `$00`
    // is unreachable — a zero byte ends the scan.
    assert_eq!(
        census.len(),
        18,
        "the pack's records name every dispatchable arm: {census:?}"
    );
    assert!(
        !census.contains(&EnemyAiCondition::Unknown),
        "no record names $10"
    );
}

/// The swept fixtures' records, which are committed and always present.
#[test]
fn every_condition_id_the_fixtures_name_maps_to_an_arm() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/battle/replay_fixtures/motavia_pack.json");
    let text = std::fs::read_to_string(&path).expect("the swept pack is committed");
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("it parses");
    let mut seen: Vec<EnemyAiCondition> = Vec::new();
    let mut records = 0;
    for enemy in parsed["enemies"].as_array().expect("an enemies array") {
        records += 1;
        for byte in enemy["condition_ids"].as_array().expect("four bytes") {
            let condition = byte.as_u64().expect("a byte") as u8;
            if condition == 0 {
                continue;
            }
            let arm = EnemyAiCondition::from_id(condition).unwrap_or_else(|| {
                panic!("condition {condition:#04X} is not an EnemyAIInstructionsOffs entry")
            });
            if !seen.contains(&arm) {
                seen.push(arm);
            }
        }
    }
    assert!(records > 10, "the sweep's records are present");
    assert!(
        seen.iter().all(|arm| CONDITIONS.contains(arm)),
        "every fixture id is a table entry: {seen:?}"
    );
}

// --- arm $00 / $13: Nothing ------------------------------------------------

#[test]
fn nothing_leaves_the_roll_alone() {
    let mut r = roster(&[(1, 0, [0x13; 4], [9; 4])]);
    let (outcome, drawn) = scan(&mut r, 6, &one_arm(0x13, 9), &[]);
    assert_eq!(outcome, AiOutcome::Rolled);
    assert_eq!(drawn, 0);
    assert_eq!(
        ability_of(&one_arm(0x13, 9), outcome, 3),
        3,
        "the roll stands"
    );
}

// --- arm $01: EmptySpace ---------------------------------------------------

#[test]
fn empty_space_fires_on_the_side_that_is_gone() {
    for (dead, side) in [(6u8, 6u8), (8, 8)] {
        let mut r = roster(&[
            (1, 0, [1; 4], [6; 4]),
            (2, 0, [1; 4], [6; 4]),
            (3, 0, [1; 4], [6; 4]),
        ]);
        let fighter = r.get_mut(id(dead)).expect("seated");
        fighter.stats.curr_hp = 0;
        fighter.stats.status = status::DEAD;
        let (outcome, drawn) = scan(&mut r, 7, &one_arm(1, 6), &[0]);
        assert_eq!(neighbour_of(outcome), Some(id(side)));
        assert_eq!(ability_of(&one_arm(1, 6), outcome, 0), 6);
        assert_eq!(drawn, 0, "one empty side costs no draw");
    }
}

#[test]
fn empty_space_holds_off_while_both_neighbours_stand() {
    let mut r = roster(&[
        (1, 0, [1; 4], [6; 4]),
        (2, 0, [1; 4], [6; 4]),
        (3, 0, [1; 4], [6; 4]),
    ]);
    let (outcome, drawn) = scan(&mut r, 7, &one_arm(1, 6), &[0]);
    assert_eq!(outcome, AiOutcome::Rolled);
    assert_eq!(drawn, 0);
}

#[test]
fn empty_space_draws_once_for_two_empty_sides_even_left_odd_right() {
    let record = one_arm(1, 6);
    for (draw, expected) in [(0u16, 6u8), (1, 8), (0xFFFE, 6), (0xFFFF, 8)] {
        let mut r = roster(&[
            (1, 0, [1; 4], [6; 4]),
            (2, 0, [1; 4], [6; 4]),
            (3, 0, [1; 4], [6; 4]),
        ]);
        for dead in [6, 8] {
            let fighter = r.get_mut(id(dead)).expect("seated");
            fighter.stats.curr_hp = 0;
            fighter.stats.status = status::DEAD;
        }
        let (outcome, drawn) = scan(&mut r, 7, &record, &[draw]);
        assert_eq!(
            neighbour_of(outcome),
            Some(id(expected)),
            "draw {draw:#06X}"
        );
        assert_eq!(drawn, 1);
    }
}

#[test]
fn empty_space_reads_the_party_slot_beside_the_first_enemy() {
    // `prev_obj` of Fighter_Enemy_1 is the fifth character slot ($FFFF4500), so
    // a party of four leaves the first enemy with *both* sides empty: the arm
    // fires, costs the parity draw, and hands the object whichever side it
    // picked — a party slot is not an enemy slot to refill, which is why the
    // left draw carries no neighbour.
    let record = one_arm(1, 6);
    let mut r = roster(&[(1, 0, [1; 4], [6; 4])]);
    seat_party(&mut r, 4);
    let (outcome, drawn) = scan(&mut r, 6, &record, &[0xFFFE]);
    assert_eq!(
        neighbour_of(outcome),
        None,
        "even keeps the left, a party slot"
    );
    assert_eq!(drawn, 1);
    let (outcome, drawn) = scan(&mut r, 6, &record, &[0xFFFF]);
    assert_eq!(neighbour_of(outcome), Some(id(7)), "odd keeps the right");
    assert_eq!(drawn, 1);

    // A fifth member fills that slot, leaving only the right side — which is
    // past `Obj_Fighters` ($FFFF4640, wiped and never written) and so reads
    // zero. One empty side costs no draw.
    seat_party(&mut r, 5);
    let (outcome, drawn) = scan(&mut r, 6, &record, &[]);
    assert_eq!(neighbour_of(outcome), Some(id(7)));
    assert_eq!(drawn, 0);
}

// --- arm $02: HalfHPOrLower ------------------------------------------------

#[test]
fn half_hp_or_lower_holds_at_exactly_half() {
    // `lsr.w #1` of 80 is 40, and `cmp.w curr_hp, d1 / bcs` holds at 40.
    let record = one_arm(2, 10);
    for (curr, fires) in [(40u16, true), (41, false), (1, true), (80, false)] {
        let mut r = roster(&[(1, 0, [2; 4], [10; 4])]);
        hp(&mut r, 6, curr, 80);
        let (outcome, drawn) = scan(&mut r, 6, &record, &[]);
        assert_eq!(
            outcome,
            if fires {
                AiOutcome::Replaced {
                    slot: 0,
                    neighbour: None,
                }
            } else {
                AiOutcome::Rolled
            },
            "80 max at {curr} HP"
        );
        assert_eq!(drawn, 0);
    }
}

#[test]
fn half_hp_or_lower_holds_at_exactly_half_of_an_odd_maximum() {
    // 81 >> 1 is 40, so 40 holds and 41 does not.
    let record = one_arm(2, 10);
    for (curr, fires) in [(40u16, true), (41, false)] {
        let mut r = roster(&[(1, 0, [2; 4], [10; 4])]);
        hp(&mut r, 6, curr, 81);
        let (outcome, _) = scan(&mut r, 6, &record, &[]);
        assert_eq!(
            matches!(outcome, AiOutcome::Replaced { .. }),
            fires,
            "{curr} HP"
        );
    }
}

// --- arm $03: WiredineExists ----------------------------------------------

#[test]
fn wiredine_exists_fires_for_wiredine_with_a_gap_two_slots_back() {
    // Wiredine in slot 2, and `-$80(a4)` — the fifth party slot for
    // Fighter_Enemy_2 — reading zero.
    let wiredine = one_arm(3, 12);
    let mut r = roster(&[(2, enemy_id::WIREDINE, [3; 4], [12; 4])]);
    seat_party(&mut r, 4);
    let (outcome, drawn) = scan(&mut r, 7, &wiredine, &[]);
    assert_eq!(ability_of(&wiredine, outcome, 0), 12);
    assert_eq!(drawn, 0);

    // A fifth member fills that slot: no fire.
    seat_party(&mut r, 5);
    assert_eq!(scan(&mut r, 7, &wiredine, &[]).0, AiOutcome::Rolled);

    // `$80(a4)` for Fighter_Enemy_1 is Fighter_Enemy_3, so Wiredine *itself*
    // holding the slot behind the first enemy closes the gap.
    let mut r = roster(&[
        (2, enemy_id::WIREDINE, [3; 4], [12; 4]),
        (3, 11, [0; 4], [0; 4]),
    ]);
    assert_eq!(scan(&mut r, 6, &wiredine, &[]).0, AiOutcome::Rolled);
    // And for Fighter_Enemy_3 the gap is Fighter_Enemy_1 — the slot to fill,
    // not the one to look past.
    let mut r = roster(&[(2, enemy_id::WIREDINE, [3; 4], [12; 4])]);
    let (outcome, _) = scan(&mut r, 8, &wiredine, &[]);
    assert!(matches!(outcome, AiOutcome::Replaced { .. }));
}

#[test]
fn wiredine_exists_needs_wiredine_in_slot_two() {
    let wiredine = one_arm(3, 12);
    for rows in [
        vec![(1u8, 11u16, [0; 4], [0; 4])],
        vec![(1, 11, [0; 4], [0; 4]), (2, 11, [3; 4], [12; 4])],
    ] {
        let mut r = roster(&rows);
        assert_eq!(
            scan(&mut r, 7, &wiredine, &[]).0,
            AiOutcome::Rolled,
            "slot 2 is not Wiredine: {rows:?}"
        );
    }
}

// --- arm $04: ArthroPodExists ---------------------------------------------

#[test]
fn arthro_pod_exists_fires_with_the_other_pod_slot_empty() {
    let record = one_arm(4, 13);
    let mut r = roster(&[(1, enemy_id::ARTHRO_POD, [4; 4], [13; 4])]);
    let (outcome, drawn) = scan(&mut r, 6, &record, &[]);
    assert_eq!(ability_of(&record, outcome, 0), 13);
    assert_eq!(drawn, 0);
    // Slot 3 instead of slot 1, with slot 1 empty.
    let mut r = roster(&[(3, enemy_id::ARTHRO_POD, [4; 4], [13; 4])]);
    let (outcome, _) = scan(&mut r, 8, &record, &[]);
    assert!(matches!(outcome, AiOutcome::Replaced { .. }));
}

#[test]
fn arthro_pod_exists_holds_off_between_two_pods() {
    let record = one_arm(4, 13);
    let mut r = roster(&[
        (1, enemy_id::ARTHRO_POD, [4; 4], [13; 4]),
        (3, 11, [0; 4], [0; 4]),
    ]);
    let (outcome, _) = scan(&mut r, 6, &record, &[]);
    assert_eq!(outcome, AiOutcome::Rolled);
}

// --- arm $05: CRayTubeNearSatMinion ---------------------------------------

#[test]
fn cray_tube_between_sat_minions_fires_without_stopping_the_scan() {
    let sat = one_arm(5, 21);
    let mut r = roster(&[
        (1, enemy_id::SAT_MINION, [5; 4], [21; 4]),
        (2, enemy_id::CRAY_TUBE, [5; 4], [21; 4]),
        (3, enemy_id::SAT_MINION, [5; 4], [21; 4]),
    ]);
    let (outcome, drawn) = scan(&mut r, 7, &sat, &[]);
    assert_eq!(ability_of(&sat, outcome, 0), 21);
    assert_eq!(drawn, 0);

    // The routine writes `d0`, so the scan keeps walking: with a *different*
    // arm in a later slot, that arm decides instead. `$02` HalfHPOrLower holds
    // for the actor at half its maximum.
    let mixed = record(0, [5, 2, 0, 0], [21, 40, 0, 0]);
    hp(&mut r, 7, 40, 80);
    let (outcome, drawn) = scan(&mut r, 7, &mixed, &[]);
    assert_eq!(
        ability_of(&mixed, outcome, 0),
        40,
        "slot 2's HalfHPOrLower is reached and holds"
    );
    assert_eq!(drawn, 0);
}

#[test]
fn cray_tube_arm_holds_off_when_the_formation_is_wrong() {
    let record = one_arm(5, 21);
    for rows in [
        vec![
            (1u8, enemy_id::SAT_MINION, [5; 4], [21; 4]),
            (2, enemy_id::SAT_MINION, [5; 4], [21; 4]),
            (3, enemy_id::SAT_MINION, [5; 4], [21; 4]),
        ],
        vec![
            (1, enemy_id::CRAY_TUBE, [5; 4], [21; 4]),
            (2, enemy_id::CRAY_TUBE, [5; 4], [21; 4]),
            (3, enemy_id::SAT_MINION, [5; 4], [21; 4]),
        ],
    ] {
        let mut r = roster(&rows);
        let (outcome, _) = scan(&mut r, 7, &record, &[]);
        assert_eq!(outcome, AiOutcome::Rolled);
    }
}

// --- arm $06: ZolSlugs ----------------------------------------------------

#[test]
fn zol_slugs_fires_for_exactly_two_of_them() {
    let record = one_arm(6, 18);
    let mut r = roster(&[
        (1, enemy_id::ZOL_SLUG, [6; 4], [18; 4]),
        (2, enemy_id::ZOL_SLUG, [6; 4], [18; 4]),
    ]);
    let (outcome, drawn) = scan(&mut r, 6, &record, &[]);
    assert_eq!(ability_of(&record, outcome, 0), 18);
    assert_eq!(drawn, 0);
}

#[test]
fn zol_slugs_holds_off_for_one_or_three() {
    let record = one_arm(6, 18);
    let mut r = roster(&[(1, enemy_id::ZOL_SLUG, [6; 4], [18; 4])]);
    assert_eq!(scan(&mut r, 6, &record, &[]).0, AiOutcome::Rolled, "one");

    let mut r = roster(&[
        (1, enemy_id::ZOL_SLUG, [6; 4], [18; 4]),
        (2, enemy_id::ZOL_SLUG, [6; 4], [18; 4]),
        (3, enemy_id::ZOL_SLUG, [6; 4], [18; 4]),
    ]);
    assert_eq!(scan(&mut r, 6, &record, &[]).0, AiOutcome::Rolled, "three");

    let mut r = roster(&[
        (1, enemy_id::ZOL_SLUG, [6; 4], [18; 4]),
        (2, 11, [0; 4], [0; 4]),
    ]);
    assert_eq!(
        scan(&mut r, 6, &record, &[]).0,
        AiOutcome::Rolled,
        "another id ends the count"
    );
}

// --- arms $07 / $08 / $0A / $0B: reaction_flags ---------------------------

/// Each of the four bit-testing arms fires on its own bit and clears the byte.
#[test]
fn the_received_arms_fire_on_their_bit_and_wipe_the_byte() {
    for (condition, id_, bit) in [
        (0x07u8, 0x07u8, reaction::PHYSICAL),
        (0x08, 0x08, reaction::MAGIC),
        (0x0A, 0x0A, reaction::TECHNIQUE),
        (0x0B, 0x0B, reaction::AMBUSH),
    ] {
        let record = one_arm(id_, 24);
        let mut r = roster(&[(1, 0, [id_; 4], [24; 4])]);
        r.get_mut(id(6)).expect("seated").reaction_flags = bit;
        let (outcome, drawn) = scan(&mut r, 6, &record, &[]);
        assert_eq!(ability_of(&record, outcome, 0), 24, "{condition:#04X}");
        assert_eq!(drawn, 0);
        assert_eq!(
            r.get(id(6)).expect("seated").reaction_flags,
            0,
            "{condition:#04X} clears the whole byte"
        );

        // The same record with only *another* bit set does not fire.
        r.get_mut(id(6)).expect("seated").reaction_flags = !bit;
        let (outcome, _) = scan(&mut r, 6, &record, &[]);
        assert_eq!(outcome, AiOutcome::Rolled, "{condition:#04X} holds off");
        assert_ne!(r.get(id(6)).expect("seated").reaction_flags, 0);
    }
}

// --- arm $09: Alone -------------------------------------------------------

#[test]
fn alone_fires_for_the_last_enemy_standing() {
    let record = one_arm(9, 65);
    let mut r = roster(&[(1, 0, [9; 4], [65; 4]), (2, 0, [9; 4], [65; 4])]);
    let fighter = r.get_mut(id(7)).expect("seated");
    fighter.stats.curr_hp = 0;
    fighter.stats.status = status::DEAD;
    let (outcome, drawn) = scan(&mut r, 6, &record, &[]);
    assert_eq!(ability_of(&record, outcome, 0), 65);
    assert_eq!(drawn, 0);
}

#[test]
fn alone_holds_off_while_two_enemies_stand() {
    let record = one_arm(9, 65);
    let mut r = roster(&[(1, 0, [9; 4], [65; 4]), (2, 0, [9; 4], [65; 4])]);
    assert_eq!(scan(&mut r, 6, &record, &[]).0, AiOutcome::Rolled);
    // A dormant neighbour is not an object: `EnemyInit_Igglanova` clears it.
    r.get_mut(id(7)).expect("seated").active = false;
    let (outcome, _) = scan(&mut r, 6, &record, &[]);
    assert!(matches!(outcome, AiOutcome::Replaced { .. }));
}

// --- arm $0C: TechSealed --------------------------------------------------

#[test]
fn tech_sealed_fires_on_the_actors_own_seal() {
    let record = one_arm(0x0C, 48);
    let mut r = roster(&[(1, 0, [0x0C; 4], [48; 4])]);
    r.get_mut(id(6)).expect("seated").stats.status = status::TECH_SEALED;
    let (outcome, drawn) = scan(&mut r, 6, &record, &[]);
    assert_eq!(ability_of(&record, outcome, 0), 48);
    assert_eq!(drawn, 0);
}

#[test]
fn tech_sealed_holds_off_for_every_other_status() {
    let record = one_arm(0x0C, 48);
    for status_byte in [status::POISONED, status::ASLEEP, status::DEAD] {
        let mut r = roster(&[(1, 0, [0x0C; 4], [48; 4])]);
        r.get_mut(id(6)).expect("seated").stats.status = status_byte;
        assert_eq!(
            scan(&mut r, 6, &record, &[]).0,
            AiOutcome::Rolled,
            "{status_byte:#04X}"
        );
    }
}

// --- arms $0D / $0E: the partner pair -------------------------------------

#[test]
fn each_partner_arm_fires_for_the_last_of_its_pair() {
    // 84 BladeRight names $0D, 86 HakenLeft names $0E; each fires when the
    // *other* id is gone, and neither fires while a second of its own stands.
    let blade = one_arm(0x0D, 58);
    let mut r = roster(&[(2, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4])]);
    let (outcome, drawn) = scan(&mut r, 7, &blade, &[]);
    assert_eq!(ability_of(&blade, outcome, 0), 58);
    assert_eq!(drawn, 0);

    let haken = one_arm(0x0E, 59);
    let mut r = roster(&[(2, enemy_id::HAKEN_LEFT, [0x0E; 4], [59; 4])]);
    let (outcome, _) = scan(&mut r, 7, &haken, &[]);
    assert_eq!(ability_of(&haken, outcome, 0), 59);
}

#[test]
fn the_partner_arms_hold_off_while_the_pair_stands() {
    let blade = one_arm(0x0D, 58);
    for rows in [
        // The partner is present.
        vec![
            (1u8, enemy_id::HAKEN_LEFT, [0x0E; 4], [59; 4]),
            (2, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4]),
        ],
        // A second BladeRight keeps the first from firing.
        vec![
            (2, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4]),
            (3, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4]),
        ],
    ] {
        let mut r = roster(&rows);
        assert_eq!(scan(&mut r, 7, &blade, &[]).0, AiOutcome::Rolled);
    }
    // The actor's own slot is skipped: a lone BladeRight *is* the last one.
    let mut r = roster(&[(1, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4])]);
    assert!(matches!(
        scan(&mut r, 6, &blade, &[]).0,
        AiOutcome::Replaced { .. }
    ));
}

// --- arm $0F: HalfHPOrLower_AllEnemies ------------------------------------

#[test]
fn any_enemy_at_half_hp_fires_the_all_enemies_arm() {
    let record = one_arm(0x0F, 69);
    let mut r = roster(&[(1, 0, [0x0F; 4], [69; 4]), (2, 0, [0x0F; 4], [69; 4])]);
    hp(&mut r, 6, 80, 80);
    hp(&mut r, 7, 40, 80);
    let (outcome, drawn) = scan(&mut r, 6, &record, &[]);
    assert_eq!(ability_of(&record, outcome, 0), 69);
    assert_eq!(drawn, 0, "the arm is a pure read");
}

#[test]
fn the_all_enemies_arm_holds_off_above_half() {
    let record = one_arm(0x0F, 69);
    let mut r = roster(&[(1, 0, [0x0F; 4], [69; 4]), (2, 0, [0x0F; 4], [69; 4])]);
    hp(&mut r, 6, 80, 80);
    hp(&mut r, 7, 41, 80);
    assert_eq!(scan(&mut r, 6, &record, &[]).0, AiOutcome::Rolled);
    // At exactly half it fires — the boundary is inclusive for every enemy.
    hp(&mut r, 7, 40, 80);
    assert!(matches!(
        scan(&mut r, 6, &record, &[]).0,
        AiOutcome::Replaced { .. }
    ));
}

// --- arm $12: ThreeXeAThouls ---------------------------------------------

#[test]
fn three_xe_a_thouls_fires_and_clears_the_multi_target_bits() {
    let record = one_arm(0x12, 92);
    let mut r = roster(&[
        (1, enemy_id::XE_A_THOUL, [0x12; 4], [92; 4]),
        (2, enemy_id::XE_A_THOUL, [0x12; 4], [92; 4]),
        (3, enemy_id::XE_A_THOUL, [0x12; 4], [92; 4]),
    ]);
    for slot in 6..=8u8 {
        r.get_mut(id(slot)).expect("seated").reaction_flags = reaction::MULTI_TARGET;
    }
    let (outcome, drawn) = scan(&mut r, 6, &record, &[]);
    assert_eq!(ability_of(&record, outcome, 0), 92);
    assert_eq!(drawn, 0);
    for slot in 6..=8u8 {
        assert_eq!(
            r.get(id(slot)).expect("seated").reaction_flags,
            0,
            "slot {slot}'s bit 4"
        );
    }
}

#[test]
fn three_xe_a_thouls_holds_off_without_the_multi_target_bit() {
    let record = one_arm(0x12, 92);
    let mut r = roster(&[
        (1, enemy_id::XE_A_THOUL, [0x12; 4], [92; 4]),
        (2, enemy_id::XE_A_THOUL, [0x12; 4], [92; 4]),
        (3, enemy_id::XE_A_THOUL, [0x12; 4], [92; 4]),
    ]);
    assert_eq!(scan(&mut r, 6, &record, &[]).0, AiOutcome::Rolled);
    // Bit 4 set but one slot is not a XeAThoul.
    r.get_mut(id(8)).expect("seated").reaction_flags = reaction::MULTI_TARGET;
    r.get_mut(id(8)).expect("seated").stats.enemy_id = 11;
    assert_eq!(scan(&mut r, 6, &record, &[]).0, AiOutcome::Rolled);
}

// --- the unsupported arms -------------------------------------------------

#[test]
fn the_two_arms_without_a_writable_fact_are_explicitly_unsupported() {
    // $10 reads $FFFFEEA4 bit 0, which nothing in the disassembly writes; $11
    // reads $FFFFEE86, set by Lashiec's own battle object.
    for (condition, ability) in [(0x10u8, 83u8), (0x11, 98)] {
        let record = one_arm(condition, ability);
        let mut r = roster(&[(1, 128, [condition; 4], [ability; 4])]);
        let (outcome, drawn) = scan(&mut r, 6, &record, &[]);
        assert_eq!(
            outcome,
            AiOutcome::Unsupported {
                condition: EnemyAiCondition::from_id(condition).expect("a table entry")
            },
            "{condition:#04X}"
        );
        assert_eq!(
            outcome.unreported_ability(&record),
            Some(ability),
            "the timeline names the ability the arm would have written"
        );
        assert_eq!(drawn, 0);
    }
}

#[test]
fn an_unsupported_arm_stops_the_scan_before_a_later_one() {
    // 17 HP25PercentOrLower in slot 0 and a HalfHPOrLower in slot 1: the port
    // cannot tell whether the first held, so nothing after it may decide.
    let mixed = record(0, [0x11, 2, 0, 0], [98, 40, 0, 0]);
    let mut r = roster(&[(1, 128, [0x11, 2, 0, 0], [98, 40, 0, 0])]);
    hp(&mut r, 6, 1, 100);
    let (outcome, _) = scan(&mut r, 6, &mixed, &[]);
    assert!(matches!(outcome, AiOutcome::Unsupported { .. }));
}

// --- the scan itself ------------------------------------------------------

#[test]
fn the_scan_stops_at_the_first_arm_that_holds() {
    // Slot 0 is `$09` Alone (two enemies: no fire), slot 1 is `$02` at half HP.
    let mixed = record(0, [9, 2, 0, 0], [65, 40, 0, 0]);
    let mut r = roster(&[
        (1, 0, [9, 2, 0, 0], [65, 40, 0, 0]),
        (2, 0, [9, 2, 0, 0], [65, 40, 0, 0]),
    ]);
    hp(&mut r, 6, 10, 80);
    let (outcome, _) = scan(&mut r, 6, &mixed, &[]);
    assert_eq!(
        outcome,
        AiOutcome::Replaced {
            slot: 1,
            neighbour: None
        }
    );
    assert_eq!(ability_of(&mixed, outcome, 0), 40);
}

#[test]
fn a_zero_condition_ends_the_scan_before_the_table() {
    let record = record(0, [0, 2, 2, 2], [0, 40, 40, 40]);
    let mut r = roster(&[(1, 0, [0, 2, 2, 2], [0, 40, 40, 40])]);
    hp(&mut r, 6, 1, 100);
    let (outcome, drawn) = scan(&mut r, 6, &record, &[]);
    assert_eq!(outcome, AiOutcome::Rolled);
    assert_eq!(drawn, 0);
}

#[test]
fn an_all_zero_record_is_the_rolled_ability() {
    let record = record(0, [0; 4], [0; 4]);
    let mut r = roster(&[(1, 0, [0; 4], [0; 4])]);
    let (outcome, drawn) = scan(&mut r, 6, &record, &[]);
    assert_eq!(outcome, AiOutcome::Rolled);
    assert_eq!(drawn, 0);
    assert_eq!(outcome.unreported_ability(&record), None);
}
