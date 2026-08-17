//! Retail Dezo endgame scenes after Gumbious and through the final-battle gate.
//!
//! Long palette/object loops are retained as literal waits and presentation
//! records. The persistent edges are the reason these scenes are registered:
//! tower flags, the transient party-slot bridge, Elsydeon, Reunion and the
//! Profound Darkness battle handoff.

use super::{CHAZ, DEMI, GRYZ, HAHN, KYRA, RAJA};
use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);

const MUSIC_TOWER: u8 = 0x9A;
const MUSIC_TAKE_OFF_LANDALE: u8 = 0x9B;
const MUSIC_MACHINE_CENTER: u8 = 0x89;
const MUSIC_THE_AGE_OF_FABLES: u8 = 0xA1;
const MUSIC_BLACK_BLOOD: u8 = 0xA8;
const MUSIC_MYSTERY: u8 = 0xAB;
const SOUND_STOP_ALL: u8 = 0xFE;
const SFX_BARRIER_BROKEN: u8 = 0xE6;
const SFX_DOOR_OPENED: u8 = 0xE2;
const SFX_STAIRS: u8 = 0xDF;
const SFX_TELEPORT: u8 = 0xDE;

const ITEM_ELSYDEON: u8 = 0x77;
const ITEM_MAHLAY_DAGGER: u8 = 0x71;
const ITEM_MAHLAY_SHIELD: u8 = 0x88;
const ITEM_MAHLAY_RING: u8 = 0xA0;
const ITEM_MAHLAY_MAIL: u8 = 0x7C;
const ITEM_LACO_AXE: u8 = 0x78;
const ITEM_LACO_HELM: u8 = 0x68;
const ITEM_LACO_MAIL: u8 = 0x67;
const ITEM_SONIC_BUSTER: u8 = 0x79;
const ITEM_LACO_GEAR: u8 = 0x70;
const ITEM_LACO_ARMOR: u8 = 0x6F;
const ITEM_LACO_ROD: u8 = 0x62;
const ITEM_LACO_CIRCLET: u8 = 0x6A;
const ITEM_REFLECT_ROBE: u8 = 0x59;
const ITEM_LACO_SLASHER: u8 = 0x5D;
const ITEM_MOON_SLASHER: u8 = 0x65;
const ITEM_LACO_CROWN: u8 = 0x69;

const fn entry(id: u16) -> DialogueSource {
    DialogueSource::Entry(DialogueId(id))
}

const fn standard(id: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: entry(id),
        window: DialogueWindow::Standard,
    }
}

const fn cutscene(id: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: entry(id),
        window: DialogueWindow::Cutscene,
    }
}

/// `$005E`, `Event_StrengthTowerTop`, `$070E4E..$071101`.
pub static STRENGTH_TOWER_TOP: Scene = Scene {
    name: "Event_StrengthTowerTop",
    event: EventIndex(0x005E),
    ops: &[
        SceneOp::MoveCamera {
            x: 0x1F0,
            y: 0x1C8,
            speed: 1,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::ObjectAnimation {
            slot: 5,
            object_id: 0x2AC,
            art_tile: 0,
            frames: 15,
        },
        SceneOp::ObjectAnimation {
            slot: 6,
            object_id: 0x2AC,
            art_tile: 0,
            frames: 15,
        },
        SceneOp::ObjectAnimation {
            slot: 7,
            object_id: 0x2AC,
            art_tile: 0,
            frames: 15,
        },
        SceneOp::ObjectAnimation {
            slot: 8,
            object_id: 0x2AC,
            art_tile: 0,
            frames: 15,
        },
        SceneOp::Wait { ticks: 39 },
        SceneOp::PlaySound { id: 0xC2 },
        SceneOp::Wait { ticks: 48 },
        SceneOp::PlaySound { id: 0xC3 },
        SceneOp::Wait { ticks: 8 },
        SceneOp::PlaySound { id: 0xB9 },
        SceneOp::Wait { ticks: 60 },
        SceneOp::MoveCamera {
            x: 0x1F0,
            y: 0x270,
            speed: 8,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0xE2),
            value: true,
        },
    ],
};

