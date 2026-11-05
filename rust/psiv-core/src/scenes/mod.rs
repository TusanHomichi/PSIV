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
//! | `Cutscene_Ending` | `$8021` | `89_Ending.md` | 370 |
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

pub(crate) mod aiedo_events;
mod bioplant;
pub(crate) mod census_events;
mod conveyors;
pub(crate) mod dezo_campaign;
pub(crate) mod dezo_endgame;
pub(crate) mod dezolis_route;
pub(crate) mod esper_events;
mod field_reload;
pub mod fifth_character;
mod flight;
mod game_start;
pub(crate) mod garuberk_events;
pub(crate) mod gumbious_events;
pub(crate) mod home_decor;
pub mod hunters_guild;
pub(crate) mod kadary_events;
pub(crate) mod late_arc;
pub(crate) mod missing_student;
pub(crate) mod monsen;
pub(crate) mod monsen_quake;
pub(crate) mod motavia_side;
pub(crate) mod next_arc;
pub(crate) mod next_arc_followup;
pub(crate) mod opening;
pub(crate) mod plate_wreckage;
pub(crate) mod post_rika_cutscenes;
pub(crate) mod post_rika_events;
pub(crate) mod post_zio_cutscenes;
pub(crate) mod ranch;
pub(crate) mod recovery;
pub(crate) mod retail_endgame;
pub(crate) mod rocky;
pub(crate) mod soldiers_temple;
pub(crate) mod strip_club;
mod vahal_common;
pub(crate) mod vahal_fort;
pub(crate) mod vehicles;
pub(crate) mod weapon_plant;
pub(crate) mod wreckage;
pub(crate) mod zosa_events;

