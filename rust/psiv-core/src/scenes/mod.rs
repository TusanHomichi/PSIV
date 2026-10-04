//! The scene registry: retail-byte transcriptions from the opening act through
//! the post-Zio Zelan/Dezolis/Kuran handoff and terminal ending surfaces.
//!
//! Every scene here comes from `docs/scenes/`, which disassembled the cartridge
//! directly. That indirection is not ceremony: `ps4.asm` `include`s
//! `script/scenes/<Name>/event.asm` for seventeen scenes, and that
//! directory does not exist in the clone at all — for those the reference
//! contains no behaviour, just a label, an absent include and an `rts`. Six are
//! in the opening act. A seventh (`Event_PrincipalConfession`) is commented out to a
//! bare `rts`, and an eighth (`Event_PiataGuardsReprimand`) is rewritten in
//! place to use the wrong dialogue tree. Nothing here was read off the clone.
//!
//! # Coverage: the whole opening act
//!
//! | Scene | Event | Doc | Ops |
//! |---|---|---|---:|
//! | `Event_GameStart` | `$9F` | `01_GameStart.md` | 78 |
//! | `Event_PiataChazAlone` | `$A0` | `02_PiataChazAlone.md` | 2 |
//! | `Event_AlysFound` | `$03` | `03_AlysFound.md` | 12 |
//! | `Cutscene_PiataPrincipal` | `$8001` | `04_PiataPrincipal.md` | 10 |
//! | `Event_SuspicionOnPrincipal` | `$0F` | `05_SuspicionOnPrincipal.md` | 2 |
//! | `Event_MeetingHahn` | `$04` | `06_MeetingHahn.md` | 14 |
//! | `Event_BasementContainers` | `$0C` | `07_BasementContainers.md` | 7 |
//! | `Event_IgglanovaBattle` | `$6B` | `08_IgglanovaBattle.md` | 4 |
//! | `Event_AfterIgglanova` | `$25` | `09_AfterIgglanova.md` | 15 |
//! | `Event_PrincipalConfession` | `$26` | `10_PrincipalConfession.md` | 6 |
//! | `Event_PiataGuardsReprimand` | `$9E` | `11_PiataGuardsReprimand.md` | 10 |
//!
//! Op counts are the docs' own, and a test asserts each one — a transcription
//! that drifts from its doc trips a test rather than a playthrough.
//!
//! # Terminal deterministic surfaces
//!
//! | Scene | Event | Doc | Ops |
//! |---|---|---|---:|
//! | `Cutscene_RajaSick` | `$8012` | `88_RetailBoundaries.md` | 54 |
//! | `Cutscene_Rykros` | `$801B` | `88_RetailBoundaries.md` | 27 |
//! | `Cutscene_Ending` | `$8021` | `89_Ending.md` | 367 |
//!
//! These are kept beside the opening and next-arc registry because their
//! retail bytes are deterministic presentation routines, while the remaining
//! recorded boundaries are input-driven controls rather than fixed scenes.
//!
//! # Two waits, three primitives
//!
//! [`SceneOp::Wait`] is `DoMapUpdateLoop` (`$5A71E`): it runs a map update per
//! frame and discards input. [`SceneOp::WaitFrames`] is `VInt_PrepareLoop`
//! (`$5A7AC`) or a bare `VInt_Prepare` (`$4204C`): frames pass, no map update.
//! Collapsing them would stay frame-accurate while running map updates retail
//! does not, so the transcriptions keep them apart. Both are `dbra` loops, so
//! every count below is the corrected `d0 + 1`.

mod bioplant;
pub(crate) mod dezo_campaign;
pub(crate) mod dezo_endgame;
mod flight;
mod game_start;
pub(crate) mod next_arc;
pub(crate) mod next_arc_followup;
pub(crate) mod opening;
pub(crate) mod post_rika_cutscenes;
pub(crate) mod post_rika_events;
pub(crate) mod post_zio_cutscenes;
pub(crate) mod retail_endgame;
pub(crate) mod vehicles;

