//! Retail-backed scenes after the Zema/Tonoe chain.
//!
//! These are the four scenes whose retail pointer ranges are present in the
//! cartridge and whose `grand_cross=0` bodies survive in the clone. Fortune
//! Teller and After Fortune Teller are intentionally not here: their retail
//! event-pointer slots do not exist, and the clone only includes Grand Cross
//! source for them.

use super::RIKA;
use crate::geom::Direction;
use crate::scene::{ActorRef, Axis, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::{PresentationAsset, PresentationOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);
const NPC_0: ActorRef = ActorRef::Npc(0);
const NPC_1: ActorRef = ActorRef::Npc(1);
const RIKA_ACTOR: ActorRef = ActorRef::Character(RIKA);

const TREE_34: u32 = 0x001F7190;

const MUSIC_STOP: u8 = 0xFB;
const MUSIC_MOTABIA_TOWN: u8 = 0x84;
const MUSIC_FIELD_MOTABIA: u8 = 0x8C;
const MUSIC_DUNGEON_ARRANGE_1: u8 = 0x91;
const MUSIC_FAL: u8 = 0x92;
const MUSIC_EXPLOSION: u8 = 0xAD;
const SFX_ALARM: u8 = 0xDB;
const SFX_DOOR_OPENED: u8 = 0xE2;
const SFX_RADAR: u8 = 0xF8;
const SOUND_STOP_SPC: u8 = 0xFD;
const SOUND_STOP_ALL: u8 = 0xFE;

/// `Event_BioPlantAlarm`, EventPtrs[$12].
pub static BIO_PLANT_ALARM: Scene = Scene {
    name: "Event_BioPlantAlarm",
    event: EventIndex(0x0012),
    ops: &[
        SceneOp::PlaySound { id: SFX_ALARM },
        SceneOp::Presentation {
            op: PresentationOp::FadeToRed { lines: 3 },
        },
        SceneOp::Presentation {
            op: PresentationOp::FadeFromRed { lines: 3 },
        },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x0A)),
            window: DialogueWindow::Standard,
        },
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
        SceneOp::SetFlag {
            flag: Flag::temp(0x08),
            value: true,
        },
    ],
};

/// `Event_GirlsSneakingOut`, EventPtrs[$23].
pub static GIRLS_SNEAKING_OUT: Scene = Scene {
    name: "Event_GirlsSneakingOut",
    event: EventIndex(0x0023),
    ops: &[
        SceneOp::WaitFrames { frames: 31 },
        SceneOp::Presentation {
            op: PresentationOp::WindowDestroy { render_mode: 1 },
        },
        SceneOp::Presentation {
            op: PresentationOp::WindowDestroy { render_mode: 1 },
        },
        SceneOp::Presentation {
            op: PresentationOp::WindowDestroy { render_mode: 1 },
        },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::Presentation {
            op: PresentationOp::SavePartySpriteX { parked_x: 0x65 },
        },
        SceneOp::Presentation {
            op: PresentationOp::SetPaletteWords {
                offset: 0x44,
                first: 0x0AAA,
                second: 0x0666,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::SetPaletteWords {
                offset: 0x5C,
                first: 0x0620,
                second: 0x0EEE,
            },
        },
        SceneOp::FadeIn,
        SceneOp::PlaySound {
            id: SFX_DOOR_OPENED,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::Presentation {
            op: PresentationOp::SetGameMode { mode: 0x0C },
        },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x3E)),
            window: DialogueWindow::Standard,
        },
        SceneOp::FadeOut,
        SceneOp::Presentation {
            op: PresentationOp::SetGameMode { mode: 0x18 },
        },
        SceneOp::Presentation {
            op: PresentationOp::RestorePartySpriteX,
        },
        SceneOp::Presentation {
            op: PresentationOp::ClearHeldInput,
        },
        SceneOp::DespawnNpc {
            npc_index: 3,
            count: 2,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::Presentation {
            op: PresentationOp::WindowCreate { render_mode: 1 },
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadWindowTiles {
                group: 0,
                vram: 0xC000,
                priority: 1,
                asset: PresentationAsset::WinTilesMeseta,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::WindowCreate { render_mode: 1 },
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadPortrait {
                asset: PresentationAsset::ShopkeeperDialPortrait2,
                tile: 0x55C,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::DrawPortrait {
                mapping_rom_addr: 0x002A2B36,
                x: 6,
                y: 7,
                width: 6,
                height: 6,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::WindowCreate { render_mode: 1 },
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x46),
            value: true,
        },
    ],
};

/// `Event_ChazHouse`, EventPtrs[$3B].
pub static CHAZ_HOUSE: Scene = Scene {
    name: "Event_ChazHouse",
    event: EventIndex(0x003B),
    ops: &[
        SceneOp::BranchFlag {
            flag: Flag::event(0x42),
            if_set: 1,
            if_clear: 2,
        },
        SceneOp::Presentation {
            op: PresentationOp::SetDialoguePortrait {
                entry: 0x31,
                portrait: 1,
            },
        },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x31)),
            window: DialogueWindow::Standard,
        },
        SceneOp::BranchChoice {
            if_yes: 4,
            if_no: 12,
        },
        SceneOp::PlaySound { id: MUSIC_STOP },
        SceneOp::FadeOut,
        SceneOp::RecoverStats,
        SceneOp::LoadMap {
            map: 0x5E,
            prev_map: 0x54,
            start_x: 0x2E,
            start_y: 0x44,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::FadeIn,
        SceneOp::PlaySound {
            id: MUSIC_MOTABIA_TOWN,
        },
        SceneOp::SetFlag {
            flag: Flag::temp(0x18),
            value: true,
        },
        SceneOp::SetFlag {
            flag: Flag::temp(0x18),
            value: true,
        },
    ],
};

