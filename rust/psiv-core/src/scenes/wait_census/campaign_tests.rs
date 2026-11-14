//! The dezo_campaign.rs census's own checks: the Le Roof loops against their
//! retail helper bytes, and the negative controls for the claims, counts and
//! coverage rows that census added (#470, #591).

use super::Claim::{Wait, Within};
use super::Retail::{self, Facing, Frames, Map};
use super::*;

/// The target of a `bsr.w` or `jsr (abs).l` at `at`.
fn call_target(rom: &[u8], at: usize) -> Option<u32> {
    if rom.get(at..at + 2)? == [0x61, 0x00] {
        let disp = i16::from_be_bytes([rom[at + 2], rom[at + 3]]);
        return Some((at as i64 + 2 + i64::from(disp)) as u32);
    }
    jsr_target(rom, at)
}

/// `loc_70638`, the Le Roof colour cycle.
const LE_ROOF_CYCLE: u32 = 0x07_0638;
/// `loc_705D6`, the Le Roof fade-in.
const LE_ROOF_FADE_IN: u32 = 0x07_05D6;

/// Every Le Roof own loop is the retail helper its op says, with the op's
/// literals: a colour loop's pass is `move.l d0,-(sp)`, a call of
/// `loc_70638`, `move.l (sp)+,d0`, `jsr VInt_Prepare` and its `dbf d0`, and
/// `loc_705D6`'s is its blacking, its ramp, `bsr loc_70638` and
/// `VInt_Prepare` under `dbf d7`. The census's own count check holds the
/// passes (180, 71, 120, 120; 180, 71, 120).
#[test]
fn the_le_roof_loops_are_the_retail_palette_helpers() {
    let Some(rom) = rom() else {
        return;
    };
    let word = |at: usize| u16::from_be_bytes([rom[at], rom[at + 1]]);
    let long = |at: usize| u32::from_be_bytes(rom[at..at + 4].try_into().unwrap());
    let cycle_bytes = |cycle: crate::CounterCycle| {
        let at = LE_ROOF_CYCLE as usize;
        // lea Palette_Table_Buffer,a1 / lea table,a2 / move.w
        // Main_Frame_Count,d0 / andi.w #3,d0 / bne / cmpi.b #entries,$ECF3.
        assert_eq!(word(at), 0x43F8);
        assert_eq!(word(at + 2), 0xFB00);
        assert_eq!(word(at + 4), 0x45F9);
        assert_eq!(long(at + 6), cycle.source_rom_addr);
        assert_eq!([word(at + 14), word(at + 16)], [0x0240, 3]);
        assert_eq!(word(at + 20), 0x0C38);
        assert_eq!(word(at + 22), u16::from(cycle.entries));
        assert_eq!(word(at + 24), 0xECF3);
        // move.w (a2,d0.w),$off(a1) / move.w $16(a2,d0.w),$off+2(a1).
        let first = at + 0x28;
        assert_eq!([word(first), word(first + 2)], [0x3372, 0x0000]);
        assert_eq!(word(first + 4), cycle.offset * 2);
        assert_eq!(word(first + 6), 0x3372);
        assert_eq!(word(first + 8), u16::from(cycle.entries) * 2);
        assert_eq!(word(first + 10), cycle.offset * 2 + 2);
        // addq.b #1,$ECF3.
        assert_eq!([word(first + 12), word(first + 14)], [0x5238, 0xECF3]);
    };
    let mut seen = 0;
    for row in ROWS.iter().filter(|row| {
        matches!(row.scene, "Event_MeetingLeRoof" | "Cutscene_LeRoofAgain")
            && matches!(row.retail, Retail::Own { .. })
    }) {
        let Within(index) = row.claim else {
            panic!("{row:?}: an own loop the census does not hand to an op");
        };
        let at = retail_at(row.retail) as usize;
        let end = loop_end(rom, at).expect("the loop's dbf");
        let SceneOp::Presentation { op } = scene(row.scene).ops[index] else {
            panic!("{row:?}: not a palette op");
        };
        match op {
            crate::PresentationOp::CounterPaletteCycle { cycle, .. } => {
                assert_eq!(word(at), 0x2F00, "{row:?}: move.l d0,-(sp)");
                assert_eq!(call_target(rom, at + 2), Some(LE_ROOF_CYCLE), "{row:?}");
                let after = at + if word(at + 2) == 0x6100 { 6 } else { 8 };
                assert_eq!(word(after), 0x201F, "{row:?}: move.l (sp)+,d0");
                assert_eq!(jsr_target(rom, after + 2), Some(VINT_PREPARE), "{row:?}");
                assert_eq!(end, after + 8, "{row:?}: then dbf");
                assert_eq!(word(end), 0x51C8, "{row:?}: dbf d0");
                cycle_bytes(cycle);
            }
            crate::PresentationOp::PaletteGrowIn {
                source_rom_addr,
                offset,
                words,
                passes,
                cycle,
            } => {
                assert_eq!(at, LE_ROOF_FADE_IN as usize + 6);
                // moveq #passes-1,d7 / lea Palette_Table_Buffer+offset,a1 /
                // moveq #words-1,d1.
                assert_eq!(word(at - 2), 0x7E00 | (passes - 1));
                assert_eq!([word(at), word(at + 2)], [0x43F8, 0xFB00 + offset * 2]);
                assert_eq!(word(at + 4), 0x7200 | u16::from(words - 1));
                // lea table,a0 / the same destination / moveq #words-1,d1.
                assert_eq!(word(at + 12), 0x41F9);
                assert_eq!(long(at + 14), source_rom_addr);
                assert_eq!(
                    [word(at + 18), word(at + 20)],
                    [0x43F8, 0xFB00 + offset * 2]
                );
                assert_eq!(word(at + 22), 0x7200 | u16::from(words - 1));
                // andi.w #7,d6: d0 steps every eighth count.
                assert_eq!([word(at + 0x2A), word(at + 0x2C)], [0x0246, 7]);
                assert_eq!(call_target(rom, at + 0x36), Some(LE_ROOF_CYCLE));
                assert_eq!(jsr_target(rom, at + 0x3E), Some(VINT_PREPARE));
                assert_eq!(word(end), 0x51CF, "dbf d7");
                assert_eq!(end, at + 0x44);
                assert_eq!(word(end + 4), 0x4E75, "rts");
                cycle_bytes(cycle);
            }
            other => panic!("{row:?}: {other:?}"),
        }
        assert!(!calls_in(rom, at, end, RUN_MAP_UPDATES), "{row:?}");
        assert!(!calls_in(rom, at, end, UPDATE_OBJECTS), "{row:?}");
        seen += 1;
    }
    assert_eq!(seen, 7, "four loops in one scene, three in the other");
    // `Cutscene_LeRoofAgain` calls the fade-in at `$078406`.
    assert_eq!(jsr_target(rom, 0x07_8406), Some(LE_ROOF_FADE_IN));
}