pub(crate) use flight::INSIDE_SPACESHIP_ROUTE;
pub use flight::{
    DESTINATION_DEFAULT_MASK, DESTINATION_FLAG_TABLE, DestinationMask, FlightLeg, FlightTarget,
    SPACEPORT_MAPS, WORLD_AIR_CASTLE, WORLD_COUNT, WORLD_DEZOLIS, WORLD_KURAN, WORLD_MOTAVIA,
    WORLD_RYKROS, WORLD_ZELAN, destination_mask_byte, destination_worlds, flight_target,
};

use crate::scene::{DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_runner::Scene;
use crate::state::CharId;
use crate::trigger::EventIndex;

/// `CharID_Chaz`. Behaviourally confirmed on the oracle tape.
pub const CHAZ: CharId = CharId(0);
/// `CharID_Alys`.
pub const ALYS: CharId = CharId(1);
/// `CharID_Hahn`, from `Event_MeetingHahn`'s `moveq #2,d0`.
pub const HAHN: CharId = CharId(2);
/// `CharID_Rune`, used by the Zema/Tonoe recruitment scenes.
pub const RUNE: CharId = CharId(3);
/// `CharID_Gryz`, used by Dorin's replacement scene.
pub const GRYZ: CharId = CharId(4);
/// `CharID_Rika`, added to party slot 5 after the Bio Plant escape.
pub const RIKA: CharId = CharId(5);
/// `CharID_Demi`, seated after Alys and Hahn leave the party.
pub const DEMI: CharId = CharId(6);
/// `CharID_Wren`, added by `Cutscene_MeetingWren`.
pub const WREN: CharId = CharId(7);
/// `CharID_Raja`, added by `Cutscene_CrashLaanding`.
pub const RAJA: CharId = CharId(8);
/// `CharID_Kyra`.
pub const KYRA: CharId = CharId(9);
/// `CharID_Seth`.
pub const SETH: CharId = CharId(10);

/// `moveq #entry, d0 / jsr Event_GetAndRunDialogue2` (`$5ACDC`): entry
/// `entry` of the current tree, with the window and panels left on screen
/// ([`DialogueWindow::Retained`]). The census
/// `every_dialogue2_caller_is_a_retained_window` ties each use to its
/// cartridge address.
pub(crate) const fn retained(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Retained,
    }
}