/// `Event_LeavingChazHouse`, EventPtrs[$3C].
pub static LEAVING_CHAZ_HOUSE: Scene = Scene {
    name: "Event_LeavingChazHouse",
    event: EventIndex(0x003C),
    ops: &[SceneOp::SetFlag {
        flag: Flag::temp(0x18),
        value: false,
    }],
};

/// `Cutscene_MeetingRika`, CutscenePtrs[$07].
pub static MEETING_RIKA: Scene = Scene {
    name: "Cutscene_MeetingRika",
    event: EventIndex(0x8007),
    ops: &[
        // Initial object alignment: copy the leader's X to the script NPC.
        SceneOp::BranchIfAligned {
            a: LEADER,
            b: NPC_0,
            axis: Axis::X,
            if_aligned: 3,
            if_not: 1,
        },
        SceneOp::MoveActorToActorAxis {
            actor: NPC_0,
            target: LEADER,
            axis: Axis::X,
            wait: true,
        },
        SceneOp::Face {
            actor: NPC_0,
            facing: Direction::Down,
        },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::PlaySound {
            id: MUSIC_DUNGEON_ARRANGE_1,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_DUNGEON_ARRANGE_1,
        },
        SceneOp::SetMapLoadFlags {
            set: 0x08,
            clear: 0,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x18,
            art_tile: 0x26A,
            frames: 1,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::FadeIn,
        SceneOp::MoveActorTo {
            actor: NPC_1,
            x: 0x210,
            y: 0x140,
            wait: true,
        },
        SceneOp::SetStepOffset { value: 0 },
        SceneOp::MoveActorTo {
            actor: NPC_0,
            x: 0x200,
            y: 0x140,
            wait: true,
        },
        SceneOp::SetStepOffset { value: 1 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x200,
            y: 0x160,
            wait: true,
        },
        SceneOp::FadeOut,
        SceneOp::LoadMap {
            map: 0xAD,
            prev_map: 0xAC,
            start_x: 0x3C,
            start_y: 0x4C,
            facing: Direction::Up,
            align: 4,
            clear_load_flags: 0x08,
        },
        SceneOp::FadeIn,
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1E0,
            y: 0x160,
            wait: true,
        },
        SceneOp::Wait { ticks: 30 },
        SceneOp::SetRenderSpritesInCutscene { enabled: false },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(1)),
            window: DialogueWindow::Standard,
        },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x33 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::PanelCreate { id: 0x34 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResume,
        SceneOp::SetMapLoadFlags {
            set: 0x08,
            clear: 0,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::FadeIn,
        SceneOp::SetFollowMode { bits: 0x04 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1F0,
            y: 0x240,
            wait: true,
        },
        SceneOp::MoveActorTo {
            actor: NPC_0,
            x: 0x1E0,
            y: 0x240,
            wait: true,
        },
        SceneOp::SetStepOffset { value: 0 },
        SceneOp::MoveActorTo {
            actor: NPC_1,
            x: 0x1F0,
            y: 0x170,
            wait: true,
        },
        SceneOp::SetStepOffset { value: 1 },
        SceneOp::Wait { ticks: 30 },
        SceneOp::Face {
            actor: NPC_1,
            facing: Direction::Right,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: NPC_1,
            facing: Direction::Up,
        },
        SceneOp::Wait { ticks: 120 },
        SceneOp::MoveActorTo {
            actor: NPC_1,
            x: 0x1F0,
            y: 0x240,
            wait: true,
        },
        SceneOp::PlaySound { id: MUSIC_STOP },
        SceneOp::Wait { ticks: 30 },
        SceneOp::SetRenderSpritesInCutscene { enabled: false },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(2)),
            window: DialogueWindow::Standard,
        },
        SceneOp::PlaySound { id: SFX_RADAR },
        SceneOp::Wait { ticks: 60 },
        SceneOp::Presentation {
            op: PresentationOp::SetPaletteWords {
                offset: 0x18,
                first: 0x000E,
                second: 0x000E,
            },
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::FadeOut,
        SceneOp::PlaySound { id: SOUND_STOP_SPC },
        SceneOp::JoinParty { slot: 4, who: RIKA },
        SceneOp::ObjectAnimation {
            slot: 5,
            object_id: 0x18,
            art_tile: 0x55C,
            frames: 1,
        },
        SceneOp::Presentation {
            op: PresentationOp::AddMacro { slot: 5 },
        },
        SceneOp::DespawnNpc {
            npc_index: 1,
            count: 1,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x34),
            value: true,
        },
        SceneOp::LoadMap {
            map: 0x24,
            prev_map: 0,
            start_x: 0x3C,
            start_y: 0x14,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaySound {
            id: MUSIC_MOTABIA_TOWN,
        },
        SceneOp::LoadArt {
            rom_addr: 0x001289FA,
            tile: 0x4A5,
        },
        SceneOp::ObjectAnimation {
            slot: 7,
            object_id: 0x194,
            art_tile: 0x4A5,
            frames: 1,
        },
        SceneOp::FadeIn,
        SceneOp::Presentation {
            op: PresentationOp::SetObjectDestination {
                slot: 7,
                x: 0x1F0,
                y: 0xF0,
            },
        },
        SceneOp::SetFollowMode { bits: 0 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1E0,
            y: 0xF0,
            wait: true,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::MoveActorOffset {
            actor: RIKA_ACTOR,
            dx: 0x10,
            dy: 0x10,
            wait: true,
        },
        SceneOp::Face {
            actor: RIKA_ACTOR,
            facing: Direction::Right,
        },
        SceneOp::Wait { ticks: 20 },
        SceneOp::Face {
            actor: RIKA_ACTOR,
            facing: Direction::Left,
        },
        SceneOp::Wait { ticks: 20 },
        SceneOp::Face {
            actor: RIKA_ACTOR,
            facing: Direction::Down,
        },
        SceneOp::InitVramAndCram,
        SceneOp::SetDialogueTree { rom_addr: TREE_34 },
        SceneOp::PlaySound { id: MUSIC_FAL },
        SceneOp::FadeIn,
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(3)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PlaySound { id: SOUND_STOP_ALL },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::PlaySound {
            id: MUSIC_EXPLOSION,
        },
        SceneOp::PanelCreate { id: 0x3B },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::PanelCreate { id: 0x3C },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(4)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x35),
            value: true,
        },
        SceneOp::LoadMap {
            map: 0,
            prev_map: 0x24,
            start_x: 0xC6,
            start_y: 0xA4,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaySound {
            id: MUSIC_FIELD_MOTABIA,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_FIELD_MOTABIA,
        },
        SceneOp::FadeIn,
        SceneOp::Return { value: 1 },
    ],
};
