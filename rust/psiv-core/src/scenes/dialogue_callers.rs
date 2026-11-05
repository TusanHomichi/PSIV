//! The dialogue-entry caller census: every `jsr`/`jmp` to the five
//! `Event_GetAndRunDialogue*` entries in the US image, and every inline
//! `jsr GetDialogueByID` inside a transcribed scene's bytes, as a row of
//! [`DIALOGUE_CALLERS`]; and the test that holds the registry to it.
//!
//! The rows are re-derived from the image by `tests/test_dialogue_callers.py`,
//! which also reads each routine's exit from the bytes and checks
//! [`ROUTINE_WINDOWS`] against it, so neither table can rot. The audit, the
//! findings and the callers no scene holds yet are in
//! `docs/scenes/DIALOGUE_CALLERS.md`.

use std::collections::BTreeMap;

use super::SCENES;
use crate::scene::{DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_runner::Scene;

/// `Event_GetAndRunDialogue`: panels, both windows and the map chunks go.
const D1: u32 = 0x5AC66;
/// `Event_GetAndRunDialogue2`: nothing is destroyed, the counts are zeroed.
const D2: u32 = 0x5ACDC;
/// `Event_GetAndRunDialogue3`: as `D1` without `Panel_DestroyAll`.
const D3: u32 = 0x5AD32;
/// `Event_GetAndRunDialogue4`: `RunText3`, no button wait, no panel destroy.
const D4: u32 = 0x5ADA2;
/// `Event_GetAndRunDialogue5`: as `D3`, then `PalFadeOut_ClrSpriteTbl` and
/// `Panel_DestroyAll`.
const D5: u32 = 0x5ADF8;
/// `GetDialogueByID` called by the scene's own window code, which exits its
/// own way: any window may stand for it.
const INLINE: u32 = 0x59164;
/// `d0` is not a literal at the call (read off an object or out of a table).
const DYN: u16 = 0xFFFF;

/// Which [`DialogueWindow`] each routine is. Five routines, five variants;
/// `Cutscene5` (`Event_RunDialogue5`'s resume) and `EndingIntro` (an inline
/// `RunText3` window) belong to no `Get` entry.
const ROUTINE_WINDOWS: &[(u32, DialogueWindow)] = &[
    (D1, DialogueWindow::Standard),
    (D2, DialogueWindow::Retained),
    (D3, DialogueWindow::Cutscene3),
    (D4, DialogueWindow::Ending),
    (D5, DialogueWindow::Cutscene),
];

/// `(scene, routine, call address, entry)`, in image order. A call that more
/// than one `d0` load reaches has a row per entry.
const DIALOGUE_CALLERS: &[(&str, u32, u32, u16)] = &[
    ("Event_AlysFound", D1, 0x06B32C, DYN),
    ("Event_MeetingHahn", D1, 0x06B438, 0x11),
    ("Event_MachineCenterAppearing", D1, 0x06B564, 0x0B),
    ("Event_BasementContainers", D1, 0x06BC46, 0x0F),
    ("Event_MeetingSaya", D1, 0x06BC6C, 0x0E),
    ("Event_AiedoManCrushOnAlys", D1, 0x06BE90, 0x3C),
    ("Event_SuspicionOnPrincipal", D1, 0x06BEEA, 0x6E),
    ("Event_BioPlantAlarm", D1, 0x06C29E, 0x0A),
    ("Event_AiedoManSavings", D1, 0x06D46A, 0x3D),
    ("Event_GirlsSneakingOut", D1, 0x06D53C, 0x3E),
    ("Event_AfterIgglanova", D1, 0x06D6CA, 0x12),
    ("Event_PrincipalConfession", D1, 0x06D7A4, 0x13),
    ("Event_RuneFlaeli", D1, 0x06D830, 0x26),
    ("Event_AlshlineFound", D1, 0x06DBDA, 0x27),
    ("Event_WreckageEngine", D1, 0x06DBF4, 0x48),
    ("Event_WreckageEngine", D1, 0x06DBFC, 0x47),
    ("Event_CancellerReminder", D1, 0x06DEB0, 0x0C),
    ("Event_GettingLandRover", D1, 0x06DEC0, 0x09),
    ("Event_GettingLandRover", D1, 0x06E0A8, 0x0C),
    ("Event_MonsenEarthquake", D1, 0x06E102, 0x40),
    ("Event_PlateSysEngine", D1, 0x06E828, 0x41),
    ("Event_RuneLadaeTower", D1, 0x06E9E0, 0x06),
    ("Event_PsycoWandChest", D1, 0x06EA18, 0x07),
    ("Event_PsycoWandChest", D1, 0x06EACC, 0x08),
    ("Event_ZioFortBarrier", D1, 0x06EC70, 0x44),
    ("Event_ZioFortBarrier", D1, 0x06EDC4, 0x45),
    ("Event_ZioFortBarrier", D1, 0x06EE10, 0x4A),
    ("Event_MeetingDorin", D1, 0x06EEEE, 0x18),
    ("Event_TonoeBasementDoor", D1, 0x06F1A6, 0x24),
    ("Event_ZioNurvus", D2, 0x06F3D4, 0x0B),
    ("Event_ChazHouse", INLINE, 0x06FA42, 0x31),
    ("Event_ChazHouse", D1, 0x06FA50, 0x31),
    ("Event_KuranArrival", D1, 0x06FAD6, 0x04),
    ("Event_NearDarkForce1", D1, 0x06FAE8, 0x05),
    ("Event_DarkForce1", D2, 0x06FAFA, 0x06),
    ("Event_Juza", D2, 0x06FB42, 0x48),
    ("Event_OutsideRajaTemple", D1, 0x06FC84, 0x06),
    ("Event_TylerGraveOpening", D1, 0x06FCB0, 0x1E),
    ("Event_TylerGraveOpening", D1, 0x06FE16, 0x22),
    ("Event_PersistentEsperGuards", D1, 0x06FE2A, 0x2E),
    ("Event_EclipseTorchStolen", D1, 0x06FE8A, 0x32),
    ("Event_EclipseTorchUsed", D1, 0x070460, 0x2A),
    ("Event_MeetingLeRoof", D1, 0x070566, 0x00),
    ("Event_MuskCatsGuarding", D1, 0x0706A0, 0x1C),
    ("Event_MuskCatElder", D1, 0x0706DC, 0x2E),
    ("Event_MuskCatElder", D1, 0x0706E4, 0x2D),
    ("Event_PenguinOwner", D1, 0x07073A, 0x12),
    ("Event_CarnivorousTrees", D1, 0x070820, 0x34),
    ("Event_SavingKyra", D1, 0x070902, 0x34),
    ("Event_SavingKyra", D2, 0x070936, 0x26),
    ("Event_DarkForce2", D2, 0x070978, 0x3A),
    ("Event_EsperGuardPermission", D1, 0x0709E8, 0x00),
    ("Event_EsperGuardPermission", D1, 0x0709F0, 0x02),
    ("Event_EsperGuardPermission", D1, 0x070A0A, 0x01),
    ("Event_DarkForce3Defeated", D1, 0x070A2C, 0x02),
    ("Event_InnerSanctGuard", D1, 0x070A66, 0x35),
    ("Event_InnerSanctGuardBeforeElsydeon", D1, 0x070A8E, 0x36),
    ("Event_ClmCenterAfterBattle", D1, 0x070ADC, 0x31),
    ("Event_DElmLars", D1, 0x070AEE, 0x33),
    ("Event_AfterDElmLarsBattle", D1, 0x070B0E, 0x34),
    ("Event_AirCastleArrival", D1, 0x070B20, 0x35),
    ("Event_XeAThoulBeforeBattle", D2, 0x070B32, 0x36),
    ("Event_AirCastleFakeChest", D2, 0x070B58, 0x3C),
    ("Event_RajaSick", D1, 0x070C72, 0x5D),
    ("Event_RajaSick", D1, 0x070C72, 0x5E),
    ("Event_RajaSick", D1, 0x070C72, 0x5F),
    ("Event_RajaSick", D1, 0x070C7C, 0x60),
    ("Event_Gyuna", D1, 0x070C94, 0x31),
    ("Event_Gyuna", D1, 0x070C94, 0x32),
    ("Event_LashiecAppearance", D2, 0x070E1E, 0x37),
    ("Event_DeVars", D1, 0x07144C, 0x03),
    ("Event_SaLews", D1, 0x07146C, 0x01),
    ("Event_AngerTowerAlys", D1, 0x07148C, 0x07),
    ("Event_AngerTowerAlys", D2, 0x071538, 0x08),
    ("Event_ReFaze", D1, 0x07157C, 0x09),
    ("Event_DeVarsDefeated", D1, 0x07175C, 0x04),
    ("Event_SaLewsDefeated", D1, 0x071824, 0x02),
    ("Event_HuntersGuild", D1, 0x071946, 0x12),
    ("Event_HuntersGuild", D1, 0x07196A, DYN),
    ("Event_HuntersGuild", D1, 0x07199C, DYN),
    ("Event_HuntersGuild", D1, 0x0719E8, DYN),
    ("Event_HuntersGuild", INLINE, 0x071A24, 0x0F),
    ("Event_HuntersGuild", INLINE, 0x071B4E, 0x10),
    ("Event_HuntersGuild", INLINE, 0x071BAC, 0x13),
    ("Event_HuntersGuild", INLINE, 0x071C38, DYN),
    ("Event_HuntersGuild", INLINE, 0x071C88, 0x12),
    ("Event_HuntersGuild", INLINE, 0x071CCA, DYN),
    ("Event_HuntersGuild", INLINE, 0x071CFA, 0x11),
    ("Event_RuneHealingChaz", D1, 0x071E58, 0x2B),
    ("Event_Phonon", D1, 0x072320, 0x4E),
    ("Event_Phonon", D1, 0x072406, 0x4F),
    ("Event_Hijammer", D1, 0x072446, 0x0A),
    ("Event_Burstroc", D1, 0x0724C2, 0x08),
    ("Event_PosiBolt", D1, 0x07253E, 0x06),
    ("Event_RanchOwner", D1, 0x0725DE, 0x20),
    ("Event_RanchOwnerAfterBattle", D1, 0x072690, 0x21),
    ("Event_RockyFound", D1, 0x0726B6, 0x2C),
    ("Event_CatchingRocky", D1, 0x07273A, DYN),
    ("Event_CatchingRocky", D1, 0x0727E8, DYN),
    ("Event_CatchingRocky", D1, 0x07285E, 0x46),
    ("Event_RockyOwner", D1, 0x07288A, 0x5E),
    ("Event_PiataDormOwner", D1, 0x0728B0, 0x6C),
    ("Event_MissingStudentFound", D1, 0x0728E0, 0x52),
    ("Event_MissingStudentFound", D1, 0x0728E8, 0x4C),
    ("Event_MissingStudentFound", D1, 0x072964, 0x4D),
    ("Event_MissingStudentFound", D1, 0x0729DE, 0x53),
    ("Event_OldManNearMissingStudent", D1, 0x072A10, 0x4F),
    ("Event_OldManNearMissingStudent", D1, 0x072A20, 0x53),
    ("Event_MissingStudentInBed", D1, 0x072A48, 0x50),
    ("Event_MissingStudentInBed", D1, 0x072A50, 0x54),
    ("Event_MissingStudentInBed", D1, 0x072ACA, 0x5B),
    ("Event_TallasMom", D1, 0x072AE8, 0x2D),
    ("Event_TallasMomAfterRescue", D1, 0x072B0E, 0x2E),
    ("Event_InsideMonsenHole", D1, 0x072B1E, 0x24),
    ("Event_FractOozeFound", D2, 0x072B2E, 0x2F),
    ("Event_TallasRescued", D1, 0x072B54, 0x27),
    ("Event_LostGirlsMother", D1, 0x072C20, 0x13),
    ("Event_GirlPrison", D1, 0x072C46, 0x5F),
    ("Event_GirlsBail", D1, 0x072C6C, 0x50),
    ("Event_PayingGirlsBail", D1, 0x072CAE, 0x51),
    ("Event_GirlsBailedOut", D1, 0x072CDE, 0x61),
    ("Event_GirlsBailedOut", D1, 0x072D3E, 0x0A),
    ("Event_KingRappy", D2, 0x072FEA, 0x36),
    ("Event_KingRappy", D1, 0x073012, 0x35),
    ("Event_KingRappyDefeated", D1, 0x07302E, 0x37),
    ("Event_KingRappyDefeated", D1, 0x0730EC, 0x38),
    ("Event_ZemaOldMan", D1, 0x073156, 0x6C),
    ("Event_ZemaOldManAfterMission", D1, 0x07317E, 0x6D),
    ("Event_VahalFortEntrance", D1, 0x073190, 0x00),
    ("Event_VahalFortMidway", D1, 0x0731A2, 0x03),
    ("Event_DaughterTerminal", D2, 0x0731B4, 0x04),
    ("Event_VahalFortBarrier", D1, 0x0731E8, 0x01),
    ("Event_VahalFortBarrier", D1, 0x0731FA, 0x02),
    ("Event_DominatorsDefeated", D1, 0x0732C8, 0x05),
    ("Event_WeaponPlantArrival", D1, 0x0733BC, 0x07),
    ("Event_MeeseClinicSickWoman", D1, 0x07341A, 0x3C),
    ("Event_MeeseClinicSickWoman", D1, 0x07341A, 0x3D),
    ("Event_PenguinFeedStolen", D1, 0x0736B4, 0x10),
    ("Event_Pennant", D1, 0x0736EC, 0x64),
    ("Event_WoodCarving", D1, 0x073756, 0x66),
    ("Event_EnterAngerTower", D1, 0x0737CE, 0x05),
    ("Event_EnterAngerTower", D1, 0x073822, 0x06),
    ("Event_SoldiersTempleCaveDialogue1", D1, 0x073882, 0x0E),
    ("Event_SoldiersTempleCaveDialogue2", D1, 0x073894, 0x0F),
    ("Event_SoldiersTempleReached", D1, 0x0738A6, 0x10),
    ("Event_AeroPrismFound", D1, 0x0738B8, 0x11),
    ("Event_PiataGuardsReprimand", D1, 0x073932, 0x10),
    ("Event_GameStart", D1, 0x073960, 0x36),
    ("Event_GameStart", D1, 0x073A78, 0x37),
    ("Event_GameStart", D1, 0x073AF8, 0x38),
    ("Event_GameStart", D1, 0x073BD6, 0x39),
    ("Event_GameStart", INLINE, 0x073CA8, 0x3A),
    ("Event_GameStart", INLINE, 0x073CC8, 0x3B),
    ("Event_GameStart", INLINE, 0x073CE8, 0x3C),
    ("Event_GameStart", INLINE, 0x073D08, 0x3D),
    ("Event_GameStart", INLINE, 0x073D5C, 0x3E),
    ("Event_GameStart", INLINE, 0x073D7C, 0x3F),
    ("Event_GameStart", INLINE, 0x073D9C, 0x40),
    ("Event_GameStart", INLINE, 0x073DBC, 0x41),
    ("Event_PiataChazAlone", D1, 0x073ED0, 0x6D),
    ("Cutscene_PiataPrincipal", D5, 0x073F04, 0x17),
    ("Cutscene_ProfHolt", D5, 0x073F3E, 0x66),
    ("Cutscene_MeetingRune", D5, 0x073FCE, 0x03),
    ("Cutscene_Dorin", D5, 0x074048, 0x25),
    ("Cutscene_Alshline", D5, 0x074362, 0x67),
    ("Cutscene_Alshline", D2, 0x074502, 0x68),
    ("Cutscene_ZemaIgglanovaDefeated", D5, 0x07458E, 0x69),
    ("Cutscene_MeetingRika", D5, 0x074618, 0x00),
    ("Cutscene_MeetingRika", D1, 0x0746FC, 0x01),
    ("Cutscene_MeetingRika", D1, 0x07480E, 0x02),
    ("Cutscene_MeetingRika", D5, 0x0749C4, 0x03),
    ("Cutscene_MeetingRika", D5, 0x074A28, 0x04),
    ("Cutscene_DemiRescue", D3, 0x074A92, 0x42),
    ("Cutscene_AlysWounded", INLINE, 0x074B86, 0x43),
    ("Cutscene_AlysWounded", D1, 0x074D6C, 0x2C),
    ("Cutscene_AlysWounded", D1, 0x075052, 0x2D),
    ("Cutscene_PsycoWand", D5, 0x075214, 0x05),
    ("Cutscene_PsycoWand", D1, 0x07534E, 0x2E),
    ("Cutscene_ZioDefeated", INLINE, 0x075A76, 0x00),
    ("Cutscene_MeetingWren", D5, 0x07600C, 0x01),
    ("Cutscene_SpaceshipSabotage", D3, 0x07643C, 0x02),
    ("Cutscene_CrashLaanding", INLINE, 0x076622, 0x03),
    ("Cutscene_CrashLaanding", D5, 0x076774, 0x07),
    ("Cutscene_CrashLaanding", D5, 0x076B40, 0x08),
    ("Cutscene_Landale", D5, 0x07717E, 0x24),
    ("Cutscene_DarkForce1Defeated", D5, 0x0771F6, 0x07),
    ("Cutscene_DarkForce1Defeated", D5, 0x0772D0, 0x08),
    ("Cutscene_RajaSick", D1, 0x077364, 0x59),
    ("Cutscene_RajaSick", D1, 0x0774AE, 0x62),
    ("Cutscene_RajaSick", D1, 0x07769E, 0x63),
    ("Cutscene_MeetingKyra", D1, 0x0777A6, 0x27),
    ("Cutscene_MeetingKyra", D3, 0x0777D8, 0x28),
    ("Cutscene_LutzRevelation", D1, 0x077898, 0x2A),
    ("Cutscene_LutzRevelation", D1, 0x0779B0, 0x2F),
    ("Cutscene_FindingAirCastle", D5, 0x077A52, 0x18),
    ("Cutscene_LashiecDefeated", D3, 0x077A8C, 0x38),
    ("Cutscene_LashiecDefeated", D1, 0x077BC4, 0x39),
    ("Cutscene_DarkForce2Defeated", D3, 0x077C70, 0x3B),
    ("Cutscene_GumbiousBishop", D1, 0x077DC8, 0x33),
    ("Cutscene_GumbiousBishop", D1, 0x077E68, 0x17),
    ("Cutscene_MeetingSeth", D5, 0x077EDC, 0x00),
    ("Cutscene_AeroPrism", D3, 0x077F8C, 0x01),
    ("Cutscene_Rykros", D3, 0x0781D8, 0x16),
    ("Cutscene_LeRoofAgain", D1, 0x078424, 0x0B),
    ("Cutscene_LeRoofAgain", D5, 0x078496, 0x0C),
    ("Cutscene_BeforeElsydeonCave", D5, 0x078520, 0x30),
    ("Cutscene_Elsydeon", D3, 0x0785B2, 0x31),
    ("Cutscene_Elsydeon", D5, 0x078A1E, 0x32),
    ("Cutscene_Reunion", D5, 0x078ADC, 0x00),
    ("Cutscene_Reunion", D5, 0x078C02, 0x01),
    ("Cutscene_ProfoundDarkness", D2, 0x078E9C, 0x03),
    ("Cutscene_Ending", INLINE, 0x078F70, 0x09),
    ("Cutscene_Ending", D4, 0x079084, 0x0A),
    ("Cutscene_Ending", D4, 0x079114, 0x0B),
    ("Cutscene_Ending", D4, 0x0798AC, 0x0C),
    ("Cutscene_Ending", INLINE, 0x079C18, 0x12),
    ("Cutscene_Ending", INLINE, 0x079C36, 0x13),
    ("Cutscene_Ending", INLINE, 0x079C54, 0x14),
    ("Cutscene_Ending", INLINE, 0x079C72, 0x15),
];

type Row = (&'static str, u32, u32, u16);

fn routine_name(routine: u32) -> &'static str {
    match routine {
        D1 => "Event_GetAndRunDialogue",
        D2 => "Event_GetAndRunDialogue2",
        D3 => "Event_GetAndRunDialogue3",
        D4 => "Event_GetAndRunDialogue4",
        D5 => "Event_GetAndRunDialogue5",
        _ => "GetDialogueByID (inline)",
    }
}

/// Each `RunDialogue` of a scene: its window and its literal entry, `DYN`
/// when the entry is read off an object.
fn dialogue_ops(scene: &Scene) -> Vec<(DialogueWindow, u16)> {
    scene
        .ops
        .iter()
        .filter_map(|op| match op {
            SceneOp::RunDialogue { source, window } => Some((
                *window,
                match source {
                    DialogueSource::Entry(DialogueId(entry)) => *entry,
                    DialogueSource::NpcDialogueId(_) => DYN,
                },
            )),
            _ => None,
        })
        .collect()
}

/// Everything wrong between `scenes` and `table`; empty when they agree.
///
/// Per scene and window: the entries its rows for that window's routine give
/// must be exactly the entries it runs in that window (as multisets), except
/// that an entry an `INLINE` row gives may run in any window, and a routine
/// with a `DYN` row only needs its literal entries present (the port unrolls
/// the table `d0` came from). A `RunDialogue` in a window with no routine row
/// is an error, so a scene cannot claim a routine its bytes do not call.
fn caller_problems(scenes: &[Scene], table: &[Row]) -> Vec<String> {
    let mut problems = Vec::new();
    let window_of = |routine: u32| {
        ROUTINE_WINDOWS
            .iter()
            .find(|(r, _)| *r == routine)
            .map(|(_, w)| *w)
    };
    let mut seen = BTreeMap::new();
    for row in table {
        if let Some(other) = seen.insert((row.1, row.2, row.3), row.0) {
            problems.push(format!(
                "${:06X} {} entry {:#X} is claimed by both {other} and {}",
                row.2,
                routine_name(row.1),
                row.3,
                row.0
            ));
        }
        if !scenes.iter().any(|scene| scene.name == row.0) {
            problems.push(format!(
                "{} has rows in DIALOGUE_CALLERS (${:06X}) but is not in the registry",
                row.0, row.2
            ));
        }
    }
    for scene in scenes {
        let rows: Vec<&Row> = table.iter().filter(|row| row.0 == scene.name).collect();
        let inline: Vec<u16> = rows
            .iter()
            .filter(|row| row.1 == INLINE)
            .map(|row| row.3)
            .collect();
        let ran = dialogue_ops(scene);
        let mut windows: Vec<DialogueWindow> = ran.iter().map(|(w, _)| *w).collect();
        windows.extend(ROUTINE_WINDOWS.iter().map(|(_, w)| *w));
        windows.sort();
        windows.dedup();
        for window in windows {
            let mut want: Vec<(u16, u32, u32)> = rows
                .iter()
                .filter(|row| window_of(row.1) == Some(window))
                .map(|row| (row.3, row.1, row.2))
                .collect();
            let mut have: Vec<u16> = ran
                .iter()
                .filter(|(w, _)| *w == window)
                .map(|(_, e)| *e)
                .collect();
            let dynamic = want.iter().any(|(entry, _, _)| *entry == DYN);
            want.retain(|(entry, _, _)| *entry != DYN);
            if dynamic && have.is_empty() {
                problems.push(format!(
                    "{}: its bytes call the {window:?} routine with a computed entry, \
                     the scene runs no {window:?} dialogue",
                    scene.name
                ));
            }
            for (entry, routine, at) in &want {
                if let Some(i) = have.iter().position(|e| e == entry) {
                    if !dynamic {
                        have.swap_remove(i);
                    }
                } else {
                    problems.push(format!(
                        "{}: ${at:06X} runs entry {entry:#X} through {}, the scene has no \
                         RunDialogue {{ Entry({entry:#X}), DialogueWindow::{window:?} }}",
                        scene.name,
                        routine_name(*routine)
                    ));
                }
            }
            if dynamic {
                continue;
            }
            for entry in have {
                if entry == DYN || !inline.contains(&entry) {
                    problems.push(format!(
                        "{}: runs entry {entry:#X} in DialogueWindow::{window:?}, which no \
                         caller in its bytes asks for",
                        scene.name
                    ));
                }
            }
        }
    }
    problems
}

#[test]
fn every_dialogue_entry_caller_runs_its_window() {
    let problems = caller_problems(SCENES, DIALOGUE_CALLERS);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The H22 shape: `RunDialogueResume` reopens `Saved_Dialogue_Addr`, so a
/// scene may only resume after it has run a dialogue itself. A scene whose
/// first dialogue op is a resume reads whatever ran last in play, or nothing
/// after a load.
#[test]
fn no_scene_resumes_before_it_opens_a_dialogue() {
    for scene in SCENES {
        let first = scene.ops.iter().find(|op| {
            matches!(
                op,
                SceneOp::RunDialogue { .. }
                    | SceneOp::RunDialogueResume
                    | SceneOp::RunDialogueResumeWithWindow { .. }
            )
        });
        assert!(
            !matches!(
                first,
                Some(SceneOp::RunDialogueResume | SceneOp::RunDialogueResumeWithWindow { .. })
            ),
            "{} resumes a dialogue it never opened",
            scene.name
        );
    }
}

/// The registry with `name`'s ops replaced by `ops`.
fn replaced(name: &str, ops: Vec<SceneOp>) -> Vec<Scene> {
    let ops: &'static [SceneOp] = Box::leak(ops.into_boxed_slice());
    assert!(
        SCENES.iter().any(|scene| scene.name == name),
        "{name} is not registered"
    );
    SCENES
        .iter()
        .map(|scene| Scene {
            name: scene.name,
            event: scene.event,
            ops: if scene.name == name { ops } else { scene.ops },
        })
        .collect()
}

fn ops_of(name: &str) -> Vec<SceneOp> {
    SCENES
        .iter()
        .find(|scene| scene.name == name)
        .map(|scene| scene.ops.to_vec())
        .unwrap_or_else(|| panic!("{name} is not registered"))
}

/// The registry with `name`'s ops rewritten by `edit`, which must change one.
fn edited(name: &str, edit: impl Fn(&SceneOp) -> SceneOp) -> Vec<Scene> {
    let before = ops_of(name);
    let after: Vec<SceneOp> = before.iter().map(edit).collect();
    assert_ne!(
        before, after,
        "the control's edit changed nothing in {name}"
    );
    replaced(name, after)
}

const fn run(entry: u16, window: DialogueWindow) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window,
    }
}

