//! `Event_GameStart` — retail `$073946..$073ECD`, 1416 bytes, 78 ops.
//!
//! The intro: Alys wakes Chaz at his house, they walk out through Aiedo, cross
//! the Motavia overworld, the AW 2284 prologue crawls over a title image, and
//! the game drops you on the academy's first floor in Piata. About as long as
//! every other opening-act scene put together.
//!
//! Doc: `docs/scenes/01_GameStart.md`. Doubly fork-damaged — the body is
//! `include`d away from an absent file, *and* its dialogue tree 17 is one the
//! fork rewrote wholesale (66 retail entries against the clone's 80). Every
//! line here came from the cartridge.
//!
//! # It tours five maps
//!
//! `$11` (title) -> `$5E` ChazHouse -> `$54` Aiedo -> `$00` Motavia -> `$13`
//! PiataAcademy_F1. Each [`SceneOp::LoadMap`] emits
//! `SceneEffect::MapRequested`, and the runtime must load the map and
//! `SceneRunner::recast` for it before ticking again — an [`ActorRef::Npc`]
//! index means nothing across a map change. This scene only ever drives
//! `Character_1`/`Character_2`, which survive the tour, but the cast still has
//! to be re-seated because their positions are rewritten by each load.
//!
//! # The prologue pages
//!
//! Phase 4 runs the same shape twice — draw four text entries, ramp the colour
//! up over 20 steps, hold, ramp down, pause — for entries `$3A..$3D` then
//! `$3E..$41`. The two passes differ only in the closing pause: 90 frames
//! (`$59 + 1`) then 120 (`$77 + 1`). Every `VInt_PrepareLoop` count below is
//! the corrected `d0 + 1`.

use super::opening::EventIndexes;
use super::{ALYS, CHAZ};
use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_runner::Scene;
use crate::state::Flag;

const LEADER: ActorRef = ActorRef::PartyMember(0);
const SECOND: ActorRef = ActorRef::PartyMember(1);

const TREE_17: u32 = 0x001E_BA90;
const MUSIC_STOP: u8 = 0xFB;
const MUSIC_MOTAVIA_TOWN: u8 = 0x84;
const MUSIC_FIELD_MOTAVIA: u8 = 0x85;

/// One prologue page: four text draws, a 20-step fade up, a 900-frame hold, a
/// 20-step fade down, and a closing pause.
macro_rules! prologue_page {
    ($first:expr, $pause:expr) => {
        [
            SceneOp::DrawTextToPlane {
                entry: $first,
                plane: 0x0000_9280,
                vram: 0x0000_9280,
            },
            SceneOp::DrawTextToPlane {
                entry: $first + 1,
                plane: 0x0000_9280,
                vram: 0x0000_9280,
            },
            SceneOp::DrawTextToPlane {
                entry: $first + 2,
                plane: 0x0000_9280,
                vram: 0x0000_9280,
            },
            SceneOp::DrawTextToPlane {
                entry: $first + 3,
                plane: 0x0000_9280,
                vram: 0x0000_9280,
            },
            SceneOp::IntroTextFadeUp,
            SceneOp::WaitFrames { frames: 900 },
            SceneOp::IntroTextFadeDown,
            SceneOp::WaitFrames { frames: $pause },
        ]
    };
}

