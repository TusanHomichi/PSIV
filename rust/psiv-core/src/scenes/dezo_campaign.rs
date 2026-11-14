//! Retail Dezo campaign scenes from the Le Roof gate through Gumbious.
//!
//! The pointer ranges for these records are in `docs/scenes/51_*` onward.
//! Renderer-only palette/panel/object choreography stays in typed presentation
//! ops; party, map, inventory, flag and battle writes stay explicit here.

use super::flight::with_ship_menu;
use super::{KYRA, cutscene3, retained};
use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::{CounterCycle, PaletteBuffer, PaletteSource, PresentationOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);
const KYRA_ACTOR: ActorRef = ActorRef::Character(KYRA);

const TREE_37: u32 = 0x001F_9580;

const MUSIC_DEZOLIS_FIELD: u8 = 0x99;
const MUSIC_TOWER: u8 = 0x9A;
const MUSIC_LAND_MASTER: u8 = 0x8D;
const MUSIC_FAL: u8 = 0x92;
const MUSIC_RED_ALERT: u8 = 0xA9;
const SOUND_STOP_SPC: u8 = 0xFD;
const SOUND_STOP_ALL: u8 = 0xFE;
const SFX_DEBAN: u8 = 0xDC;
const SFX_STAIRS: u8 = 0xDF;

/// `loc_70638` (`$070638`): the Le Roof scenes' colour cycle, words 11 and 12
/// (`$16(a1)`, `$18(a1)`) from the two eleven-word series at `loc_70672`.
const LE_ROOF_CYCLE: CounterCycle = CounterCycle {
    source_rom_addr: 0x0007_0672,
    offset: 11,
    entries: 11,
};

/// `lea loc_1DE258, a0` / `lea Palette_Table_Buffer_2, a1` / `moveq #$F, d7`
/// / `trap #1`: Le Roof's sixteen words, words 0-15 of the copy
/// `Pal_VariableFadeToPal2` fades up to (`$0704D8`, `$078398`).
const LE_ROOF_PALETTE: SceneOp = SceneOp::Presentation {
    op: PresentationOp::PaletteCopy {
        source: PaletteSource::Rom(0x001D_E258),
        destination: PaletteBuffer::Saved,
        start: 0,
        words: 16,
    },
};

/// `lea Palette_2_Line_2, a0` / `moveq #7, d7` / `trap #0`: eight longwords
/// of zero, words 16-31 of the copy (`$0704E6`, `$0783A6`).
const LE_ROOF_LINE_2_CLEAR: SceneOp = SceneOp::Presentation {
    op: PresentationOp::PaletteCopy {
        source: PaletteSource::Fill(0),
        destination: PaletteBuffer::Saved,
        start: 16,
        words: 16,
    },
};

/// `lea Pal_SilenceTm, a0` / `lea Palette_Table_Buffer_2, a1` / `move.w #$1F,
/// d7` / `trap #1` (`$0705A8`): words 0-31 of Le Roof Room's map palette, the
/// field colours `Event_MeetingLeRoof` fades back up to before it returns.
const SILENCE_TM_PALETTE: SceneOp = SceneOp::Presentation {
    op: PresentationOp::PaletteCopy {
        source: PaletteSource::Rom(0x0018_711A),
        destination: PaletteBuffer::Saved,
        start: 0,
        words: 32,
    },
};

/// A Le Roof colour loop: `frames` passes of `loc_70638` and `VInt_Prepare`.
const fn le_roof_cycle(frames: u16) -> SceneOp {
    SceneOp::Presentation {
        op: PresentationOp::CounterPaletteCycle {
            cycle: LE_ROOF_CYCLE,
            frames,
        },
    }
}

/// `loc_705D6` (`$0705D6`): 71 passes (`moveq #$46, d7`) lighting words 2-10
/// from `loc_70626`, each with a `loc_70638` frame and `VInt_Prepare`.
const LE_ROOF_FADE_IN: SceneOp = SceneOp::Presentation {
    op: PresentationOp::PaletteGrowIn {
        source_rom_addr: 0x0007_0626,
        offset: 2,
        words: 9,
        passes: 71,
        cycle: LE_ROOF_CYCLE,
    },
};

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

