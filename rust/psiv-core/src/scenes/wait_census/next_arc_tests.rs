//! The next_arc_followup.rs census's negative controls: an omitted claim or
//! row of each kind it added, a wrong loop classification, a bare frame
//! claimed by an op that may not carry it, and a wrong count (#470).

use super::Claim::{Wait, Within};
use super::Retail::{Facing, Frames, Main, Map};
use super::*;

const RIKA: &str = "Cutscene_MeetingRika";
const GIRLS: &str = "Event_GirlsSneakingOut";

fn without(keep: &dyn Fn(&Row) -> bool) -> Vec<Row> {
    ROWS.iter().copied().filter(|row| keep(row)).collect()
}

fn with(change: &dyn Fn(&mut Row)) -> Vec<Row> {
    let mut rows = ROWS.to_vec();
    rows.iter_mut().for_each(change);
    rows
}

#[test]
fn the_next_arc_claims_reject_an_omission_and_a_misclaim() {
    for (scene, index) in [(RIKA, 50), (RIKA, 59), ("Event_ChazHouse", 8), (GIRLS, 11)] {
        let omitted = without(&|row| !(row.scene == scene && row.claim == Wait(index)));
        assert_eq!(
            claim_problems(&omitted),
            [format!("{scene} op {index}: a Wait with 0 claims")]
        );
    }

    // The 60 bare frames' `Wait` without the defect that says it runs map
    // updates the loop does not.
    let unflagged = with(&|row| {
        if row.scene == RIKA && row.claim == Wait(59) {
            row.defect = None;
            row.exact = false;
        }
    });
    assert_eq!(
        claim_problems(&unflagged),
        [format!(
            "{RIKA} op 59: a Wait for a loop with no map updates, without a defect"
        )]
    );

    // A bare frame may be waited out by a `WaitFrames`, but not when it
    // follows `Field_UpdateObjects`, and not by an op that waits nothing.
    let objects = with(&|row| {
        if row.scene == RIKA && row.claim == Within(13) {
            row.retail = frame(0x07_4652, true);
        }
    });
    assert_eq!(
        claim_problems(&objects),
        [format!("{RIKA} op 13: Within an own loop")]
    );
    let misclaimed = with(&|row| {
        if row.scene == RIKA && row.claim == Within(91) {
            row.claim = Within(90);
        }
    });
    assert_eq!(
        claim_problems(&misclaimed),
        [
            format!(
                "{RIKA} op 90: Within names an op that waits no loop, {:?}",
                scene(RIKA).ops.get(90)
            ),
            format!("{RIKA} op 90: Within an own loop"),
        ]
    );
}

#[test]
fn the_next_arc_rows_reject_a_wrong_loop_or_count() {
    let Some(rom) = rom() else {
        return;
    };
    // `DoMapUpdateLoop` called `DoMainUpdatesLoop`, a frame after
    // `Field_BuildSprites` said to follow `Field_UpdateObjects`, the bare
    // loop said to update objects, and two `VInt_PrepareLoop` claims swapped.
    let wrong = with(&|row| {
        if row.scene == RIKA && row.claim == Wait(50) {
            row.retail = Main(0x07_47DE);
        }
        if row.scene == RIKA && row.claim == Within(13) {
            row.retail = frame(0x07_4652, true);
        }
        if row.scene == GIRLS && row.claim == Within(0) {
            row.retail = own(0x06_D4A8, true, false);
        }
        if row.scene == RIKA && row.claim == Within(95) {
            row.retail = Frames(0x07_4A1A);
        }
    });
    assert_eq!(
        retail_problems(rom, &wrong),
        [
            format!("{GIRLS} $06D4A8: objects is not true"),
            format!("{RIKA} $074652: objects is not true"),
            format!("{RIKA} $0747DE: not a jsr to $05A73C"),
            format!("{RIKA} $074A1A: claims 10 frames, the loop runs 60"),
        ]
    );

    // The 60-pass loop's claim swapped with the 30-frame `DoMapUpdateLoop`'s:
    // its `Wait` names a defect but stays counted.
    let swapped = with(&|row| {
        if row.scene == RIKA && row.claim == Wait(59) {
            row.claim = Wait(53);
        } else if row.scene == RIKA && row.claim == Wait(53) {
            row.claim = Wait(59);
        }
    });
    assert_eq!(
        retail_problems(rom, &swapped),
        [
            format!("{RIKA} $074802: claims 60 frames, the loop runs 30"),
            format!("{RIKA} $074836: claims 30 frames, the loop runs 60"),
        ]
    );
}

#[test]
fn the_next_arc_coverage_rejects_an_omitted_row() {
    let Some(rom) = rom() else {
        return;
    };
    for (scene, retail) in [
        (GIRLS, frame(0x06_D4E2, false)),
        (GIRLS, Map(0x06_D52E)),
        ("Event_ChazHouse", Map(0x06_FAA2)),
        (RIKA, Facing(0x07_47BE)),
        (RIKA, frame(0x07_49DE, false)),
        (RIKA, Frames(0x07_4722)),
        (RIKA, Map(0x07_498E)),
    ] {
        let omitted = without(&|row| !(row.scene == scene && row.retail == retail));
        assert_eq!(
            coverage_problems(rom, &omitted),
            [format!(
                "{scene} ${:06X}: {retail:?} has no row",
                retail_at(retail)
            )]
        );
    }
    // Without their rows the two bare loops are no longer known as loops,
    // and each `VInt_Prepare` is a bare frame with no row.
    for (scene, at) in [(GIRLS, 0x06_D4A8), (RIKA, 0x07_4836)] {
        let omitted = without(&|row| !(row.scene == scene && row.retail == own(at, false, false)));
        assert_eq!(
            coverage_problems(rom, &omitted),
            [format!(
                "{scene} ${at:06X}: {:?} has no row",
                frame(at, false)
            )]
        );
    }
}
