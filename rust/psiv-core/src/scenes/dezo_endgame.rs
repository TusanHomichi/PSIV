//! Retail Dezo endgame scenes after Gumbious and through the final-battle gate.
//!
//! Long palette/object loops are retained as literal waits and presentation
//! records. The persistent edges are the reason these scenes are registered:
//! tower flags, the transient party-slot bridge, Elsydeon, Reunion and the
//! Profound Darkness battle handoff.

use super::{CHAZ, DEMI, GRYZ, HAHN, KYRA, RAJA, retained};
use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::{PaletteHold, PresentationOp};
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
/// `MusicID_RyucrossField` (`ps4.constants.asm:933`).
const MUSIC_RYUCROSS_FIELD: u8 = 0x90;
/// `Sound_StopMusic` (`ps4.constants.asm:1055`).
const SOUND_STOP_MUSIC: u8 = 0xFB;
const SOUND_STOP_ALL: u8 = 0xFE;
const SFX_BARRIER_BROKEN: u8 = 0xE6;
const SFX_DOOR_OPENED: u8 = 0xE2;
const SFX_STAIRS: u8 = 0xDF;
const SFX_TELEPORT: u8 = 0xDE;
/// `SFXID_Deban` (`ps4.constants.asm:1016`). `Cutscene_Elsydeon` plays it
/// under `if revision>0` (`ps4.asm:157483`), between the `$31` dialogue's
/// wait and the `$12B` panel.
const SFX_DEBAN: u8 = 0xDC;

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
        SceneOp::PlaySound { id: SFX_DEBAN },
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
        // `move.b #0, (World_Index).w` (`ps4.asm:157791`)
        SceneOp::SetWorldIndex { world: 0 },
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

/// `$0062`, `Event_AngerTowerAlys`, `$07148A..$071579` (`ps4.asm:150697-150768`;
/// record `docs/scenes/114_AngerTowerAlys.md`): tree 41 entry 15's `$F6`. The
/// illusion (object 0) speaks, turns to Chaz, steps eight pixels at him while
/// he backs off sixteen and steps eight more, then the retained line 8, Alys
/// Fight `$E4`, Chaz set at `$1F0,$1F0` facing down, and battle `$18`.
pub static ANGER_TOWER_ALYS: Scene = Scene {
    name: "Event_AngerTowerAlys",
    event: EventIndex(0x0062),
    ops: &[
        // $07148A: `moveq #7, d0` / `jsr Event_GetAndRunDialogue`.
        standard(7),
        // $071492..$0714A7: object 0 faces opposite the leader.
        SceneOp::FaceOppositeOf {
            actor: ActorRef::Npc(0),
            of: LEADER,
        },
        // $0714A8..$0714D5: `move.w #7, d2` / `jsr Event_StepObject`, the
        // step read from `loc_71572` by object 0's facing.
        SceneOp::StepObjectAlongFacing {
            actor: ActorRef::Npc(0),
            backwards: false,
            frames: 8,
        },
        // $0714D6..$071507: the leader's own facing, `neg.w` on both, `#$F`.
        SceneOp::StepObjectAlongFacing {
            actor: LEADER,
            backwards: true,
            frames: 16,
        },
        // $071508..$071535: object 0 again, eight frames.
        SceneOp::StepObjectAlongFacing {
            actor: ActorRef::Npc(0),
            backwards: false,
            frames: 8,
        },
        // $071536: `moveq #8, d0` / `jsr Event_GetAndRunDialogue2`.
        retained(8),
        // $07153E: `move.b #EventFlag_AlysFight, d0` / `jsr EventFlags_Set`.
        SceneOp::SetFlag {
            flag: Flag::event(0xE4),
            value: true,
        },
        // $071548..$07155D: `move.w #$1F0` to `$30` and `$34` of Character_1,
        // then `move.w #0, $6(a4)`: a raw facing write (down), no
        // `Event_UpdateObjFacing` call.
        SceneOp::PlaceActor {
            actor: LEADER,
            x: 0x1F0,
            y: 0x1F0,
        },
        SceneOp::Face {
            actor: LEADER,
            facing: Direction::Down,
        },
        // $07155E: `bset #3, (Map_Load_Flags).w`.
        SceneOp::SetMapLoadFlags {
            set: 0x08,
            clear: 0,
        },
        // $071564..$071570: battle `$18`, `bset #3, Routine_Exit_Flags`, `rts`.
        SceneOp::StartBattle { index: 0x18 },
    ],
};