/// `$0048`, `Event_MeetingLeRoof`, `$070482..$07069D`.
pub static MEETING_LE_ROOF: Scene = Scene {
    name: "Event_MeetingLeRoof",
    event: EventIndex(0x0048),
    ops: &[
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1F0,
            y: 0x1E0,
            wait: true,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::Presentation {
            op: PresentationOp::TwoLinePaletteFade {
                fade_in: false,
                mode: 2,
            },
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::LoadArt {
            rom_addr: 0x001D_B2C8,
            tile: 0x260,
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x001D_DE0A,
                destination_ram: 0xFFFF_0000,
            },
        },
        LE_ROOF_PALETTE,
        LE_ROOF_LINE_2_CLEAR,
        SceneOp::DmaPlanes,
        SceneOp::PlaySound { id: 0xA1 },
        SceneOp::Presentation {
            op: PresentationOp::TwoLinePaletteFade {
                fade_in: true,
                mode: 2,
            },
        },
        // `$070534..$07057F`: the colour loops (`move.w #$B3, d0` and
        // `#$77` twice), `bsr loc_705D6` after the first, and dialogue `0`
        // between the other two. `VInt_Prepare` only: no map update.
        le_roof_cycle(180),
        LE_ROOF_FADE_IN,
        le_roof_cycle(120),
        standard(0),
        le_roof_cycle(120),
        SceneOp::PlaySound { id: 0xFB },
        // `$070590`: `Pal_VariableFadeOut_Lines_1_2` takes the cinematic
        // colours (ramp, cycle words and the black second line) to black.
        SceneOp::Presentation {
            op: PresentationOp::TwoLinePaletteFade {
                fade_in: false,
                mode: 2,
            },
        },
        // `$070596..$0705A7`: `RefreshPlaneFG`, `RefreshPlaneBG`,
        // `DMAPlanes_VInt`.
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
        // `$0705A8..$0705C3`: the room's own palette, `Pal_SilenceTm`, back
        // into `Palette_Table_Buffer_2`, and `Pal_VariableFadeToPal2` up to it.
        SILENCE_TM_PALETTE,
        SceneOp::Presentation {
            op: PresentationOp::TwoLinePaletteFade {
                fade_in: true,
                mode: 2,
            },
        },
        SceneOp::PlaySound { id: MUSIC_TOWER },
        SceneOp::SetFlag {
            flag: Flag::event(0xD1),
            value: true,
        },
    ],
};

/// `$801C`, `Cutscene_LeRoofAgain`, `$078346..$0784B5`.
pub static LE_ROOF_AGAIN: Scene = Scene {
    name: "Cutscene_LeRoofAgain",
    event: EventIndex(0x801C),
    ops: with_ship_menu![
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1F0,
            y: 0x1F0,
            wait: true,
        },
        SceneOp::Presentation {
            op: PresentationOp::TwoLinePaletteFade {
                fade_in: false,
                mode: 2,
            },
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::LoadArt {
            rom_addr: 0x001D_B2C8,
            tile: 0x260,
        },
        LE_ROOF_PALETTE,
        LE_ROOF_LINE_2_CLEAR,
        SceneOp::DmaPlanes,
        SceneOp::PlaySound { id: 0xA1 },
        SceneOp::Presentation {
            op: PresentationOp::TwoLinePaletteFade {
                fade_in: true,
                mode: 2,
            },
        },
        // `$0783EE..$078420`: `Event_MeetingLeRoof`'s colour loops, 180
        // passes, `jsr loc_705D6` at `$078406`, then 120.
        le_roof_cycle(180),
        LE_ROOF_FADE_IN,
        le_roof_cycle(120),
        standard(0x0B),
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x11B },
        SceneOp::DmaPlanes,
        // `$078454`: `popdlg` / `jsr Event_RunDialogue3`: the panels stay.
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Cutscene3,
        },
        SceneOp::FadeOut,
        // `$07846C`: `moveq #$3B, d0` / `jsr DoMapUpdateLoop`.
        SceneOp::Wait { ticks: 60 },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x125 },
        SceneOp::DmaPlanes,
        cutscene(0x0C),
        SceneOp::SetFlag {
            flag: Flag::event(0xD6),
            value: true,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0xD7),
            value: true,
        },
    ],
};