/// The dezo_campaign.rs census rejects an omitted row of each kind it
/// added (a `Wait`'s claim, a bare frame, a `VInt_PrepareLoop`), a frame or
/// own loop claimed by a `Wait` with no defect, a claim on the ship's route,
/// and own-loop passes that do not add up.
#[test]
fn the_campaign_checks_reject_an_omission_and_a_wrong_count() {
    let without = |keep: &dyn Fn(&Row) -> bool| -> Vec<Row> {
        ROWS.iter().copied().filter(|row| keep(row)).collect()
    };
    let lutz = "Cutscene_LutzRevelation";
    let le_roof = "Event_MeetingLeRoof";
    let again = "Cutscene_LeRoofAgain";

    let omitted = without(&|row| !(row.scene == lutz && row.claim == Wait(12)));
    assert_eq!(
        claim_problems(&omitted),
        [format!("{lutz} op 12: a Wait with 0 claims")]
    );

    let mut unflagged = ROWS.to_vec();
    for row in &mut unflagged {
        if row.scene == "Event_EclipseTorchUsed" && row.claim == Wait(5) {
            row.defect = None;
            row.exact = false;
        }
        // The colour loop's op made to stand for a loop that updates the map.
        if row.scene == le_roof && row.claim == Within(11) {
            row.retail = own(0x07_0538, false, true);
        }
        if row.scene == lutz && row.claim == Wait(10) {
            row.defect = Some(CAMPAIGN);
            row.exact = true;
        }
    }
    assert_eq!(
        claim_problems(&unflagged),
        [
            format!("{le_roof} op 11: Within an own loop"),
            "Event_EclipseTorchUsed op 5: a Wait for a loop with no map updates, without a defect"
                .to_owned(),
            format!("{lutz} op 10: exact for a loop that runs map updates"),
        ]
    );

    // The appended route's first `Wait` (224 ticks) is not the scene's own.
    let route_wait = censused_ops(again).len() + 9;
    assert!(matches!(
        scene(again).ops[route_wait],
        SceneOp::Wait { ticks: 224 }
    ));
    let mut on_route = ROWS.to_vec();
    on_route.push(row(again, Map(0x07_846C), Wait(route_wait)));
    assert_eq!(
        claim_problems(&on_route),
        [format!(
            "{again} op {route_wait}: claimed as a Wait, is None"
        )]
    );

    // #591's omissions: the 60-frame `DoMapUpdateLoop` after the fade-out
    // with no row, and a colour loop handed to the op before it (the red
    // fade, which waits no loop) or to a map-updating `Wait`.
    let index_60 = 20;
    assert!(matches!(
        scene(again).ops[index_60],
        SceneOp::Wait { ticks: 60 }
    ));
    let omitted = without(&|row| !(row.scene == again && row.claim == Wait(index_60)));
    assert_eq!(
        claim_problems(&omitted),
        [format!("{again} op {index_60}: a Wait with 0 claims")]
    );
    let mut misclaimed = ROWS.to_vec();
    for row in &mut misclaimed {
        if row.scene == again && row.claim == Within(10) {
            row.claim = Within(9);
        }
    }
    assert_eq!(
        claim_problems(&misclaimed),
        [
            format!(
                "{again} op 9: Within names an op that waits no loop, {:?}",
                scene(again).ops.get(9)
            ),
            format!("{again} op 9: Within an own loop"),
        ]
    );

    let Some(rom) = rom() else {
        return;
    };
    // The 180-pass loop's claim moved to the 71-pass fade-in loop, and one
    // half of the Machine Center's split loop dropped.
    let mut miscounted = ROWS.to_vec();
    for row in &mut miscounted {
        if row.scene == le_roof && row.claim == Within(11) {
            row.retail = own(0x07_05DC, false, false);
        }
        if row.scene == "Cutscene_MeetingKyra" && row.claim == Within(10) {
            row.retail = Frames(0x07_77FC);
        }
    }
    let machine = "Event_MachineCenterAppearing";
    miscounted.retain(|row| !(row.scene == machine && row.claim == Wait(7)));
    let machine_passes = own_ticks(rom, 0x06_B514, loop_end(rom, 0x06_B514).unwrap()).unwrap();
    let machine_claimed =
        claimed_ticks(&row(machine, own(0x06_B514, true, true), Wait(5))).unwrap();
    assert_eq!(
        retail_problems(rom, &miscounted),
        [
            format!(
                "{machine} $06B514: claims {machine_claimed} frames, the loop runs {machine_passes}"
            ),
            format!("{le_roof} $0705DC: claims 251 frames, the loop runs 71"),
            "Cutscene_MeetingKyra $0777FC: claims 60 frames, the loop runs 120".to_owned(),
        ]
    );

    let mut wrong_objects = ROWS.to_vec();
    for row in &mut wrong_objects {
        if row.scene == "Event_SavingKyra" && row.retail == frame(0x07_08EE, true) {
            row.retail = frame(0x07_08EE, false);
        }
    }
    assert_eq!(
        retail_problems(rom, &wrong_objects),
        ["Event_SavingKyra $0708EE: objects is not false".to_owned()]
    );

    for (scene, retail) in [
        ("Event_CarnivorousTrees", frame(0x07_07A0, false)),
        ("Event_SavingKyra", frame(0x07_08EE, true)),
        ("Cutscene_MeetingKyra", Frames(0x07_77FC)),
        (again, Map(0x07_846C)),
        (lutz, Facing(0x07_7964)),
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
    // Without its row the colour loop at `$070538` is no longer known as
    // an own loop, and its `VInt_Prepare` is a bare frame with no row.
    let omitted =
        without(&|row| !(row.scene == le_roof && row.retail == own(0x07_0538, false, false)));
    assert_eq!(
        coverage_problems(rom, &omitted),
        [format!(
            "{le_roof} $070540: {:?} has no row",
            frame(0x07_0540, false)
        )]
    );
}