/// `$0099`, `Event_EnterAngerTower`, `$0737BE..$07387F` (`ps4.asm:153370-153411`;
/// record `docs/scenes/115_EnterAngerTower.md`): `RunEvent_EnterAngerTower`
/// (trigger `$56`) fires it while Anger Tower `$E0` is clear. Without Elsydeon
/// (`$D9`) the barrier throws the party back to Rykros; with it the barrier
/// breaks (the palette whitens, the third line held) and `$E0` is set.
pub static ENTER_ANGER_TOWER: Scene = Scene {
    name: "Event_EnterAngerTower",
    event: EventIndex(0x0099),
    ops: &[
        // $0737BE..$0737CB: `move.b #EventFlag_Elsydeon, d0` / test / `bne.w loc_73820`.
        SceneOp::BranchFlag {
            flag: Flag::event(0xD9),
            if_set: 8,
            if_clear: 1,
        },
        // $0737CC..$0737E1: dialogue 5, stop the music, fade out.
        standard(5),
        SceneOp::PlaySound {
            id: SOUND_STOP_MUSIC,
        },
        SceneOp::FadeOut,
        // $0737E2..$073811: Rykros (`$2`) from the Anger Tower (`$FC`) at
        // `$44,$C2`, facing 0, alignment 0, `bclr #3, Map_Load_Flags`,
        // `jsr RefreshMap`.
        SceneOp::LoadMap {
            map: 0x002,
            prev_map: 0xFC,
            start_x: 0x44,
            start_y: 0xC2,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        // $073812..$07381F: the Rykros field music, `jmp Pal_FadeIn`.
        SceneOp::PlaySound {
            id: MUSIC_RYUCROSS_FIELD,
        },
        SceneOp::FadeIn,
        SceneOp::End,
        // loc_73820: dialogue 6 and `SFXID_BarrierBroken`.
        standard(6),
        SceneOp::PlaySound {
            id: SFX_BARRIER_BROKEN,
        },
        // $073830..$073855: `moveq #$27, d7`; per frame `Pal_IncreaseTone`,
        // `Pal_Init_Line_3` (`$296300`) copied over `Palette_Line_3`
        // (`$FFFFFB40`, sixteen words), `VInt_Prepare`.
        SceneOp::Presentation {
            op: PresentationOp::PaletteIncreaseTone {
                frames: 40,
                hold: Some(PaletteHold {
                    source_rom_addr: 0x0029_6300,
                    destination_ram: 0xFFFF_FB40,
                    words: 16,
                }),
            },
        },
        SceneOp::WaitFrames { frames: 40 },
        // $073856..$073867: `moveq #$27, d7`; per frame `loc_58666` steps all
        // 64 words back toward `Palette_Table_Buffer_2`, then `VInt_Prepare`.
        SceneOp::Presentation {
            op: PresentationOp::PaletteToneFlash {
                brighten: false,
                words: 64,
                frames: 40,
            },
        },
        SceneOp::WaitFrames { frames: 40 },
        // $073868..$073875: `popdlg` / `jsr Event_RunDialogue`.
        SceneOp::RunDialogueResume,
        // $073876: `move.b #EventFlag_AngerTower, d0` / `jmp EventFlags_Set`.
        SceneOp::SetFlag {
            flag: Flag::event(0xE0),
            value: true,
        },
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
        retained(3),
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
