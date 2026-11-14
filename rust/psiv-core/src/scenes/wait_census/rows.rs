//! The census's rows: one per retail wait of a censused scene, in scene-file
//! order (#470). The checks that read them are in the parent module.

use super::Claim::{Missing, Recorded, Wait, Within};
use super::Retail::{Facing, Frames, Main, Map, Step};
use super::{CAMPAIGN, DEZO, Row, frame, own, row};

/// A [`Row`] whose claim is approximate, with the issue that fixes it.
const fn defect(issue: u32, row: Row) -> Row {
    Row {
        defect: Some(issue),
        ..row
    }
}

/// A `Wait` row whose count is the retail one, for a frame or loop that runs
/// no map updates: the issue that fixes the updates.
const fn updates(issue: u32, row: Row) -> Row {
    Row {
        defect: Some(issue),
        exact: true,
        ..row
    }
}

#[rustfmt::skip]
pub(super) const ROWS: &[Row] = &[
    // post_rika_events.rs
    // One loop of `$170 + 1` passes the transcription splits at the door
    // chunk's beat (d7 `$3D`).
    row("Event_MachineCenterAppearing", own(0x06_B514, true, true), Wait(5)),
    row("Event_MachineCenterAppearing", own(0x06_B514, true, true), Wait(7)),
    row("Event_GettingLandRover", Main(0x06_DF08), Wait(3)),
    row("Event_GettingLandRover", Map(0x06_DF2E), Wait(6)),
    // The BG camera's scroll: `RunMapUpdates` and `VInt_Prepare`, no objects.
    row("Event_GettingLandRover", own(0x06_DF52, false, true), Wait(9)),
    row("Event_GettingLandRover", Map(0x06_DF98), Wait(12)),
    row("Event_GettingLandRover", Map(0x06_DFDA), Wait(19)),
    Row { defect: Some(496), ..row("Event_PsycoWandChest", own(0x06_EA4E, true, false), Wait(3)) },
    Row { defect: Some(496), ..row("Event_PsycoWandChest", own(0x06_EA88, true, false), Wait(6)) },
    Row { defect: Some(496), ..row("Event_PsycoWandChest", own(0x06_EAA0, true, false), Wait(8)) },
    row("Event_PsycoWandChest", Map(0x06_EAB6), Missing(496)),
    Row { defect: Some(496), ..row("Event_PsycoWandChest", own(0x06_EB32, true, false), Wait(14)) },
    row("Event_PsycoWandChest", Map(0x06_EB5E), Missing(496)),
    // The beams' own updates run through the `DoMainUpdatesLoop($27)`.
    row("Event_ZioFortBarrier", Main(0x06_ED0C), Within(15)),
    // `Palette_Line_4`'s ramp waits `DoMapUpdateLoop($1D)` between steps.
    row("Event_ZioFortBarrier", Map(0x06_ED62), Within(18)),
    row("Event_ZioFortBarrier", Map(0x06_ED70), Wait(19)),
    row("Event_ZioFortBarrier", Map(0x06_EDBC), Wait(24)),
    row("Event_ZioFortBarrier", Main(0x06_EDEA), Wait(29)),
    row("Event_ZioNurvus", Map(0x06_F2F4), Wait(1)),
    row("Event_ZioNurvus", Map(0x06_F300), Missing(496)),
    row("Event_ZioNurvus", own(0x06_F384, true, true), Wait(6)),
    row("Event_ZioNurvus", Map(0x06_F3A4), Wait(7)),
    row("Event_ZioNurvus", Map(0x06_F3CC), Wait(11)),
    // post_rika_cutscenes.rs
    row("Cutscene_AlysWounded", Map(0x07_4C0E), Wait(8)),
    row("Cutscene_AlysWounded", Map(0x07_4C58), Wait(17)),
    // The sparkle object `$1A0` run to its end: `RunSingleObject`, not the
    // field's object update, and no map updates.
    row("Cutscene_AlysWounded", own(0x07_4DD4, false, false), Missing(502)),
    row("Cutscene_AlysWounded", Map(0x07_4DFC), Missing(502)),
    row("Cutscene_PsycoWand", Map(0x07_5256), Wait(5)),
    row("Cutscene_PsycoWand", Map(0x07_5538), Wait(45)),
    row("Cutscene_PsycoWand", Map(0x07_598A), Wait(106)),
    row("Cutscene_ZioDefeated", Map(0x07_5AC0), Wait(13)),
    row("Cutscene_ZioDefeated", Map(0x07_5B0A), Wait(17)),
    row("Cutscene_ZioDefeated", Map(0x07_5B7E), Wait(19)),
    row("Cutscene_ZioDefeated", Map(0x07_5BFE), Wait(22)),
    row("Cutscene_ZioDefeated", Map(0x07_5C12), Wait(24)),
    row("Cutscene_ZioDefeated", Map(0x07_5C34), Wait(25)),
    row("Cutscene_ZioDefeated", Map(0x07_5C48), Wait(27)),
    row("Cutscene_ZioDefeated", Map(0x07_5C66), Wait(30)),
    row("Cutscene_ZioDefeated", Map(0x07_5CE4), Wait(43)),
    // post_zio_cutscenes.rs
    row("Cutscene_CrashLaanding", Map(0x07_6734), Wait(40)),
    row("Cutscene_CrashLaanding", Map(0x07_67FE), Wait(51)),
    row("Cutscene_CrashLaanding", Map(0x07_69DC), Wait(63)),
    Row { defect: Some(504), ..row("Cutscene_CrashLaanding", Map(0x07_69F2), Wait(65)) },
    row("Cutscene_CrashLaanding", Map(0x07_6AD8), Wait(68)),
    // `bsr loc_76E00`: five frames of the wreck, each followed by six passes
    // of `RunMapUpdates` and `DMAPlane_B_VInt`, no objects.
    row("Cutscene_CrashLaanding", own(0x07_6E3C, false, true), Wait(71)),
    row("Cutscene_CrashLaanding", own(0x07_6E3C, false, true), Wait(73)),
    row("Cutscene_CrashLaanding", own(0x07_6E3C, false, true), Wait(75)),
    row("Cutscene_CrashLaanding", own(0x07_6E3C, false, true), Wait(77)),
    row("Cutscene_CrashLaanding", own(0x07_6E3C, false, true), Wait(79)),
    row("Cutscene_CrashLaanding", Map(0x07_6AEC), Wait(80)),
    row("Cutscene_CrashLaanding", Map(0x07_6C14), Wait(100)),
    row("Cutscene_CrashLaanding", Map(0x07_6D58), Wait(119)),
    // The shared `$170 + 1` pass loop the scene calls at `$0771C8`, split at
    // the BG chunk's beat (d7 `$3D`).
    row("Cutscene_Landale", own(0x06_C0A2, true, true), Wait(16)),
    row("Cutscene_Landale", own(0x06_C0A2, true, true), Wait(18)),
    row("Cutscene_DarkForce1Defeated", Map(0x07_71FE), Wait(6)),
    row("Cutscene_DarkForce1Defeated", Map(0x07_720E), Wait(8)),
    // dezo_endgame.rs
    // Seventeen `DoMainUpdatesLoop` calls and two `Event_StepObject` calls
    // against five `Wait`s. Each `$2AC` spawn follows a 15-frame wait its
    // `ObjectAnimation` records without blocking.
    row("Event_StrengthTowerTop", Main(0x07_0E60), Wait(1)),
    defect(DEZO, row("Event_StrengthTowerTop", Main(0x07_0E7C), Recorded(2))),
    defect(DEZO, row("Event_StrengthTowerTop", Main(0x07_0E9E), Recorded(3))),
    defect(DEZO, row("Event_StrengthTowerTop", Main(0x07_0EC0), Recorded(4))),
    defect(DEZO, row("Event_StrengthTowerTop", Main(0x07_0EE2), Recorded(5))),
    row("Event_StrengthTowerTop", Main(0x07_0F04), Wait(6)),
    row("Event_StrengthTowerTop", Main(0x07_0F16), Missing(DEZO)),
    row("Event_StrengthTowerTop", Main(0x07_0F28), Missing(DEZO)),
    row("Event_StrengthTowerTop", Main(0x07_0F30), Missing(DEZO)),
    row("Event_StrengthTowerTop", Step(0x07_0F44), Missing(DEZO)),
    row("Event_StrengthTowerTop", Step(0x07_0F58), Missing(DEZO)),
    row("Event_StrengthTowerTop", Main(0x07_0F60), Missing(DEZO)),
    row("Event_StrengthTowerTop", Main(0x07_0FB4), Wait(8)),
    row("Event_StrengthTowerTop", Main(0x07_100C), Missing(DEZO)),
    row("Event_StrengthTowerTop", Main(0x07_104C), Wait(10)),
    row("Event_StrengthTowerTop", Main(0x07_10B2), Missing(DEZO)),
    // Retail runs it after the speed-8 pan (`$071074`) and the 30-frame
    // wait; the transcription runs it before the pan. The pan back after it
    // is #565's.
    defect(DEZO, row("Event_StrengthTowerTop", Main(0x07_10DE), Wait(12))),
    // Each `$2BC` spawn is followed by a 2-frame wait its `ObjectAnimation`
    // records; the second round of four spawns has no ops.
    row("Event_CourageTowerTop", Facing(0x07_110A), Missing(164)),
    row("Event_CourageTowerTop", Main(0x07_1122), Wait(2)),
    defect(DEZO, row("Event_CourageTowerTop", Main(0x07_1152), Recorded(3))),
    defect(DEZO, row("Event_CourageTowerTop", Main(0x07_118A), Recorded(4))),
    defect(DEZO, row("Event_CourageTowerTop", Main(0x07_11BA), Recorded(5))),
    defect(DEZO, row("Event_CourageTowerTop", Main(0x07_11F2), Recorded(6))),
    row("Event_CourageTowerTop", Main(0x07_11FA), Wait(8)),
    row("Event_CourageTowerTop", Main(0x07_122A), Missing(DEZO)),
    row("Event_CourageTowerTop", Main(0x07_1262), Missing(DEZO)),
    row("Event_CourageTowerTop", Main(0x07_1292), Missing(DEZO)),
    row("Event_CourageTowerTop", Main(0x07_12CA), Missing(DEZO)),
    row("Event_CourageTowerTop", Main(0x07_12D2), Missing(DEZO)),
    row("Event_CourageTowerTop", Main(0x07_1332), Wait(12)),
    row("Event_CourageTowerTop", Main(0x07_139E), Missing(DEZO)),
    row("Event_CourageTowerTop", Main(0x07_13CA), Wait(14)),
    row("Event_CourageTowerTop", Main(0x07_13F0), Missing(DEZO)),
    row("Event_CourageTowerTop", Main(0x07_1414), Wait(16)),
    // The wait before the pan back to `Character_1`.
    row("Event_CourageTowerTop", Main(0x07_1426), Missing(565)),
    row("Event_ReFaze", Map(0x07_1584), Wait(1)),
    row("Event_ReFaze", Facing(0x07_1590), Missing(164)),
    row("Event_ReFaze", Map(0x07_1598), Wait(3)),
    row("Event_ReFaze", Facing(0x07_15A4), Missing(164)),
    row("Event_ReFaze", Map(0x07_15AC), Wait(5)),
    row("Event_ReFaze", Facing(0x07_15C6), Missing(164)),
    row("Event_ReFaze", Map(0x07_15E2), Wait(9)),
    defect(DEZO, row("Event_ReFaze", Main(0x07_160C), Recorded(10))),
    row("Event_ReFaze", Map(0x07_162E), Wait(11)),
    // The 64-frame `loc_58666` restore (no map updates), transcribed after
    // the FireBreath SFX, where retail's 136-frame wait runs.
    defect(DEZO, row("Event_ReFaze", own(0x07_1636, false, false), Wait(13))),
    row("Event_ReFaze", Main(0x07_1652), Missing(DEZO)),
    // The Megid branch's tone loops and restore.
    row("Event_ReFaze", own(0x07_1688, false, false), Missing(DEZO)),
    row("Event_ReFaze", own(0x07_16B8, false, false), Missing(DEZO)),
    row("Event_ReFaze", Main(0x07_16E8), Missing(DEZO)),
    row("Event_ReFaze", own(0x07_1708, false, false), Missing(DEZO)),
    // A `FadeOut` and 40 frames of map updates stand for the first of two
    // 40-frame `Pal_IncreaseTone` loops; the second, and the restore after
    // the objects' 10-frame wait, have no ops.
    defect(DEZO, row("Event_DeVarsDefeated", own(0x07_1772, false, false), Wait(2))),
    row("Event_DeVarsDefeated", own(0x07_17A8, false, false), Missing(DEZO)),
    defect(DEZO, row("Event_DeVarsDefeated", Main(0x07_17F8), Recorded(3))),
    row("Event_DeVarsDefeated", own(0x07_1808, false, false), Missing(DEZO)),
    defect(DEZO, row("Event_SaLewsDefeated", own(0x07_183A, false, false), Wait(2))),
    row("Event_SaLewsDefeated", own(0x07_1870, false, false), Missing(DEZO)),
    defect(DEZO, row("Event_SaLewsDefeated", Main(0x07_18BC), Recorded(3))),
    row("Event_SaLewsDefeated", own(0x07_18CC, false, false), Missing(DEZO)),
    row("Cutscene_BeforeElsydeonCave", Map(0x07_84FC), Wait(5)),
    row("Cutscene_Reunion", Map(0x07_8AB2), Wait(3)),
    row("Cutscene_Reunion", Map(0x07_8AE4), Wait(9)),
    row("Cutscene_Reunion", Map(0x07_8AF4), Wait(11)),
    row("Cutscene_Reunion", Map(0x07_8BBE), Wait(19)),
    row("Event_AngerTowerTop", Map(0x07_224A), Wait(9)),
    row("Event_AngerTowerAlys", Facing(0x07_14A2), Missing(164)),
    row("Event_AngerTowerAlys", Step(0x07_14D0), Within(2)),
    row("Event_AngerTowerAlys", Step(0x07_1502), Within(3)),
    row("Event_AngerTowerAlys", Step(0x07_1530), Within(4)),
    // `loc_78ECE`'s 600 passes: the edge's palette updates and the random
    // white flashes with `SFXID_Tandle`, no `RunMapUpdates`.
    defect(DEZO, row("Cutscene_ProfoundDarkness", own(0x07_8ED4, false, false), Wait(12))),
    // dezo_campaign.rs
    // The `loc_70638` colour-cycle loops run no map updates; `bsr loc_705D6`
    // between the first two adds 71 passes of fade-in with no op.
    row("Event_MeetingLeRoof", Map(0x07_0492), Wait(1)),
    row("Event_MeetingLeRoof", Map(0x07_04A6), Wait(3)),
    updates(CAMPAIGN, row("Event_MeetingLeRoof", own(0x07_0538, false, false), Wait(9))),
    row("Event_MeetingLeRoof", own(0x07_05DC, false, false), Missing(CAMPAIGN)),
    updates(CAMPAIGN, row("Event_MeetingLeRoof", own(0x07_0552, false, false), Wait(10))),
    updates(CAMPAIGN, row("Event_MeetingLeRoof", own(0x07_0570, false, false), Wait(12))),
    row("Cutscene_LeRoofAgain", Map(0x07_8366), Wait(2)),
    updates(CAMPAIGN, row("Cutscene_LeRoofAgain", own(0x07_83F2, false, false), Wait(7))),
    // `jsr loc_705D6` at `$078406`: `Event_MeetingLeRoof`'s fade-in loop.
    row("Cutscene_LeRoofAgain", own(0x07_05DC, false, false), Missing(CAMPAIGN)),
    updates(CAMPAIGN, row("Cutscene_LeRoofAgain", own(0x07_840E, false, false), Wait(8))),
    row("Cutscene_LeRoofAgain", Map(0x07_846C), Missing(CAMPAIGN)),
    // Mounted: a frame of map updates after the SFX stop, a bare frame after
    // `Field_BuildSprites`, and a frame with the objects after the dismount.
    row("Event_CarnivorousTrees", Map(0x07_078C), Wait(3)),
    row("Event_CarnivorousTrees", frame(0x07_07A0, false), Missing(CAMPAIGN)),
    row("Event_CarnivorousTrees", frame(0x07_080C, true), Missing(CAMPAIGN)),
    row("Event_SavingKyra", Map(0x07_086E), Wait(3)),
    row("Event_SavingKyra", frame(0x07_0882, false), Missing(CAMPAIGN)),
    row("Event_SavingKyra", frame(0x07_08EE, true), Missing(CAMPAIGN)),
    row("Event_SavingKyra", Map(0x07_0912), Wait(10)),
    row("Cutscene_MeetingKyra", Frames(0x07_77E2), Within(10)),
    row("Cutscene_MeetingKyra", Frames(0x07_77FC), Within(13)),
    // Mounted: three frames, the last after the dismount; op 5 is that one.
    row("Event_EclipseTorchUsed", frame(0x07_01A0, false), Missing(CAMPAIGN)),
    row("Event_EclipseTorchUsed", frame(0x07_01B4, false), Missing(CAMPAIGN)),
    updates(CAMPAIGN, row("Event_EclipseTorchUsed", frame(0x07_0220, true), Wait(5))),
    row("Event_EclipseTorchUsed", Map(0x07_0238), Wait(9)),
    // Two white flashes, each 2 frames of map updates and 40 passes of the
    // `loc_58666` restore; the transcription has the first restore only.
    row("Event_EclipseTorchUsed", Map(0x07_0266), Missing(CAMPAIGN)),
    updates(CAMPAIGN, row("Event_EclipseTorchUsed", own(0x07_0270, false, false), Wait(11))),
    row("Event_EclipseTorchUsed", Map(0x07_02A8), Missing(CAMPAIGN)),
    row("Event_EclipseTorchUsed", own(0x07_02B2, false, false), Missing(CAMPAIGN)),
    // After the nine `$338` spawns, whose `ObjectAnimation`s record one
    // frame each that no retail loop runs.
    row("Event_EclipseTorchUsed", Main(0x07_044C), Missing(CAMPAIGN)),
    row("Cutscene_LutzRevelation", Map(0x07_78A0), Wait(1)),
    row("Cutscene_LutzRevelation", Facing(0x07_7944), Missing(164)),
    row("Cutscene_LutzRevelation", Main(0x07_794C), Wait(10)),
    row("Cutscene_LutzRevelation", Facing(0x07_7954), Missing(164)),
    row("Cutscene_LutzRevelation", Main(0x07_795C), Wait(12)),
    row("Cutscene_LutzRevelation", Facing(0x07_7964), Missing(164)),
    row("Cutscene_LutzRevelation", Main(0x07_796C), Wait(14)),
    row("Cutscene_LutzRevelation", Facing(0x07_7974), Missing(164)),
    row("Cutscene_LutzRevelation", Main(0x07_797C), Wait(16)),
    row("Cutscene_LutzRevelation", Facing(0x07_7984), Missing(164)),
    row("Cutscene_LutzRevelation", Main(0x07_798C), Wait(18)),
    row("Cutscene_LutzRevelation", Frames(0x07_79D6), Within(25)),
];