/// `$004C`, `Event_CarnivorousTrees`, `$070774..$070855`.
pub static CARNIVOROUS_TREES: Scene = Scene {
    name: "Event_CarnivorousTrees",
    event: EventIndex(0x004C),
    ops: &[
        SceneOp::SetSavedMusic {
            id: MUSIC_DEZOLIS_FIELD,
        },
        SceneOp::BranchIfVehicle {
            if_mounted: 2,
            if_on_foot: 7,
        },
        SceneOp::PlaySound { id: SOUND_STOP_SPC },
        SceneOp::Wait { ticks: 1 },
        SceneOp::PlaySound {
            id: MUSIC_RED_ALERT,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::SetVehicleIndex { index: 0 },
        SceneOp::Jump { to: 8 },
        SceneOp::SetDialogueTree { rom_addr: TREE_37 },
        standard(0x34),
        // `$070826..$070847`: the leader's Y plus `$20`, written to every
        // party object; no walk.
        SceneOp::PlacePartyY { dy: 0x20 },
        SceneOp::StartBattle { index: 0x0A },
        SceneOp::Return { value: 1 },
    ],
};

/// `$004D`, `Event_SavingKyra`, `$070856..$070975`.
pub static SAVING_KYRA: Scene = Scene {
    name: "Event_SavingKyra",
    event: EventIndex(0x004D),
    ops: &[
        SceneOp::BranchIfVehicle {
            if_mounted: 2,
            // `tst.w Vehicle_Index / beq.w loc_70908` (`ps4.asm:149987-149988`):
            // on foot skips the dismount and dialogue `$34` and lands on the
            // red-alert music, op 9.
            if_on_foot: 9,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_DEZOLIS_FIELD,
        },
        SceneOp::PlaySound { id: SOUND_STOP_SPC },
        SceneOp::Wait { ticks: 1 },
        SceneOp::PlaySound { id: 0xFB },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::SetVehicleIndex { index: 0 },
        SceneOp::SetDialogueTree { rom_addr: TREE_37 },
        standard(0x34),
        SceneOp::PlaySound {
            id: MUSIC_RED_ALERT,
        },
        SceneOp::Wait { ticks: 1 },
        SceneOp::PanelCreate { id: 0x98 },
        SceneOp::DmaPlanes,
        SceneOp::SetDialogueTree { rom_addr: TREE_37 },
        retained(0x26),
        // `$07093C..$07095D`: the leader's Y plus `$20`, written to every
        // party object; no walk.
        SceneOp::PlacePartyY { dy: 0x20 },
        SceneOp::SetFlag {
            flag: Flag::event(0x95),
            value: true,
        },
        SceneOp::StartBattle { index: 0x0A },
        SceneOp::Return { value: 1 },
    ],
};

/// `$8013`, `Cutscene_MeetingKyra`, `$077788..$077895`.
pub static MEETING_KYRA: Scene = Scene {
    name: "Cutscene_MeetingKyra",
    event: EventIndex(0x8013),
    ops: &[
        SceneOp::PanelCreate { id: 0x140 },
        SceneOp::DmaPlanes,
        SceneOp::SetDialogueTree { rom_addr: TREE_37 },
        // `$0777A6`: `jsr Event_GetAndRunDialogue`.
        standard(0x27),
        SceneOp::InitVramAndCram,
        SceneOp::PlaySound { id: MUSIC_FAL },
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x99 },
        SceneOp::DmaPlanes,
        // `$0777D8`: `jsr Event_GetAndRunDialogue3`.
        cutscene3(0x28),
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x9B },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        // `$077810`: `popdlg` / `jsr Event_RunDialogue5`, the `Get5` exit.
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Cutscene5,
        },
        SceneOp::JoinParty { slot: 4, who: KYRA },
        SceneOp::Presentation {
            op: PresentationOp::AddMacro { slot: 9 },
        },
        SceneOp::SetFlag {
            flag: Flag::event(0xA0),
            value: true,
        },
        SceneOp::LoadMap {
            map: 0x001,
            prev_map: 0xFFFF,
            start_x: 0x174,
            start_y: 0x1C,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaySound {
            id: MUSIC_LAND_MASTER,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_LAND_MASTER,
        },
        SceneOp::SetVehicleIndex { index: 2 },
        SceneOp::SetMapLoadFlags { set: 1, clear: 0 },
        SceneOp::Return { value: 0 },
    ],
};

