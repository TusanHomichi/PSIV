//! `Cutscene_InsideSpaceship`'s census controls: its shared code left out or
//! cut short, a library loop in it without a row, and a wait claimed twice or
//! merged into the wrong one (#470, #522).

use super::Claim::{Merged, Wait};
use super::Retail::Map;
use super::*;

fn with(change: &dyn Fn(&mut Row)) -> Vec<Row> {
    let mut rows = ROWS.to_vec();
    rows.iter_mut().for_each(change);
    rows
}

fn ship_rows() -> Vec<Row> {
    ROWS.iter()
        .copied()
        .filter(|row| row.scene == SHIP)
        .collect()
}

#[test]
fn the_ships_eight_waits_are_claimed_through_its_shared_code() {
    let waits: Vec<usize> = censused_ops(SHIP)
        .iter()
        .enumerate()
        .filter(|(_, op)| matches!(op, SceneOp::Wait { .. }))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(waits, [9, 11, 13, 15, 21, 24, 33, 39]);
    let (start, end) = (SHARED_CODE[0].1, SHARED_CODE[0].2);
    for row in ship_rows() {
        assert!((start..end).contains(&retail_at(row.retail)), "{row:?}");
    }
}

#[test]
fn the_ships_claims_reject_a_duplicate_and_a_wrong_merge() {
    // The second flight's map wait claimed as a `Wait` too: two claims.
    let duplicate = with(&|row| {
        if row.scene == SHIP && row.claim == Merged(24) {
            row.claim = Wait(24);
        }
    });
    assert_eq!(
        claim_problems(&duplicate),
        [format!("{SHIP} op 24: a Wait with 2 claims")]
    );

    // A merged copy whose `Wait` row is gone, or that names that row's own
    // wait, or a different kind of loop.
    let orphan = with(&|row| {
        if row.scene == SHIP && row.claim == Wait(33) {
            row.claim = Merged(33);
        }
    });
    assert_eq!(
        claim_problems(&orphan),
        [
            format!("{SHIP} op 33: a Wait with 0 claims"),
            format!("{SHIP} op 33: merged into a Wait no row claims"),
            format!("{SHIP} op 33: merged into a Wait no row claims"),
        ]
    );
    let same = with(&|row| {
        if row.scene == SHIP && row.claim == Merged(39) {
            row.retail = own(0x06_4800, true, true);
        }
    });
    assert_eq!(
        claim_problems(&same),
        [format!(
            "{SHIP} op 39: merged with the retail wait its Wait row names"
        )]
    );
    let kind = with(&|row| {
        if row.scene == SHIP && row.claim == Merged(39) {
            row.retail = Map(0x06_4A76);
        }
    });
    assert_eq!(
        claim_problems(&kind),
        [format!(
            "{SHIP} op 39: merged {:?} into a Wait for {:?}",
            Map(0x06_4A76),
            own(0x06_4800, true, true)
        )]
    );
    let not_a_wait = with(&|row| {
        if row.scene == SHIP && row.claim == Merged(24) {
            row.claim = Merged(23);
        }
    });
    assert_eq!(
        claim_problems(&not_a_wait),
        [
            format!("{SHIP} op 23: merged into a Wait, is Some(WaitFrames {{ frames: 1 }})"),
            format!("{SHIP} op 23: merged into a Wait no row claims"),
        ]
    );
}

#[test]
fn the_ships_shared_code_rejects_an_omission() {
    // Without its shared code, every ship row stands outside the bytes the
    // scene is censused over.
    let others: Vec<Shared> = SHARED_CODE
        .iter()
        .copied()
        .filter(|(scene, ..)| *scene != SHIP)
        .collect();
    let expected: Vec<String> = ship_rows()
        .iter()
        .map(|row| {
            format!(
                "{SHIP} ${:06X}: outside the scene's censused bytes",
                retail_at(row.retail)
            )
        })
        .collect();
    assert_eq!(placement_problems(ROWS, &others), expected);

    let Some(rom) = rom() else {
        return;
    };
    // Cut short before the planet screen (`loc_64C1E`): both flights' `bsr`
    // to it leave the span.
    let short: &[Shared] = &[(SHIP, 0x06_3BC4, 0x06_4B02)];
    assert_eq!(
        shared_code_problems(rom, short),
        [
            format!("{SHIP} $06475C: bsr to $064C1E, outside its censused bytes"),
            format!("{SHIP} $0649D2: bsr to $064C1E, outside its censused bytes"),
        ]
    );
    // Started past the menu: its dispatch's `jmp` no longer reaches it, and
    // the flights' ten `bsr`s to the ship art and planet routines before
    // `loc_64568` leave the span.
    let late: &[Shared] = &[(SHIP, 0x06_4568, 0x06_4D00)];
    let problems = shared_code_problems(rom, late);
    assert_eq!(
        problems[0],
        format!("{SHIP} $064568: not reached from its other bytes")
    );
    assert_eq!(problems.len(), 11, "{problems:#?}");
    assert!(
        problems[1..]
            .iter()
            .all(|problem| problem.contains(": bsr to $064")
                && problem.ends_with("outside its censused bytes")),
        "{problems:#?}"
    );

    // A library loop in the shared code with no row: the second flight's
    // first map wait.
    let omitted: Vec<Row> = ROWS
        .iter()
        .copied()
        .filter(|row| !(row.scene == SHIP && row.retail == Map(0x06_49C6)))
        .collect();
    assert_eq!(
        coverage_problems(rom, &omitted),
        [format!("{SHIP} $0649C6: {:?} has no row", Map(0x06_49C6))]
    );
    // And the same loop unread when the shared code is left out.
    assert!(
        coverage_problems_over(rom, &omitted, &others).is_empty(),
        "without the span nothing reads the flights"
    );
}