/// `$005F`, `Event_CourageTowerTop`, `$071102..$071449`.
pub static COURAGE_TOWER_TOP: Scene = Scene {
    name: "Event_CourageTowerTop",
    event: EventIndex(0x005F),
    ops: &[
        SceneOp::Face {
            actor: LEADER,
            facing: Direction::Up,
        },
        SceneOp::MoveCamera {
            x: 0x1F0,
            y: 0x1C0,
            speed: 1,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::ObjectAnimation {
            slot: 5,
            object_id: 0x2BC,
            art_tile: 0,
            frames: 2,
        },
        SceneOp::ObjectAnimation {
            slot: 6,
            object_id: 0x2BC,
            art_tile: 0,
            frames: 2,
        },
        SceneOp::ObjectAnimation {
            slot: 7,
            object_id: 0x2BC,
            art_tile: 0,
            frames: 2,
        },
        SceneOp::ObjectAnimation {
            slot: 8,
            object_id: 0x2BC,
            art_tile: 0,
            frames: 2,
        },
        SceneOp::PlaySound { id: 0xBE },
        SceneOp::Wait { ticks: 15 },
        SceneOp::ObjectAnimation {
            slot: 5,
            object_id: 0x2B4,
            art_tile: 0,
            frames: 40,
        },
        SceneOp::ObjectAnimation {
            slot: 6,
            object_id: 0x2B4,
            art_tile: 0,
            frames: 40,
        },
        SceneOp::PlaySound { id: 0xCD },
        SceneOp::Wait { ticks: 40 },
        SceneOp::PlaySound { id: 0xC5 },
        SceneOp::Wait { ticks: 8 },
        SceneOp::PlaySound { id: 0xB9 },
        SceneOp::Wait { ticks: 30 },
        SceneOp::SetFlag {
            flag: Flag::event(0xE3),
            value: true,
        },
    ],
};

/// `$0060`, `Event_DeVars`, `$07144A..$071469`.
pub static DE_VARS: Scene = Scene {
    name: "Event_DeVars",
    event: EventIndex(0x0060),
    ops: &[
        standard(3),
        SceneOp::SetFlag {
            flag: Flag::event(0xD4),
            value: true,
        },
        SceneOp::StartBattle { index: 0x16 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$0061`, `Event_SaLews`, `$07146A..$071489`.
pub static SA_LEWS: Scene = Scene {
    name: "Event_SaLews",
    event: EventIndex(0x0061),
    ops: &[
        standard(1),
        SceneOp::SetFlag {
            flag: Flag::event(0xD2),
            value: true,
        },
        SceneOp::StartBattle { index: 0x17 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$0063`, `Event_ReFaze`, `$07157A..$071759`.
pub static REFAZE: Scene = Scene {
    name: "Event_ReFaze",
    event: EventIndex(0x0063),
    ops: &[
        standard(9),
        SceneOp::Wait { ticks: 40 },
        SceneOp::Face {
            actor: LEADER,
            facing: Direction::Right,
        },
        SceneOp::Wait { ticks: 20 },
        SceneOp::Face {
            actor: LEADER,
            facing: Direction::Left,
        },
        SceneOp::Wait { ticks: 20 },
        SceneOp::Face {
            actor: LEADER,
            facing: Direction::Up,
        },
        SceneOp::PlaySound { id: 0xFB },
        SceneOp::FadeOut,
        SceneOp::Wait { ticks: 60 },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x2C0,
            art_tile: 0,
            frames: 63,
        },
        SceneOp::Wait { ticks: 8 },
        SceneOp::PlaySound { id: 0xC2 },
        SceneOp::Wait { ticks: 64 },
        SceneOp::PlaySound { id: 0x9F },
        SceneOp::RunDialogueResume,
        SceneOp::BranchChoice {
            if_yes: 18,
            if_no: 23,
        },
        SceneOp::Jump { to: 18 },
        SceneOp::PlaySound { id: 0xC0 },
        SceneOp::SetSavedMusic { id: MUSIC_TOWER },
        SceneOp::PlaySound { id: MUSIC_TOWER },
        SceneOp::SetFlag {
            flag: Flag::event(0xE1),
            value: true,
        },
        SceneOp::Return { value: 0 },
        SceneOp::SetFlag {
            flag: Flag::event(0xE1),
            value: true,
        },
        SceneOp::StartBattle { index: 0x19 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$0064`, `Event_DeVarsDefeated`, `$07175A..$071821`.
pub static DE_VARS_DEFEATED: Scene = Scene {
    name: "Event_DeVarsDefeated",
    event: EventIndex(0x0064),
    ops: &[
        standard(4),
        SceneOp::FadeOut,
        SceneOp::Wait { ticks: 40 },
        SceneOp::ObjectAnimation {
            slot: 0,
            object_id: 0,
            art_tile: 0,
            frames: 10,
        },
        SceneOp::PlaySound {
            id: SFX_BARRIER_BROKEN,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0xE5),
            value: true,
        },
    ],
};

/// `$0065`, `Event_SaLewsDefeated`, `$071822..$0718E5`.
pub static SA_LEWS_DEFEATED: Scene = Scene {
    name: "Event_SaLewsDefeated",
    event: EventIndex(0x0065),
    ops: &[
        standard(2),
        SceneOp::FadeOut,
        SceneOp::Wait { ticks: 40 },
        SceneOp::ObjectAnimation {
            slot: 0,
            object_id: 0,
            art_tile: 0,
            frames: 10,
        },
        SceneOp::PlaySound {
            id: SFX_BARRIER_BROKEN,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0xE6),
            value: true,
        },
    ],
};

/// `$801D`, `Cutscene_BeforeElsydeonCave`, `$0784B6..$078583`.
pub static BEFORE_ELSYDEON_CAVE: Scene = Scene {
    name: "Cutscene_BeforeElsydeonCave",
    event: EventIndex(0x801D),
    ops: &[
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1E0,
            y: 0x200,
            wait: true,
        },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1D0,
            y: 0x1C0,
            wait: true,
        },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1E0,
            y: 0x190,
            wait: true,
        },
        SceneOp::InitVramAndCram,
        SceneOp::PlaySound {
            id: SFX_DOOR_OPENED,
        },
        SceneOp::Wait { ticks: 120 },
        SceneOp::PanelCreate { id: 0x18E },
        SceneOp::DmaPlanes,
        SceneOp::FadeIn,
        cutscene(0x30),
        SceneOp::SavePartySlots,
        SceneOp::SetParty {
            slots: [Some(CHAZ), None, None, None, None],
        },
        SceneOp::RecoverStats,
        SceneOp::SetFlag {
            flag: Flag::event(0xD8),
            value: true,
        },
        SceneOp::LoadMap {
            map: 0x16F,
            prev_map: 0xFFFF,
            start_x: 0x3C,
            start_y: 0x32,
            facing: Direction::Up,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// `$801E`, `Cutscene_Elsydeon`, `$078584..$078A7B`.
pub static ELSYDEON: Scene = Scene {
    name: "Cutscene_Elsydeon",
    event: EventIndex(0x801E),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x129 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 160 },
        cutscene(0x31),
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PanelCreate { id: 0x12B },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x12C },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PlaySound {
            id: MUSIC_THE_AGE_OF_FABLES,
        },
        SceneOp::PanelDestroyAll,
        SceneOp::DmaPlanes,
        SceneOp::PanelCreate { id: 0x12E },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::RunDialogueResume,
        SceneOp::PanelCreate { id: 0x12F },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 240 },
        SceneOp::PanelDestroyAll,
        SceneOp::DmaPlanes,
        SceneOp::PanelCreate { id: 0x130 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::RunDialogueResume,
        SceneOp::PanelDestroyAll,
        SceneOp::DmaPlanes,
        SceneOp::InitVramAndCram,
        SceneOp::Wait { ticks: 60 },
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x139 },
        SceneOp::DmaPlanes,
        SceneOp::PanelCreate { id: 0x13A },
        SceneOp::DmaPlanes,
        SceneOp::PanelCreate { id: 0x13B },
        SceneOp::DmaPlanes,
        cutscene(0x32),
        SceneOp::SetFlag {
            flag: Flag::event(0xD9),
            value: true,
        },
        SceneOp::SetCharacterEquipment {
            who: CHAZ,
            slots: [Some(ITEM_ELSYDEON), Some(0), None, None],
        },
        SceneOp::RestorePartySlots,
        SceneOp::LoadMap {
            map: 0x16F,
            prev_map: 0xFFFF,
            start_x: 0x3C,
            start_y: 0x32,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// `$801F`, `Cutscene_Reunion`, `$078A7C..$078D2F`.
pub static REUNION: Scene = Scene {
    name: "Cutscene_Reunion",
    event: EventIndex(0x801F),
    ops: &[
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1E0,
            y: 0x1C0,
            wait: true,
        },
        SceneOp::Presentation {
            op: PresentationOp::SetObjectDestination {
                slot: 0,
                x: 0x1E0,
                y: 0x1B0,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::SetObjectDestination {
                slot: 1,
                x: 0x1F0,
                y: 0x1B0,
            },
        },
        SceneOp::Wait { ticks: 40 },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x13D },
        SceneOp::DmaPlanes,
        cutscene(0),
        SceneOp::Wait { ticks: 60 },
        SceneOp::PlaySound {
            id: MUSIC_TAKE_OFF_LANDALE,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::InitVramAndCram,
        SceneOp::LoadMap {
            map: 0x0BF,
            prev_map: 0xFFFF,
            start_x: 0x3C,
            start_y: 0x24,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaySound {
            id: MUSIC_MACHINE_CENTER,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_MACHINE_CENTER,
        },
        SceneOp::FadeIn,
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1E0,
            y: 0x190,
            wait: true,
        },
        SceneOp::Wait { ticks: 40 },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x13F },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x13E },
        SceneOp::DmaPlanes,
        cutscene(1),
        SceneOp::SetFlag {
            flag: Flag::event(0xDA),
            value: true,
        },
        SceneOp::LoadMap {
            map: 0x0BF,
            prev_map: 0xFFFF,
            start_x: 0x3C,
            start_y: 0x36,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::ConfigureCharacter {
            who: HAHN,
            equipment: [
                ITEM_MAHLAY_DAGGER,
                ITEM_MAHLAY_SHIELD,
                ITEM_MAHLAY_RING,
                ITEM_MAHLAY_MAIL,
            ],
            restore_hp_tp: false,
        },
        SceneOp::ConfigureCharacter {
            who: GRYZ,
            equipment: [ITEM_LACO_AXE, 0, ITEM_LACO_HELM, ITEM_LACO_MAIL],
            restore_hp_tp: false,
        },
        SceneOp::ConfigureCharacter {
            who: DEMI,
            equipment: [ITEM_SONIC_BUSTER, 0, ITEM_LACO_GEAR, ITEM_LACO_ARMOR],
            restore_hp_tp: false,
        },
        SceneOp::ConfigureCharacter {
            who: RAJA,
            equipment: [ITEM_LACO_ROD, 0, ITEM_LACO_CIRCLET, ITEM_REFLECT_ROBE],
            restore_hp_tp: false,
        },
        SceneOp::ConfigureCharacter {
            who: KYRA,
            equipment: [
                ITEM_LACO_SLASHER,
                ITEM_MOON_SLASHER,
                ITEM_LACO_CROWN,
                ITEM_REFLECT_ROBE,
            ],
            restore_hp_tp: false,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// `$0069`, `Event_AngerTowerTop`, `$0721CC..$072261`.
pub static ANGER_TOWER_TOP: Scene = Scene {
    name: "Event_AngerTowerTop",
    event: EventIndex(0x0069),
    ops: &[
        SceneOp::SavePartySlots,
        SceneOp::SetParty {
            slots: [Some(CHAZ), None, None, None, None],
        },
        SceneOp::RecoverStats,
        SceneOp::PlaySound { id: SFX_STAIRS },
        SceneOp::FadeOut,
        SceneOp::LoadMap {
            map: 0xFE,
            prev_map: 0xFD,
            start_x: 0x38,
            start_y: 0x4C,
            facing: Direction::Up,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::MoveCamera {
            x: 0x1F0,
            y: 0x1D0,
            speed: 8,
        },
        SceneOp::FadeIn,
        SceneOp::PlaySound { id: MUSIC_MYSTERY },
        SceneOp::Wait { ticks: 60 },
        SceneOp::MoveCamera {
            x: 0x1C0,
            y: 0x260,
            speed: 2,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// `$006A`, `Event_AngerTowerExitTop`, `$072262..$0722D1`.
pub static ANGER_TOWER_EXIT_TOP: Scene = Scene {
    name: "Event_AngerTowerExitTop",
    event: EventIndex(0x006A),
    ops: &[
        SceneOp::RestorePartySlots,
        SceneOp::PlaySound { id: SFX_STAIRS },
        SceneOp::FadeOut,
        SceneOp::LoadMap {
            map: 0xFD,
            prev_map: 0xFE,
            start_x: 0x38,
            start_y: 0x48,
            facing: Direction::Up,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::FadeIn,
        SceneOp::BranchFlag {
            flag: Flag::event(0xE1),
            if_set: 8,
            if_clear: 6,
        },
        SceneOp::PlaySound { id: MUSIC_TOWER },
        SceneOp::Return { value: 0 },
        SceneOp::SetFlag {
            flag: Flag::event(0xE7),
            value: true,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// `$8020`, `Cutscene_ProfoundDarkness`, `$078D30..$078F3D`.
pub static PROFOUND_DARKNESS: Scene = Scene {
    name: "Cutscene_ProfoundDarkness",
    event: EventIndex(0x8020),
    ops: &[
        SceneOp::PlaySound { id: SFX_TELEPORT },
        SceneOp::FadeOut,
        SceneOp::InitVramAndCram,
        SceneOp::LoadArt {
            rom_addr: 0x001D_D566,
            tile: 0x20E,
        },
        SceneOp::ObjectAnimation {
            slot: 0,
            object_id: 0,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 2,
            object_id: 0,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 3,
            object_id: 0,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 4,
            object_id: 0,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::DmaPlanes,
        SceneOp::FadeIn,
        SceneOp::PlaySound { id: SOUND_STOP_ALL },
        SceneOp::Wait { ticks: 600 },
        SceneOp::PlaySound {
            id: MUSIC_BLACK_BLOOD,
        },
        standard(3),
        SceneOp::SetFlag {
            flag: Flag::event(0xE8),
            value: true,
        },
        SceneOp::SetSavedMusic { id: SOUND_STOP_ALL },
        SceneOp::SetMapLoadFlags {
            set: 0x88,
            clear: 0,
        },
        SceneOp::StartBattle { index: 0x1A },
        SceneOp::Return { value: 0 },
    ],
};