fn swap(from: SceneOp, to: SceneOp) -> impl Fn(&SceneOp) -> SceneOp {
    move |op| if *op == from { to } else { *op }
}

#[test]
fn control_a_scene_back_on_resume_is_reported() {
    let scenes = edited(
        "Event_ZioNurvus",
        swap(
            run(0x0B, DialogueWindow::Retained),
            SceneOp::RunDialogueResume,
        ),
    );
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS);
    assert_eq!(
        problems,
        vec![
            "Event_ZioNurvus: $06F3D4 runs entry 0xB through Event_GetAndRunDialogue2, the \
             scene has no RunDialogue { Entry(0xB), DialogueWindow::Retained }"
        ]
    );
}

#[test]
fn control_a_wrong_variant_is_reported_both_ways() {
    // `Cutscene_MeetingKyra`'s `$28` back on the `Event_GetAndRunDialogue5`
    // window, as it was transcribed before this census.
    let scenes = edited(
        "Cutscene_MeetingKyra",
        swap(
            run(0x28, DialogueWindow::Cutscene3),
            run(0x28, DialogueWindow::Cutscene),
        ),
    );
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    let reported = |needle: &str| problems.iter().any(|p| p.contains(needle));
    assert!(reported(
        "$0777D8 runs entry 0x28 through Event_GetAndRunDialogue3"
    ));
    assert!(reported(
        "runs entry 0x28 in DialogueWindow::Cutscene, which no"
    ));
}