static GAME_START_OPS: &[SceneOp] = &{
    const PAGE_1: [SceneOp; 8] = prologue_page!(0x3A, 90);
    const PAGE_2: [SceneOp; 8] = prologue_page!(0x3E, 120);
    [
        // --- phase 0: open on tree 17, entry $36 ---
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::SetDialogueTree { rom_addr: TREE_17 },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x36)),
            window: DialogueWindow::Standard,
        },
        SceneOp::FadeOut,
        // --- phase 1: ChazHouse ($5E) ---
        SceneOp::PlaySound {
            id: MUSIC_MOTAVIA_TOWN,
        },
        SceneOp::LoadMap {
            map: 0x5E,
            prev_map: 0x54,
            start_x: 0x44,
            start_y: 0x38,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0,
        },
        SceneOp::PlaceActor {
            actor: LEADER,
            x: 0x230,
            y: 0x1A0,
        },
        SceneOp::PlaceActor {
            actor: SECOND,
            x: 0x2A0,
            y: 0x240,
        },
        SceneOp::Wait { ticks: 1 },
        SceneOp::FadeIn,
        SceneOp::SetActorDest {
            actor: SECOND,
            x: 0x230,
            y: 0x250,
        },
        // bits 0 and 1: follow chain off, Y-first ordering on.
        SceneOp::SetFollowMode { bits: 0b11 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x230,
            y: 0x240,
            wait: true,
        },
        SceneOp::SetFollowMode { bits: 0 },
        // `$0100FFFF` as a long: Alys, Chaz, empty, empty.
        SceneOp::SetParty {
            slots: [Some(ALYS), Some(CHAZ), None, None, None],
        },
        // Three `trap #1` copies through the `$E200` scratch buffer.
        SceneOp::SwapCharSlots { a: 0, b: 1 },
        SceneOp::Wait { ticks: 40 },
        SceneOp::Face {
            actor: LEADER,
            facing: Direction::Up,
        },
        SceneOp::SetDialogueTree { rom_addr: TREE_17 },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x37)),
            window: DialogueWindow::Standard,
        },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1F0,
            y: 0x290,
            wait: true,
        },
        SceneOp::FadeOut,
        // --- phase 2: Aiedo ($54) ---
        SceneOp::LoadMap {
            map: 0x54,
            prev_map: 0x5E,
            start_x: 0x2C,
            start_y: 0x48,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0,
        },
        SceneOp::FadeIn,
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x160,
            y: 0x260,
            wait: true,
        },
        SceneOp::Face {
            actor: LEADER,
            facing: Direction::Right,
        },
        SceneOp::SetDialogueTree { rom_addr: TREE_17 },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x38)),
            window: DialogueWindow::Standard,
        },
        // The walk out of Aiedo: seven waypoints, each driven to completion.
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x180,
            y: 0x300,
            wait: true,
        },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x220,
            y: 0x3D0,
            wait: true,
        },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x2B0,
            y: 0x3D0,
            wait: true,
        },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x2B0,
            y: 0x420,
            wait: true,
        },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x2D0,
            y: 0x420,
            wait: true,
        },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x2D0,
            y: 0x440,
            wait: true,
        },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x300,
            y: 0x530,
            wait: true,
        },
        SceneOp::PlaySound { id: MUSIC_STOP },
        SceneOp::FadeOut,
        // --- phase 3: Motavia overworld ($00) ---
        SceneOp::PlaySound {
            id: MUSIC_FIELD_MOTAVIA,
        },
        SceneOp::LoadMap {
            map: 0x00,
            prev_map: 0x54,
            start_x: 0x4E,
            start_y: 0x74,
            facing: Direction::Right,
            align: 0x0C,
            clear_load_flags: 0,
        },
        SceneOp::FadeIn,
        SceneOp::SetDialogueTree { rom_addr: TREE_17 },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x39)),
            window: DialogueWindow::Standard,
        },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x2B0,
            y: 0x370,
            wait: true,
        },
        // bit 2: lock the camera for the walk off-screen.
        SceneOp::SetFollowMode { bits: 0b100 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x3B0,
            y: 0x370,
            wait: true,
        },
        SceneOp::SetFollowMode { bits: 0 },
        SceneOp::FadeOut,
        // --- phase 4: the AW 2284 prologue ---
        SceneOp::LoadPalette {
            rom_addr: 0x001D_2A3C,
            words: 64,
        },
        SceneOp::SetCameraPos { x: 0, y: 0 },
        SceneOp::LoadTitleImage {
            art: 0x001C_F1F2,
            mapping: 0x001D_25BE,
            width: 40,
            height: 16,
            vram: 0x0000_9280,
        },
        SceneOp::FadeIn,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::SetTextColour { colour: 0x0666 },
        PAGE_1[0],
        PAGE_1[1],
        PAGE_1[2],
        PAGE_1[3],
        PAGE_1[4],
        PAGE_1[5],
        PAGE_1[6],
        PAGE_1[7],
        PAGE_2[0],
        PAGE_2[1],
        PAGE_2[2],
        PAGE_2[3],
        PAGE_2[4],
        PAGE_2[5],
        PAGE_2[6],
        PAGE_2[7],
        SceneOp::FadeOut,
        SceneOp::PlaySound { id: MUSIC_STOP },
        SceneOp::WaitFrames { frames: 40 },
        // --- phase 5: hand over ---
        // `$00FFFFFF` as a long (slots 1-4) plus a separate byte clearing slot
        // 5: Chaz alone. One `SetParty` covers all five, so the doc's separate
        // `SetPartySlot{5, empty}` needs no second op here.
        SceneOp::SetParty {
            slots: [Some(CHAZ), None, None, None, None],
        },
        // The doc records `prev: none` — this scene does not write
        // Field_Map_Index_2. `LoadMap` has no way to say "leave it", so it
        // carries the map they came from; flagged in the report.
        SceneOp::LoadMap {
            map: 0x13,
            prev_map: 0x00,
            start_x: 0x60,
            start_y: 0x24,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0,
        },
        SceneOp::Wait { ticks: 30 },
        SceneOp::PlaySound {
            id: MUSIC_MOTAVIA_TOWN,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_MOTAVIA_TOWN,
        },
        SceneOp::FadeIn,
        SceneOp::SetFlag {
            flag: Flag::event(0x07),
            value: true,
        },
    ]
};

/// See the module docs.
pub static GAME_START: Scene = Scene {
    name: "Event_GameStart",
    event: EventIndexes::GAME_START,
    ops: GAME_START_OPS,
};