/// `$0047`, `Event_EclipseTorchUsed`, `$07018A..$070481`.
pub static ECLIPSE_TORCH_USED: Scene = Scene {
    name: "Event_EclipseTorchUsed",
    event: EventIndex(0x0047),
    ops: &[
        SceneOp::BranchIfVehicle {
            if_mounted: 1,
            if_on_foot: 7,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_DEZOLIS_FIELD,
        },
        SceneOp::PlaySound { id: SOUND_STOP_SPC },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::SetVehicleIndex { index: 0 },
        SceneOp::Wait { ticks: 1 },
        SceneOp::Jump { to: 7 },
        SceneOp::PanelCreate { id: 0x10D },
        SceneOp::DmaPlanes,
        SceneOp::Wait { ticks: 120 },
        SceneOp::PlaySound { id: SFX_DEBAN },
        SceneOp::Wait { ticks: 40 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::LoadArt {
            rom_addr: 0x001D_712C,
            tile: 0x3AF,
        },
        SceneOp::ObjectAnimation {
            slot: 0,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 2,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 3,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 4,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 5,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 6,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 7,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 8,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::SetDialogueTree { rom_addr: TREE_37 },
        standard(0x2A),
        SceneOp::SetFlag {
            flag: Flag::event(0x9C),
            value: true,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// `$004E`, `Event_DarkForce2`, `$070976..$0709A1`.
pub static DARK_FORCE_2: Scene = Scene {
    name: "Event_DarkForce2",
    event: EventIndex(0x004E),
    ops: &[
        retained(0x3A),
        SceneOp::SetFlag {
            flag: Flag::event(0x9E),
            value: true,
        },
        SceneOp::SetSavedMusic { id: SOUND_STOP_ALL },
        SceneOp::SetMapLoadFlags {
            set: 0x80,
            clear: 0,
        },
        SceneOp::StartBattle { index: 0x11 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$8014`, `Cutscene_LutzRevelation`, `$077896..$077A2D`.
pub static LUTZ_REVELATION: Scene = Scene {
    name: "Cutscene_LutzRevelation",
    event: EventIndex(0x8014),
    ops: &[
        standard(0x2A),
        SceneOp::Wait { ticks: 30 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x200,
            y: 0x1E0,
            wait: true,
        },
        SceneOp::PlaySound { id: SFX_STAIRS },
        SceneOp::FadeOut,
        SceneOp::LoadMap {
            map: 0x16F,
            prev_map: 0x16E,
            start_x: 0x3C,
            start_y: 0x4C,
            facing: Direction::Left,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::FadeIn,
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1E0,
            y: 0x1F0,
            wait: true,
        },
        SceneOp::MoveActorOffset {
            actor: KYRA_ACTOR,
            dx: 0x10,
            dy: 0,
            wait: true,
        },
        SceneOp::Face {
            actor: KYRA_ACTOR,
            facing: Direction::Up,
        },
        SceneOp::Wait { ticks: 40 },
        SceneOp::Face {
            actor: KYRA_ACTOR,
            facing: Direction::Right,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: KYRA_ACTOR,
            facing: Direction::Left,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: KYRA_ACTOR,
            facing: Direction::Right,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: KYRA_ACTOR,
            facing: Direction::Up,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::MoveActorTo {
            actor: KYRA_ACTOR,
            x: 0x1F0,
            y: 0x1F0,
            wait: true,
        },
        standard(0x2F),
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x9E },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 100 },
        // `$0779EA`: `popdlg` / `jsr Event_RunDialogue5`, the `Get5` exit.
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Cutscene5,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x97),
            value: true,
        },
        SceneOp::LoadMap {
            map: 0x16F,
            prev_map: 0x16E,
            start_x: 0x3C,
            start_y: 0x48,
            facing: Direction::Down,
            align: 8,
            clear_load_flags: 0x08,
        },
        SceneOp::Return { value: 0 },
    ],
};

#[path = "dezo_campaign_late.rs"]
mod dezo_campaign_late;
pub use dezo_campaign_late::*;