#[test]
fn the_ships_rows_are_its_retail_loops() {
    let Some(rom) = rom() else {
        return;
    };
    // A merged copy is held to its own loop's count: the second flight's
    // 60-frame map wait merged into the 257-frame descent.
    let miscounted = with(&|row| {
        if row.scene == SHIP && row.claim == Merged(24) {
            row.claim = Merged(39);
        }
    });
    assert_eq!(
        retail_problems(rom, &miscounted),
        [format!(
            "{SHIP} $0649C6: claims 257 frames, the loop runs 60"
        )]
    );
    let wrong = with(&|row| {
        if row.scene == SHIP && row.claim == Wait(21) {
            row.retail = own(0x06_4908, false, true);
        }
    });
    assert_eq!(
        retail_problems(rom, &wrong),
        [format!("{SHIP} $064908: objects is not false")]
    );
}

/// The bound of a takeoff loop that opens `clr.w (Joypad_Held).w` /
/// `cmpi.w #N, d0` and counts `d0` up from zero: `N` passes, as `loc_645E6`
/// (`ps4.asm:134199-134201`) and `loc_64908` (`:134408-134410`) do. The
/// census reads loop counts from a `dbf`, which these loops lack.
fn counted_up_passes(rom: &[u8], at: usize) -> Option<u32> {
    let word = |offset: usize| u16::from_be_bytes([rom[at + offset], rom[at + offset + 1]]);
    (word(0) == 0x4278 && word(4) == 0x0C40).then(|| u32::from(word(6)))
}

/// What the ship's `Wait` rows for one takeoff loop claim, against its passes.
fn takeoff_sums(rom: &[u8], rows: &[Row]) -> Vec<(u32, u32, Option<u32>)> {
    [0x06_45E6, 0x06_4908]
        .into_iter()
        .map(|at| {
            let claimed = rows
                .iter()
                .filter(|row| row.scene == SHIP && row.retail == own(at, true, true))
                .filter_map(claimed_ticks)
                .sum();
            (at, claimed, counted_up_passes(rom, at as usize))
        })
        .collect()
}

/// `loc_645E6`'s waits, split at its three propulsion requests (#629), add
/// up to its `$1A3` passes, and `loc_64908`'s wait to its `$E4`.
#[test]
fn the_takeoff_rows_sum_to_each_loops_passes() {
    let Some(rom) = rom() else {
        return;
    };
    assert_eq!(
        takeoff_sums(rom, ROWS),
        [
            (0x06_45E6, 0x1A3, Some(0x1A3)),
            (0x06_4908, 0xE4, Some(0xE4))
        ]
    );
    // A split wait without its row, and one claimed for the other loop.
    let dropped: Vec<Row> = ROWS
        .iter()
        .copied()
        .filter(|row| !(row.scene == SHIP && row.claim == Wait(13)))
        .collect();
    assert_eq!(takeoff_sums(rom, &dropped)[0].1, 0x1A2);
    let moved = with(&|row| {
        if row.scene == SHIP && row.claim == Wait(15) {
            row.retail = own(0x06_4908, true, true);
        }
    });
    assert_eq!(
        takeoff_sums(rom, &moved)
            .iter()
            .map(|&(_, claimed, _)| claimed)
            .collect::<Vec<_>>(),
        [0x1A3 - 192, 0xE4 + 192]
    );
    // Not a counted-up loop: the descent opens differently.
    assert_eq!(counted_up_passes(rom, 0x06_4800), None);
}
