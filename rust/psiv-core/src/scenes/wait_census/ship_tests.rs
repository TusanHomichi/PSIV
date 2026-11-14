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
fn the_ships_six_waits_are_claimed_through_its_shared_code() {
    let waits: Vec<usize> = censused_ops(SHIP)
        .iter()
        .enumerate()
        .filter(|(_, op)| matches!(op, SceneOp::Wait { .. }))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(waits, [9, 11, 17, 21, 31, 37]);
    let (start, end) = (SHARED_CODE[0].1, SHARED_CODE[0].2);
    for row in ship_rows() {
        assert!((start..end).contains(&retail_at(row.retail)), "{row:?}");
    }
}

#[test]
fn the_ships_claims_reject_a_duplicate_and_a_wrong_merge() {
    // The second flight's map wait claimed as a `Wait` too: two claims.
    let duplicate = with(&|row| {
        if row.scene == SHIP && row.claim == Merged(21) {
            row.claim = Wait(21);
        }
    });
    assert_eq!(
        claim_problems(&duplicate),
        [format!("{SHIP} op 21: a Wait with 2 claims")]
    );

    // A merged copy whose `Wait` row is gone, or that names that row's own
    // wait, or a different kind of loop.
    let orphan = with(&|row| {
        if row.scene == SHIP && row.claim == Wait(31) {
            row.claim = Merged(31);
        }
    });
    assert_eq!(
        claim_problems(&orphan),
        [
            format!("{SHIP} op 31: a Wait with 0 claims"),
            format!("{SHIP} op 31: merged into a Wait no row claims"),
            format!("{SHIP} op 31: merged into a Wait no row claims"),
        ]
    );
    let same = with(&|row| {
        if row.scene == SHIP && row.claim == Merged(37) {
            row.retail = own(0x06_4800, true, true);
        }
    });
    assert_eq!(
        claim_problems(&same),
        [format!(
            "{SHIP} op 37: merged with the retail wait its Wait row names"
        )]
    );
    let kind = with(&|row| {
        if row.scene == SHIP && row.claim == Merged(37) {
            row.retail = Map(0x06_4A76);
        }
    });
    assert_eq!(
        claim_problems(&kind),
        [format!(
            "{SHIP} op 37: merged {:?} into a Wait for {:?}",
            Map(0x06_4A76),
            own(0x06_4800, true, true)
        )]
    );
    let not_a_wait = with(&|row| {
        if row.scene == SHIP && row.claim == Merged(21) {
            row.claim = Merged(20);
        }
    });
    assert_eq!(
        claim_problems(&not_a_wait),
        [
            format!("{SHIP} op 20: merged into a Wait, is Some(WaitFrames {{ frames: 1 }})"),
            format!("{SHIP} op 20: merged into a Wait no row claims"),
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
        if row.scene == SHIP && row.claim == Merged(21) {
            row.claim = Merged(37);
        }
    });
    assert_eq!(
        retail_problems(rom, &miscounted),
        [format!(
            "{SHIP} $0649C6: claims 257 frames, the loop runs 60"
        )]
    );
    let wrong = with(&|row| {
        if row.scene == SHIP && row.claim == Wait(17) {
            row.retail = own(0x06_4908, false, true);
        }
    });
    assert_eq!(
        retail_problems(rom, &wrong),
        [format!("{SHIP} $064908: objects is not false")]
    );
}