/// Every transcribed scene, in story order.
pub static SCENES: &[Scene] = &[
    game_start::GAME_START,
    opening::PIATA_CHAZ_ALONE,
    opening::ALYS_FOUND,
    opening::PIATA_PRINCIPAL,
    opening::SUSPICION_ON_PRINCIPAL,
    opening::MEETING_HAHN,
    opening::BASEMENT_CONTAINERS,
    opening::IGGLANOVA_BATTLE,
    opening::AFTER_IGGLANOVA,
    opening::PRINCIPAL_CONFESSION,
    opening::PIATA_GUARDS_REPRIMAND,
    next_arc::PROF_HOLT,
    next_arc::MEETING_RUNE,
    next_arc::MEETING_DORIN,
    next_arc::DORIN,
    next_arc::RUNE_FLAELI,
    next_arc::ALSHLINE_FOUND,
    next_arc::ALSHLINE,
    next_arc::ZEMA_IGGLANOVA_DEFEATED,
    next_arc::ZEMA_SERVANT_BATTLE,
    next_arc::ZEMA_OLD_MAN,
    next_arc::ZEMA_OLD_MAN_AFTER_MISSION,
    next_arc::MEETING_SAYA,
    next_arc::TONOE_BASEMENT_DOOR,
    next_arc_followup::BIO_PLANT_ALARM,
    bioplant::BIOPLANT_DOOR,
    bioplant::ELEVATOR_DOOR,
    bioplant::ELEVATOR_RIDE,
    next_arc_followup::GIRLS_SNEAKING_OUT,
    next_arc_followup::CHAZ_HOUSE,
    next_arc_followup::LEAVING_CHAZ_HOUSE,
    next_arc_followup::MEETING_RIKA,
    post_rika_cutscenes::DEMI_RESCUE,
    post_rika_cutscenes::ALYS_WOUNDED,
    post_rika_events::GETTING_LAND_ROVER,
    post_rika_events::MACHINE_CENTER_APPEARING,
    post_rika_events::RUNE_LADEA_TOWER,
    post_rika_events::PSYCO_WAND_CHEST,
    post_rika_cutscenes::PSYCO_WAND,
    post_rika_events::ZIO_FORT_BARRIER,
    post_rika_events::ZIO_NURVUS,
    post_rika_cutscenes::ZIO_DEFEATED,
    post_zio_cutscenes::MEETING_WREN,
    post_zio_cutscenes::INSIDE_SPACESHIP,
    post_zio_cutscenes::SPACESHIP_SABOTAGE,
    post_zio_cutscenes::CRASH_LANDING,
    post_zio_cutscenes::LANDALE,
    post_zio_cutscenes::KURAN_ARRIVAL,
    post_zio_cutscenes::NEAR_DARK_FORCE_1,
    post_zio_cutscenes::DARK_FORCE_1,
    post_zio_cutscenes::DARK_FORCE_1_DEFEATED,
    post_zio_cutscenes::JUZA,
    post_zio_cutscenes::JUZA_DEFEATED,
    dezo_campaign::MEETING_LE_ROOF,
    dezo_campaign::LE_ROOF_AGAIN,
    dezo_campaign::CARNIVOROUS_TREES,
    dezo_campaign::SAVING_KYRA,
    dezo_campaign::MEETING_KYRA,
    dezo_campaign::ECLIPSE_TORCH_USED,
    dezo_campaign::DARK_FORCE_2,
    dezo_campaign::LUTZ_REVELATION,
    dezo_campaign::DARK_FORCE_2_DEFEATED,
    dezo_campaign::MEETING_SETH,
    dezo_campaign::AERO_PRISM,
    dezo_campaign::DARK_FORCE_3_DEFEATED,
    dezo_campaign::RESHEL_BATTLE,
    dezo_campaign::CLM_CENTER_FORCED_BATTLE,
    dezo_campaign::CLM_CENTER_AFTER_BATTLE,
    dezo_campaign::D_ELM_LARS,
    dezo_campaign::AFTER_D_ELM_LARS_BATTLE,
    dezo_campaign::FINDING_AIR_CASTLE,
    dezo_campaign::AIR_CASTLE_ARRIVAL,
    dezo_campaign::XE_ATHOUL_BEFORE_BATTLE,
    dezo_campaign::AIR_CASTLE_FAKE_CHEST,
    dezo_campaign::LASHIEC_APPEARANCE,
    dezo_campaign::LASHIEC_DEFEATED,
    dezo_campaign::GUMBIOUS_BISHOP,
    dezo_endgame::STRENGTH_TOWER_TOP,
    dezo_endgame::COURAGE_TOWER_TOP,
    dezo_endgame::DE_VARS,
    dezo_endgame::SA_LEWS,
    dezo_endgame::REFAZE,
    dezo_endgame::DE_VARS_DEFEATED,
    dezo_endgame::SA_LEWS_DEFEATED,
    dezo_endgame::BEFORE_ELSYDEON_CAVE,
    dezo_endgame::ELSYDEON,
    dezo_endgame::REUNION,
    dezo_endgame::ANGER_TOWER_TOP,
    dezo_endgame::ANGER_TOWER_EXIT_TOP,
    dezo_endgame::PROFOUND_DARKNESS,
    retail_endgame::RAJA_SICK,
    retail_endgame::RYKROS,
    retail_endgame::ENDING,
    // The field menu's own events, not story beats: the three machines board
    // from the ITEM menu (`ItemActionPtrs`, `ps4.asm:123419-123465`).
    vehicles::BOARDING_LAND_ROVER,
    vehicles::BOARDING_ICE_DIGGER,
    vehicles::BOARDING_HYDROFOIL,
];