#[test]
fn control_a_wrong_entry_is_reported() {
    let scenes = edited(
        "Event_Juza",
        swap(
            run(0x48, DialogueWindow::Retained),
            run(0x47, DialogueWindow::Retained),
        ),
    );
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS);
    assert_eq!(problems.len(), 2, "{problems:#?}");
}

#[test]
fn control_a_duplicated_op_is_reported() {
    // Twice the one retained window `Event_DarkForce2` has: the multiset
    // comparison sees a count, not just a set.
    let mut ops = ops_of("Event_DarkForce2");
    ops.push(run(0x3A, DialogueWindow::Retained));
    let scenes = replaced("Event_DarkForce2", ops);
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS);
    assert_eq!(
        problems,
        vec![
            "Event_DarkForce2: runs entry 0x3A in DialogueWindow::Retained, which no caller \
             in its bytes asks for"
        ]
    );
}

#[test]
fn control_a_dropped_row_is_reported() {
    let table: Vec<Row> = DIALOGUE_CALLERS
        .iter()
        .copied()
        .filter(|row| row.2 != 0x06FB42)
        .collect();
    let problems = caller_problems(SCENES, &table);
    assert_eq!(
        problems,
        vec![
            "Event_Juza: runs entry 0x48 in DialogueWindow::Retained, which no caller in its \
             bytes asks for"
        ]
    );
}