pub use field_reload::{field_reload_is_proxy, field_reload_rows};
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
    dezolis_route::OUTSIDE_RAJA_TEMPLE,
    dezolis_route::TYLER_GRAVE_OPENING,
    dezolis_route::GYUNA,
    census_events::CANCELLER_REMINDER,
    census_events::MILE_SAND_WORM_BATTLE,
    census_events::FRACT_OOZE_FOUND,
    census_events::KING_RAPPY,
    census_events::DAUGHTER_TERMINAL,
    // Vahal Fort and the Weapon Plant (#82).
    vahal_fort::VAHAL_FORT_ENTRANCE,
    vahal_fort::VAHAL_FORT_MIDWAY,
    vahal_fort::VAHAL_FORT_BARRIER,
    vahal_fort::VAH_FORT_MOVING_PLATFORM_1,
    vahal_fort::VAH_FORT_MOVING_PLATFORM_2,
    vahal_fort::VAHAL_FORT_TERMINAL,
    vahal_fort::DOMINATORS_DEFEATED,
    vahal_fort::POSI_BOLT,
    weapon_plant::WEAPON_PLANT_ARRIVAL,
    weapon_plant::WPN_PLNT_MOVING_PLATFORM_1,
    weapon_plant::WPN_PLNT_MOVING_PLATFORM_2,
    weapon_plant::WPN_PLNT_MOVING_PLATFORM_3,
    weapon_plant::WPN_PLNT_MOVING_PLATFORM_4,
    weapon_plant::WEAPON_PLANT_TERMINAL,
    weapon_plant::BURSTROC,
    conveyors::CONVEYOR_BELT_DOWN,
    conveyors::CONVEYOR_BELT_UP,
    conveyors::CONVEYOR_BELT_RIGHT,
    conveyors::CONVEYOR_BELT_LEFT,
    recovery::RECOVERY,
    garuberk_events::DOOR_OPENING_1,
    garuberk_events::DOOR_OPENING_2,
    garuberk_events::DOOR_ENTERED_1,
    garuberk_events::DOOR_ENTERED_2,
    garuberk_events::EYE_ACTION_1,
    garuberk_events::EYE_ACTION_2,
    soldiers_temple::CAVE_DIALOGUE_1,
    soldiers_temple::CAVE_DIALOGUE_2,
    soldiers_temple::TEMPLE_REACHED,
    soldiers_temple::AERO_PRISM_FOUND,
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
    esper_events::ESPER_GUARD_PERMISSION,
    esper_events::PERSISTENT_ESPER_GUARDS,
    esper_events::INNER_SANCT_GUARD,
    esper_events::INNER_SANCT_GUARD_BEFORE_ELSYDEON,
    dezo_campaign::ECLIPSE_TORCH_USED,
    dezo_campaign::DARK_FORCE_2,
    dezo_campaign::LUTZ_REVELATION,
    gumbious_events::ECLIPSE_TORCH_STOLEN,
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
    dezo_endgame::SILENCE_TM_DOOR_OPENING,
    dezo_endgame::ENTER_ANGER_TOWER,
    dezo_endgame::ANGER_TOWER_TOP,
    dezo_endgame::ANGER_TOWER_ALYS,
    dezo_endgame::ANGER_TOWER_EXIT_TOP,
    dezo_endgame::PROFOUND_DARKNESS,
    retail_endgame::RAJA_SICK,
    retail_endgame::RYKROS,
    retail_endgame::ENDING,
    // Aiedo and the Hunter's Guild (#81).
    aiedo_events::ROCKY_OWNER,
    aiedo_events::GIRL_PRISON,
    aiedo_events::GIRLS_BAIL,
    aiedo_events::PAYING_GIRLS_BAIL,
    aiedo_events::GIRLS_BAILED_OUT,
    aiedo_events::AIEDO_MAN_SAVINGS,
    aiedo_events::AIEDO_MAN_CRUSH_ON_ALYS,
    hunters_guild::HUNTERS_GUILD,
    late_arc::MUSK_CATS_GUARDING,
    late_arc::MUSK_CAT_ELDER,
    late_arc::PENGUIN_OWNER,
    late_arc::RAJA_SICK,
    late_arc::RUNE_HEALING_CHAZ,
    late_arc::HIJAMMER,
    fifth_character::PICKING_FIFTH_CHARACTER,
    motavia_side::LOST_GIRLS_MOTHER,
    motavia_side::KING_RAPPY_DEFEATED,
    motavia_side::MEESE_CLINIC_SICK_WOMAN,
    motavia_side::DYING_BOY_FATHER,
    motavia_side::FATHER_AFTER_BOY_RECOVERY,
    motavia_side::MEETING_SEKREAS,
    strip_club::STRIPPERS_APPEARING,
    strip_club::EXITING_STRIP_CLUB,
    wreckage::WRECKAGE_ENGINE,
    plate_wreckage::PHONON,
    plate_wreckage::PLATE_SYS_ENGINE,
    home_decor::PENNANT,
    home_decor::WOOD_CARVING,
    // The earthquake at Monsen (#81).
    monsen_quake::MONSEN_EARTHQUAKE,
    // The ranch owner (#81).
    ranch::RANCH_OWNER,
    ranch::RANCH_OWNER_AFTER_BATTLE,
    // Rocky (#81).
    rocky::ROCKY_FOUND,
    rocky::CATCHING_ROCKY,
    // The guild's missing student, Piata and Kadary (#81).
    missing_student::PIATA_DORM_OWNER,
    missing_student::MISSING_STUDENT_FOUND,
    missing_student::OLD_MAN_NEAR_MISSING_STUDENT,
    missing_student::MISSING_STUDENT_IN_BED,
    // Tallas's rescue (#81).
    monsen::TALLAS_MOM,
    monsen::TALLAS_MOM_AFTER_RESCUE,
    monsen::INSIDE_MONSEN_HOLE,
    monsen::TALLAS_RESCUED,
    // Zosa's item shop (#56).
    zosa_events::PENGUIN_FEED_STOLEN,
    // Kadary's priest and believer (#81).
    kadary_events::FAINTING_PRIEST,
    kadary_events::ZIO_FANATIC,
    // The field menu's own events, not story beats: the three machines board
    // from the ITEM menu (`ItemActionPtrs`, `ps4.asm:123355`; the three
    // boarding actions, `ps4.asm:123419-123465`).
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
mod tests;