/// The scene an event index selects, if it has been transcribed.
#[must_use]
pub fn scene_for(event: EventIndex) -> Option<&'static Scene> {
    SCENES.iter().find(|scene| scene.event == event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{DialogueId, DialogueSource, DialogueWindow, SceneOp};

    /// Op counts as the transcription docs state them.
    ///
    /// Two scenes diverge from their doc's number, both for stated reasons.
    ///
    /// `Event_GameStart`'s doc says "~78" and means it — the tilde is theirs.
    /// The count depends on how the prologue's two `dbra` colour ramps are
    /// expanded; here each page is 8 ops (4 text draws, one fade-up loop, the
    /// 900-frame hold, one fade-down loop, the closing pause), matching the
    /// asm's loop structure rather than unrolling 20 iterations each.
    ///
    /// `Event_MeetingHahn` is the other: its doc writes the
    /// command-selecting branch as a single op that computes a *value*, and
    /// this vocabulary has no value-selecting branch — so it becomes control
    /// flow over two `MoveActorCommand` ops plus a `Jump`, two ops more.
    #[test]
    fn every_scene_matches_its_docs_op_count() {
        let expected: &[(&str, usize)] = &[
            ("Event_GameStart", 80),
            ("Event_PiataChazAlone", 2),
            ("Event_AlysFound", 12),
            ("Cutscene_PiataPrincipal", 10),
            ("Event_SuspicionOnPrincipal", 2),
            ("Event_MeetingHahn", 16),
            ("Event_BasementContainers", 7),
            ("Event_IgglanovaBattle", 4),
            ("Event_AfterIgglanova", 15),
            ("Event_PrincipalConfession", 6),
            ("Event_PiataGuardsReprimand", 10),
            ("Cutscene_ProfHolt", 13),
            ("Cutscene_MeetingRune", 13),
            ("Event_MeetingDorin", 36),
            ("Cutscene_Dorin", 17),
            ("Event_RuneFlaeli", 45),
            ("Event_AlshlineFound", 2),
            ("Cutscene_Alshline", 86),
            ("Cutscene_ZemaIgglanovaDefeated", 11),
            ("Event_ZemaServantBattle", 4),
            ("Event_ZemaOldMan", 3),
            ("Event_ZemaOldManAfterMission", 3),
            ("Event_MeetingSaya", 12),
            ("Event_TonoeBasementDoor", 19),
            ("Event_BioPlantAlarm", 6),
            ("Event_BioPlantDoorOpening", 8),
            ("Event_ElevatorDoorOpening", 9),
            ("Event_RidingElevator", 19),
            ("Event_GirlsSneakingOut", 27),
            ("Event_ChazHouse", 13),
            ("Event_LeavingChazHouse", 1),
            ("Cutscene_MeetingRika", 107),
            ("Cutscene_DemiRescue", 26),
            ("Cutscene_AlysWounded", 57),
            ("Event_GettingLandRover", 31),
            ("Event_MachineCenterAppearing", 14),
            ("Event_RuneLadaeTower", 9),
            ("Event_PsycoWandChest", 20),
            ("Cutscene_PsycoWand", 110),
            ("Event_ZioFortBarrier", 35),
            ("Event_ZioNurvus", 17),
            ("Cutscene_ZioDefeated", 45),
            ("Cutscene_MeetingWren", 16),
            ("Cutscene_InsideSpaceship", 38),
            ("Cutscene_SpaceshipSabotage", 36),
            ("Cutscene_CrashLaanding", 131),
            ("Cutscene_Landale", 22),
            ("Event_KuranArrival", 2),
            ("Event_NearDarkForce1", 2),
            ("Event_DarkForce1", 5),
            ("Cutscene_DarkForce1Defeated", 28),
            ("Event_Juza", 5),
            ("Event_JuzaDefeated", 3),
            ("Event_MeetingLeRoof", 17),
            ("Cutscene_LeRoofAgain", 61),
            ("Event_CarnivorousTrees", 13),
            ("Event_SavingKyra", 19),
            ("Cutscene_MeetingKyra", 24),
            ("Event_EclipseTorchUsed", 28),
            ("Event_DarkForce2", 6),
            ("Cutscene_LutzRevelation", 30),
            ("Cutscene_DarkForce2Defeated", 33),
            ("Cutscene_MeetingSeth", 11),
            ("Cutscene_AeroPrism", 27),
            ("Event_DarkForce3Defeated", 2),
            ("Event_ReshelBattle", 4),
            ("Event_ClmCenterForcedBattle", 4),
            ("Event_ClmCenterAfterBattle", 2),
            ("Event_DElmLars", 4),
            ("Event_AfterDElmLarsBattle", 2),
            ("Cutscene_FindingAirCastle", 44),
            ("Event_AirCastleArrival", 2),
            ("Event_XeAThoulBeforeBattle", 5),
            ("Event_AirCastleFakeChest", 11),
            ("Event_LashiecAppearance", 12),
            ("Cutscene_LashiecDefeated", 28),
            ("Cutscene_GumbiousBishop", 18),
            ("Event_StrengthTowerTop", 15),
            ("Event_CourageTowerTop", 18),
            ("Event_DeVars", 4),
            ("Event_SaLews", 4),
            ("Event_ReFaze", 26),
            ("Event_DeVarsDefeated", 6),
            ("Event_SaLewsDefeated", 6),
            ("Cutscene_BeforeElsydeonCave", 16),
            ("Cutscene_Elsydeon", 47),
            ("Cutscene_Reunion", 36),
            ("Event_AngerTowerTop", 12),
            ("Event_AngerTowerExitTop", 10),
            ("Cutscene_ProfoundDarkness", 20),
            ("Cutscene_RajaSick", 54),
            ("Cutscene_Rykros", 27),
            ("Cutscene_Ending", 367),
            ("Event_BoardingLandRover", 7),
            ("Event_BoardingIceDigger", 7),
            ("Event_BoardingHydrofoil", 7),
        ];
        assert_eq!(
            SCENES.len(),
            expected.len(),
            "every act scene is registered"
        );
        for (name, ops) in expected {
            let scene = SCENES
                .iter()
                .find(|s| s.name == *name)
                .unwrap_or_else(|| panic!("{name} is missing from the registry"));
            assert_eq!(scene.ops.len(), *ops, "{name} op count");
        }
    }

    #[test]
    fn every_jump_target_is_in_range() {
        for scene in SCENES {
            let len = scene.ops.len();
            for (index, op) in scene.ops.iter().enumerate() {
                let targets: Vec<usize> = match op {
                    SceneOp::Jump { to } => vec![*to],
                    SceneOp::BranchDialogueEnd { if_ended } => vec![*if_ended],
                    SceneOp::BranchFlag {
                        if_set, if_clear, ..
                    } => vec![*if_set, *if_clear],
                    SceneOp::BranchChoice { if_yes, if_no } => vec![*if_yes, *if_no],
                    SceneOp::BranchIfAligned {
                        if_aligned, if_not, ..
                    } => vec![*if_aligned, *if_not],
                    SceneOp::BranchIfVehicle {
                        if_mounted,
                        if_on_foot,
                    } => vec![*if_mounted, *if_on_foot],
                    SceneOp::BranchIfActorGreater {
                        if_greater, if_not, ..
                    } => vec![*if_greater, *if_not],
                    _ => vec![],
                };
                for target in targets {
                    assert!(
                        target <= len,
                        "{} op {index} jumps to {target}, past {len}",
                        scene.name
                    );
                }
            }
        }
    }

    #[test]
    fn every_scene_loads_the_trees_its_retail_bytes_load() {
        // `DialogueTreesToRAM` calls per transcribed scene, counted in the US
        // retail image's own byte range for that scene
        // (`docs/scenes/REVISION_AUDIT.md`; `tests/test_scene_trees.py`
        // re-derives this column from the ROM, so the table cannot rot in
        // silence). A scene absent from this list loads no tree.
        //
        // This is the H17 guard. `Cutscene_AlysWounded` and `Cutscene_PsycoWand`
        // reached dialogue `$2C`/`$2D`/`$2E` on Krup Inn F1's bound tree 5
        // because the four `move.l #DialogueTree6, d0` calls before them sit
        // under `if revision>0` and were dropped together. Op counts and map
        // state both looked right; only a per-scene tree-load census sees it.
        const RETAIL_TREE_LOADS: &[(&str, usize)] = &[
            ("Event_GameStart", 4),
            ("Event_PiataGuardsReprimand", 2),
            ("Cutscene_Alshline", 3),
            ("Cutscene_AlysWounded", 3),
            ("Cutscene_ZemaIgglanovaDefeated", 1),
            ("Cutscene_MeetingRika", 1),
            ("Cutscene_PsycoWand", 3),
            ("Cutscene_ZioDefeated", 1),
            ("Cutscene_CrashLaanding", 2),
            ("Cutscene_LashiecDefeated", 2),
            ("Cutscene_GumbiousBishop", 1),
            ("Cutscene_MeetingSeth", 1),
            ("Cutscene_Rykros", 1),
            ("Cutscene_Ending", 1),
            ("Cutscene_MeetingKyra", 1),
            ("Event_CarnivorousTrees", 1),
            ("Event_SavingKyra", 2),
            ("Event_EclipseTorchUsed", 1),
        ];
        for scene in SCENES {
            let ported = scene
                .ops
                .iter()
                .filter(|op| matches!(op, SceneOp::SetDialogueTree { .. }))
                .count();
            let retail = RETAIL_TREE_LOADS
                .iter()
                .find(|(name, _)| *name == scene.name)
                .map_or(0, |(_, loads)| *loads);
            assert_eq!(
                ported, retail,
                "{} loads {retail} dialogue tree(s) in the cartridge",
                scene.name
            );
        }
        for (name, _) in RETAIL_TREE_LOADS {
            assert!(
                SCENES.iter().any(|scene| scene.name == *name),
                "{name} is missing from the registry"
            );
        }
    }

    /// Every `jsr Event_GetAndRunDialogue2` (`$5ACDC`) in the US image, as
    /// `(scene, address of the jsr, dialogue entry in d0)`. This is the H22
    /// class table: the cartridge runs entry `entry` of the current tree and
    /// leaves the window up, which is [`DialogueWindow::Retained`], never a
    /// `RunDialogueResume`. `tests/test_dialogue2_callers.py` re-derives the
    /// column from the ROM, so the table cannot rot; the four callers whose
    /// scenes are not transcribed yet are listed in
    /// `docs/scenes/DIALOGUE2_CALLERS.md` and `tests/test_dialogue2_callers.py`.
    const DIALOGUE2_CALLERS: &[(&str, u32, u16)] = &[
        ("Event_ZioNurvus", 0x06F3D4, 0x0B),
        ("Event_DarkForce1", 0x06FAFA, 0x06),
        ("Event_Juza", 0x06FB42, 0x48),
        ("Event_SavingKyra", 0x070936, 0x26),
        ("Event_DarkForce2", 0x070978, 0x3A),
        ("Event_XeAThoulBeforeBattle", 0x070B32, 0x36),
        ("Event_AirCastleFakeChest", 0x070B58, 0x3C),
        ("Event_LashiecAppearance", 0x070E1E, 0x37),
        ("Cutscene_Alshline", 0x074502, 0x68),
        ("Cutscene_ProfoundDarkness", 0x078E9C, 0x03),
    ];

    fn retained_entries(scene: &Scene) -> Vec<u16> {
        scene
            .ops
            .iter()
            .filter_map(|op| match op {
                SceneOp::RunDialogue {
                    source: DialogueSource::Entry(DialogueId(entry)),
                    window: DialogueWindow::Retained,
                } => Some(*entry),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn every_dialogue2_caller_is_a_retained_window() {
        for (name, _, entry) in DIALOGUE2_CALLERS {
            let scene = SCENES
                .iter()
                .find(|scene| scene.name == *name)
                .unwrap_or_else(|| panic!("{name} is missing from the registry"));
            assert_eq!(
                retained_entries(scene),
                vec![*entry],
                "{name} runs `Event_GetAndRunDialogue2` with entry {entry:#X} exactly once"
            );
        }
        // No scene outside the table claims the routine, and the table has no
        // duplicate scene (each scene has exactly one caller in the image).
        for scene in SCENES {
            let listed = DIALOGUE2_CALLERS
                .iter()
                .filter(|(name, _, _)| *name == scene.name)
                .count();
            assert_eq!(
                retained_entries(scene).len(),
                listed,
                "{} uses the retained window without a table row (or the reverse)",
                scene.name
            );
        }
    }

    /// The H22 shape: `RunDialogueResume` reopens `Saved_Dialogue_Addr`, so a
    /// scene may only resume after it has run a dialogue itself. A scene whose
    /// first dialogue op is a resume reads whatever ran last in play, or
    /// nothing after a load.
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

    #[test]
    fn event_indexes_are_unique_and_findable() {
        for scene in SCENES {
            assert_eq!(
                scene_for(scene.event).map(|s| s.name),
                Some(scene.name),
                "{} should be findable by its event index",
                scene.name
            );
        }
        let mut seen: Vec<_> = SCENES.iter().map(|s| s.event).collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), before, "two scenes share an event index");
    }

    #[test]
    fn the_cutscene_is_the_only_one_with_bit_15_set() {
        let cutscenes: Vec<_> = SCENES
            .iter()
            .filter(|s| s.event.is_cutscene())
            .map(|s| s.name)
            .collect();
        assert_eq!(
            cutscenes,
            vec![
                "Cutscene_PiataPrincipal",
                "Cutscene_ProfHolt",
                "Cutscene_MeetingRune",
                "Cutscene_Dorin",
                "Cutscene_Alshline",
                "Cutscene_ZemaIgglanovaDefeated",
                "Cutscene_MeetingRika",
                "Cutscene_DemiRescue",
                "Cutscene_AlysWounded",
                "Cutscene_PsycoWand",
                "Cutscene_ZioDefeated",
                "Cutscene_MeetingWren",
                "Cutscene_InsideSpaceship",
                "Cutscene_SpaceshipSabotage",
                "Cutscene_CrashLaanding",
                "Cutscene_Landale",
                "Cutscene_DarkForce1Defeated",
                "Cutscene_LeRoofAgain",
                "Cutscene_MeetingKyra",
                "Cutscene_LutzRevelation",
                "Cutscene_DarkForce2Defeated",
                "Cutscene_MeetingSeth",
                "Cutscene_AeroPrism",
                "Cutscene_FindingAirCastle",
                "Cutscene_LashiecDefeated",
                "Cutscene_GumbiousBishop",
                "Cutscene_BeforeElsydeonCave",
                "Cutscene_Elsydeon",
                "Cutscene_Reunion",
                "Cutscene_ProfoundDarkness",
                "Cutscene_RajaSick",
                "Cutscene_Rykros",
                "Cutscene_Ending",
            ]
        );
    }
}