#[test]
fn control_a_caller_claimed_by_two_scenes_is_reported() {
    let mut table = DIALOGUE_CALLERS.to_vec();
    table.push(("Event_DarkForce2", D2, 0x06F3D4, 0x0B));
    let problems = caller_problems(SCENES, &table);
    assert!(
        problems[0].starts_with("$06F3D4 Event_GetAndRunDialogue2 entry 0xB is claimed by both"),
        "{problems:#?}"
    );
}

#[test]
fn control_a_row_for_an_unregistered_scene_is_reported() {
    let mut table = DIALOGUE_CALLERS.to_vec();
    table.push(("Event_DyingBoy", D1, 0x072DDE, 0x33));
    let problems = caller_problems(SCENES, &table);
    assert_eq!(
        problems,
        vec!["Event_DyingBoy has rows in DIALOGUE_CALLERS ($072DDE) but is not in the registry"]
    );
}

#[test]
fn control_a_dynamic_caller_with_no_window_is_reported() {
    // `Event_AlysFound` reads `d0` off Alys's object (`move.b $14(a4), d0`).
    let scenes = edited("Event_AlysFound", |op| match op {
        SceneOp::RunDialogue { source, .. } => SceneOp::RunDialogue {
            source: *source,
            window: DialogueWindow::Cutscene,
        },
        other => *other,
    });
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(problems[0].contains("computed entry"));
}

#[test]
fn control_an_inline_entry_does_not_excuse_another_entry() {
    // `Cutscene_CrashLaanding` runs `$03` from its own window code
    // (`$076622`); a `$04` in the same window is still unexplained.
    let scenes = edited(
        "Cutscene_CrashLaanding",
        swap(
            run(0x03, DialogueWindow::Cutscene),
            run(0x04, DialogueWindow::Cutscene),
        ),
    );
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS);
    assert_eq!(
        problems,
        vec![
            "Cutscene_CrashLaanding: runs entry 0x4 in DialogueWindow::Cutscene, which no \
             caller in its bytes asks for"
        ]
    );
}
