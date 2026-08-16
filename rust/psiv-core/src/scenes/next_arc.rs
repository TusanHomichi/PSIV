//! Retail transcriptions for the act after Piata: Zema, Tonoe and Birth
//! Valley.
//!
//! The companion documents in `docs/scenes/12_*.md` through
//! `docs/scenes/25_*.md` carry the byte ranges and the retail/Grand Cross
//! audit. This module deliberately keeps the low-level panel and temporary
//! object work as data ops even though the headless runtime does not own a VDP
//! panel allocator. The actual field walks use ordinary `MoveActorTo` ops so
//! their start and arrival edges reach `FieldMap`.

use super::{GRYZ, RUNE};
use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);
const NPC_0: ActorRef = ActorRef::Npc(0);
const NPC_2: ActorRef = ActorRef::Npc(2);
const NPC_4: ActorRef = ActorRef::Npc(4);
const NPC_6: ActorRef = ActorRef::Npc(6);
const GRYZ_ACTOR: ActorRef = ActorRef::Character(GRYZ);

const TREE_3: u32 = 0x001E0BA0;
const TREE_4: u32 = 0x001E1980;

const MUSIC_STOP: u8 = 0xFB;
const MUSIC_TONOE_DE_PON: u8 = 0x81;
const MUSIC_MOTABIA_VILLAGE: u8 = 0x83;
const MUSIC_MOTABIA_TOWN: u8 = 0x84;
const MUSIC_JIJY_NO_RAG: u8 = 0xA6;
const MUSIC_THRAY: u8 = 0x94;
const MUSIC_TERRIBLE_SIGHT: u8 = 0x97;
const MUSIC_HAPPY_SETTLEMENT: u8 = 0x9E;
const SFX_MEGID: u8 = 0xC0;
const SFX_FUSION: u8 = 0xD9;
const SFX_DOOR_OPENED: u8 = 0xE2;
const SFX_BARRIER_BROKEN: u8 = 0xE6;

/// Scene event values beyond the opening act. The comments preserve the
/// pointer table they came from: ordinary values index `EventPtrs[$5A2B4]`,
/// cutscenes index `CutscenePtrs[$5A580]` after masking bit 15.
pub struct ArcEventIndexes;

impl ArcEventIndexes {
    /// `Event_MeetingSaya`, EventPtrs[$0D].
    pub const MEETING_SAYA: EventIndex = EventIndex(0x000D);
    /// `Event_TonoeBasementDoor`, EventPtrs[$33].
    pub const TONOE_BASEMENT_DOOR: EventIndex = EventIndex(0x0033);
    /// `Event_MeetingDorin`, EventPtrs[$32].
    pub const MEETING_DORIN: EventIndex = EventIndex(0x0032);
    /// `Event_RuneFlaeli`, EventPtrs[$27].
    pub const RUNE_FLAELI: EventIndex = EventIndex(0x0027);
    /// `Event_AlshlineFound`, EventPtrs[$28].
    pub const ALSHLINE_FOUND: EventIndex = EventIndex(0x0028);
    /// `Event_ZemaServantBattle`, EventPtrs[$8A].
    pub const ZEMA_SERVANT_BATTLE: EventIndex = EventIndex(0x008A);
    /// `Event_ZemaOldMan`, EventPtrs[$8B].
    pub const ZEMA_OLD_MAN: EventIndex = EventIndex(0x008B);
    /// `Event_ZemaOldManAfterMission`, EventPtrs[$8C].
    pub const ZEMA_OLD_MAN_AFTER_MISSION: EventIndex = EventIndex(0x008C);
    /// `Cutscene_ProfHolt`, CutscenePtrs[$02].
    pub const PROF_HOLT: EventIndex = EventIndex(0x8002);
    /// `Cutscene_MeetingRune`, CutscenePtrs[$03].
    pub const MEETING_RUNE: EventIndex = EventIndex(0x8003);
    /// `Cutscene_Dorin`, CutscenePtrs[$04].
    pub const DORIN: EventIndex = EventIndex(0x8004);
    /// `Cutscene_Alshline`, CutscenePtrs[$05].
    pub const ALSHLINE: EventIndex = EventIndex(0x8005);
    /// `Cutscene_ZemaIgglanovaDefeated`, CutscenePtrs[$06].
    pub const ZEMA_IGGLANOVA_DEFEATED: EventIndex = EventIndex(0x8006);
}

/// `Cutscene_ProfHolt` — retail `$073F22..$073F9B`, 122 bytes, 13 ops.
static PROF_HOLT_OPS: &[SceneOp] = &[
    SceneOp::InitVramAndCram,
    SceneOp::FadeIn,
    SceneOp::PlaySound { id: 0xAB }, // Music_Mystery
    SceneOp::SetRenderSpritesInCutscene { enabled: true },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x66)),
        window: DialogueWindow::Cutscene,
    },
    SceneOp::PlaySound { id: MUSIC_STOP },
    SceneOp::LoadMap {
        map: 0x24,
        prev_map: 0x2B,
        start_x: 0x3C,
        start_y: 0x14,
        facing: Direction::Down,
        align: 0,
        clear_load_flags: 0,
    },
    SceneOp::FadeIn,
    SceneOp::PlaySound {
        id: MUSIC_TERRIBLE_SIGHT,
    },
    SceneOp::SetSavedMusic {
        id: MUSIC_TERRIBLE_SIGHT,
    },
    SceneOp::AddMoney { amount: 500 },
    SceneOp::SetFlag {
        flag: Flag::event(0x10),
        value: true,
    },
    SceneOp::Return { value: 1 },
];

/// The professor's petrification scene.
pub static PROF_HOLT: Scene = Scene {
    name: "Cutscene_ProfHolt",
    event: ArcEventIndexes::PROF_HOLT,
    ops: PROF_HOLT_OPS,
};

/// `Cutscene_MeetingRune` — retail `$073F9C..$074033`, 152 bytes, 13 ops.
static MEETING_RUNE_OPS: &[SceneOp] = &[
    SceneOp::FaceOppositeOf {
        actor: NPC_4,
        of: LEADER,
    },
    SceneOp::InitVramAndCram,
    SceneOp::PlaySound { id: MUSIC_THRAY },
    SceneOp::FadeIn,
    SceneOp::SetRenderSpritesInCutscene { enabled: true },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(3)),
        window: DialogueWindow::Cutscene,
    },
    SceneOp::PromoteNpcToChar {
        npc: 4,
        char_id: RUNE,
        slot: 1,
        art_tile: 0x54C,
        facing: Direction::Down,
    },
    SceneOp::DespawnNpc {
        npc_index: 4,
        count: 1,
    },
    SceneOp::JoinParty { slot: 1, who: RUNE },
    SceneOp::SetFlag {
        flag: Flag::event(0x11),
        value: true,
    },
    SceneOp::PlaySound { id: MUSIC_STOP },
    SceneOp::SetSavedMusic { id: 0 },
    SceneOp::Return { value: 0 },
];

/// Rune's first recruitment scene.
pub static MEETING_RUNE: Scene = Scene {
    name: "Cutscene_MeetingRune",
    event: ArcEventIndexes::MEETING_RUNE,
    ops: MEETING_RUNE_OPS,
};

/// `Event_MeetingDorin` — retail `$06EEBE..$06F1A3`, 742 bytes, 20 ops.
static MEETING_DORIN_OPS: &[SceneOp] = &[
    SceneOp::PlaySound { id: MUSIC_STOP },
    SceneOp::Wait { ticks: 10 },
    SceneOp::PlaySound {
        id: MUSIC_JIJY_NO_RAG,
    },
    SceneOp::FaceOppositeOf {
        actor: NPC_0,
        of: LEADER,
    },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x18)),
        window: DialogueWindow::Standard,
    },
    // C480/C540/C4C0 are temporary punch, anger-line and startled-Dorin
    // objects. Their frame/keyframe work is kept as literal presentation ops.
    SceneOp::ObjectAnimation {
        slot: 7,
        object_id: 0x1A4,
        art_tile: 0x4A5,
        frames: 0x82,
    },
    SceneOp::ObjectAnimation {
        slot: 9,
        object_id: 0x1A8,
        art_tile: 0,
        frames: 0x50,
    },
    SceneOp::ObjectAnimation {
        slot: 8,
        object_id: 0x1AC,
        art_tile: 0,
        frames: 0xA,
    },
    SceneOp::MoveActorTo {
        actor: NPC_0,
        x: 0x200,
        y: 0x1E0,
        wait: true,
    },
    SceneOp::Face {
        actor: NPC_0,
        facing: Direction::Left,
    },
    SceneOp::PlaySound { id: 0xBE }, // SFX_Foi at the punch keyframe
    SceneOp::WaitFrames { frames: 130 },
    SceneOp::RunDialogueResume,
    SceneOp::Face {
        actor: NPC_0,
        facing: Direction::Right,
    },
    SceneOp::RunDialogueResume,
    SceneOp::ObjectAnimation {
        slot: 9,
        object_id: 0,
        art_tile: 0,
        frames: 1,
    },
    SceneOp::ObjectAnimation {
        slot: 8,
        object_id: 0,
        art_tile: 0,
        frames: 1,
    },
    SceneOp::RunDialogueResume,
    SceneOp::SetFlag {
        flag: Flag::event(0x36),
        value: true,
    },
    SceneOp::Return { value: 1 },
];

/// Dorin's pre-cutscene conversation. It sets the trigger flag consumed by
/// `RunEvent_Dorin` (`$14`), while `Cutscene_Dorin` replaces Rune with Gryz.
pub static MEETING_DORIN: Scene = Scene {
    name: "Event_MeetingDorin",
    event: ArcEventIndexes::MEETING_DORIN,
    ops: MEETING_DORIN_OPS,
};

/// `Cutscene_Dorin` — retail `$074034..$0741E5`, 434 bytes, 17 ops.
static DORIN_OPS: &[SceneOp] = &[
    SceneOp::InitVramAndCram,
    SceneOp::FadeIn,
    SceneOp::SetRenderSpritesInCutscene { enabled: true },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x25)),
        window: DialogueWindow::Cutscene,
    },
    // Retail sets bit 3 before RefreshMap rather than clearing it.
    SceneOp::SetMapLoadFlags {
        set: 0x08,
        clear: 0,
    },
    SceneOp::LoadMap {
        map: 0x43,
        prev_map: 0x43,
        start_x: 0x3C,
        start_y: 0x3C,
        facing: Direction::Down,
        align: 0,
        clear_load_flags: 0,
    },
    SceneOp::PromoteNpcToChar {
        npc: 2,
        char_id: GRYZ,
        slot: 1,
        art_tile: 0x534,
        facing: Direction::Down,
    },
    SceneOp::DespawnNpc {
        npc_index: 2,
        count: 1,
    },
    SceneOp::Wait { ticks: 20 },
    SceneOp::PlaySound {
        id: MUSIC_TONOE_DE_PON,
    },
    // C480 is the invisible Rune object; C300 is Dorin's secondary object.
    // Their two destinations are the comparator-visible movement landing.
    SceneOp::MoveActorTo {
        actor: NPC_6,
        x: 0x1F0,
        y: 0x300,
        wait: false,
    },
    SceneOp::MoveActorTo {
        actor: NPC_0,
        x: 0x1E0,
        y: 0x300,
        wait: false,
    },
    SceneOp::WaitForActor { actor: NPC_6 },
    SceneOp::WaitForActor { actor: NPC_0 },
    SceneOp::MoveCamera {
        x: 0x1E0,
        y: 0x300,
        speed: 2,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x30),
        value: true,
    },
    SceneOp::Return { value: 1 },
];

/// Dorin's departure and Gryz replacement scene.
pub static DORIN: Scene = Scene {
    name: "Cutscene_Dorin",
    event: ArcEventIndexes::DORIN,
    ops: DORIN_OPS,
};

/// `Event_RuneFlaeli` — retail `$06D7C2..$06DBD7`, 1046 bytes, 27 ops.
static RUNE_FLAELI_OPS: &[SceneOp] = &[
    // Four NemDecomp uploads: `$179`, `$191`, `$1FB`, `$20C`, followed by
    // the palette-line copy at `$1DE0B8`.
    SceneOp::LoadArt {
        rom_addr: 0x001D628C,
        tile: 0x179,
    },
    SceneOp::LoadArt {
        rom_addr: 0x001D642E,
        tile: 0x191,
    },
    SceneOp::LoadArt {
        rom_addr: 0x001D6A2E,
        tile: 0x1FB,
    },
    SceneOp::LoadArt {
        rom_addr: 0x001D6BC8,
        tile: 0x20C,
    },
    SceneOp::LoadPalette {
        rom_addr: 0x001DE0B8,
        words: 16,
    },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x26)),
        window: DialogueWindow::Standard,
    },
    SceneOp::SetFollowMode { bits: 0x05 },
    SceneOp::MoveActorTo {
        actor: LEADER,
        x: 0x1F0,
        y: 0x240,
        wait: true,
    },
    SceneOp::Wait { ticks: 60 },
    SceneOp::SetFollowMode { bits: 0x04 },
    SceneOp::Face {
        actor: LEADER,
        facing: Direction::Up,
    },
    SceneOp::Wait { ticks: 30 },
    SceneOp::RunDialogueResume,
    SceneOp::Wait { ticks: 60 },
    SceneOp::ObjectAnimation {
        slot: 0,
        object_id: 0x214,
        art_tile: 0x179,
        frames: 60,
    },
    SceneOp::PlaySound { id: SFX_MEGID },
    SceneOp::ObjectAnimation {
        slot: 0,
        object_id: 0x214,
        art_tile: 0x179,
        frames: 180,
    },
    SceneOp::WaitFrames { frames: 10 },
    SceneOp::Face {
        actor: LEADER,
        facing: Direction::Down,
    },
    SceneOp::SetFollowMode { bits: 0 },
    SceneOp::MoveCamera {
        x: 0x1F0,
        y: 0x240,
        speed: 2,
    },
    SceneOp::RunDialogueResume,
    SceneOp::MoveActorTo {
        actor: LEADER,
        x: 0x1F0,
        y: 0x220,
        wait: true,
    },
    SceneOp::MoveCamera {
        x: 0x1F0,
        y: 0x220,
        speed: 2,
    },
    SceneOp::MoveActorTo {
        actor: LEADER,
        x: 0x1F0,
        y: 0x210,
        wait: true,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x13),
        value: true,
    },
    SceneOp::Return { value: 1 },
];

/// Rune destroys the rock barrier on the Tonoe road.
pub static RUNE_FLAELI: Scene = Scene {
    name: "Event_RuneFlaeli",
    event: ArcEventIndexes::RUNE_FLAELI,
    ops: RUNE_FLAELI_OPS,
};

/// `Event_AlshlineFound` — retail `$06DBD8..$06DBE7`, 16 bytes, 2 ops.
static ALSHLINE_FOUND_OPS: &[SceneOp] = &[
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x27)),
        window: DialogueWindow::Standard,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x32),
        value: true,
    },
];

/// The Alshline chest conversation.
pub static ALSHLINE_FOUND: Scene = Scene {
    name: "Event_AlshlineFound",
    event: ArcEventIndexes::ALSHLINE_FOUND,
    ops: ALSHLINE_FOUND_OPS,
};

/// `Cutscene_Alshline` — retail `$0741E6..$074555`, 880 bytes, 85 ops.
static ALSHLINE_OPS: &[SceneOp] = &[
    SceneOp::InitVramAndCram,
    SceneOp::PlaySound { id: MUSIC_STOP },
    SceneOp::FadeIn,
    SceneOp::PanelCreate { id: 0x1A },
    SceneOp::DmaPlanes,
    SceneOp::WaitFrames { frames: 20 },
    SceneOp::PanelCreate { id: 0x1B },
    SceneOp::DmaPlanes,
    SceneOp::PlaySound {
        id: SFX_BARRIER_BROKEN,
    },
    SceneOp::WaitFrames { frames: 60 },
    SceneOp::PanelCreate { id: 0x1C },
    SceneOp::DmaPlanes,
    SceneOp::PlaySound {
        id: SFX_BARRIER_BROKEN,
    },
    SceneOp::WaitFrames { frames: 10 },
    SceneOp::PanelCreate { id: 0x1D },
    SceneOp::DmaPlanes,
    SceneOp::PlaySound {
        id: SFX_BARRIER_BROKEN,
    },
    SceneOp::WaitFrames { frames: 10 },
    SceneOp::PanelCreate { id: 0x1E },
    SceneOp::DmaPlanes,
    SceneOp::PlaySound {
        id: SFX_BARRIER_BROKEN,
    },
    SceneOp::WaitFrames { frames: 90 },
    SceneOp::PanelDestroy { id: 4 },
    SceneOp::PanelDestroy { id: 3 },
    SceneOp::PanelDestroy { id: 2 },
    SceneOp::PanelDestroy { id: 1 },
    SceneOp::PanelDestroy { id: 0 },
    SceneOp::DmaPlanes,
    SceneOp::WaitFrames { frames: 40 },
    SceneOp::PanelCreate { id: 0x1F },
    SceneOp::DmaPlanes,
    SceneOp::PlaySound {
        id: MUSIC_HAPPY_SETTLEMENT,
    },
    SceneOp::WaitFrames { frames: 60 },
    SceneOp::PanelCreate { id: 0x20 },
    SceneOp::DmaPlanes,
    SceneOp::WaitFrames { frames: 120 },
    SceneOp::PanelDestroy { id: 1 },
    SceneOp::DmaPlanes,
    SceneOp::PanelDestroy { id: 0 },
    SceneOp::DmaPlanes,
    SceneOp::PanelCreate { id: 0x21 },
    SceneOp::DmaPlanes,
    SceneOp::WaitFrames { frames: 60 },
    SceneOp::PanelCreate { id: 0x22 },
    SceneOp::DmaPlanes,
    SceneOp::PlaySound {
        id: SFX_BARRIER_BROKEN,
    },
    SceneOp::WaitFrames { frames: 90 },
    SceneOp::SetDialogueTree { rom_addr: TREE_3 },
    SceneOp::SetRenderSpritesInCutscene { enabled: true },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x67)),
        window: DialogueWindow::Cutscene,
    },
    SceneOp::PlaySound { id: MUSIC_STOP },
    SceneOp::Wait { ticks: 60 },
    SceneOp::FadeIn,
    SceneOp::WaitFrames { frames: 90 },
    SceneOp::PanelCreate { id: 0x25 },
    SceneOp::DmaPlanes,
    SceneOp::PlaySound {
        id: MUSIC_MOTABIA_VILLAGE,
    },
    SceneOp::WaitFrames { frames: 60 },
    SceneOp::PanelCreate { id: 0x26 },
    SceneOp::DmaPlanes,
    SceneOp::WaitFrames { frames: 10 },
    SceneOp::RunDialogueResume,
    SceneOp::RecoverStats,
    SceneOp::SetMapLoadFlags {
        set: 0,
        clear: 0x08,
    },
    SceneOp::LoadMap {
        map: 0x24,
        prev_map: 0,
        start_x: 0x44,
        start_y: 0x3E,
        facing: Direction::Left,
        align: 0x10,
        clear_load_flags: 0x08,
    },
    SceneOp::DespawnNpc {
        npc_index: 0,
        count: 7,
    },
    SceneOp::LoadArt {
        rom_addr: 0x0012951A,
        tile: 0x3A5,
    },
    SceneOp::ObjectAnimation {
        slot: 7,
        object_id: 0x188,
        art_tile: 0x3A5,
        frames: 1,
    },
    SceneOp::MoveActorTo {
        actor: LEADER,
        x: 0x1E0,
        y: 0x100,
        wait: true,
    },
    SceneOp::SetStepOffset { value: 1 },
    SceneOp::MoveCamera {
        x: 0x1E0,
        y: 0xA0,
        speed: 1,
    },
    SceneOp::PlaySound { id: SFX_FUSION },
    SceneOp::ObjectAnimation {
        slot: 7,
        object_id: 0x8000,
        art_tile: 0x3A5,
        frames: 63,
    },
    SceneOp::MoveCamera {
        x: 0x1E0,
        y: 0xE0,
        speed: 2,
    },
    SceneOp::SetDialogueTree { rom_addr: TREE_3 },
    SceneOp::SetRenderSpritesInCutscene { enabled: false },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x68)),
        window: DialogueWindow::Standard,
    },
    SceneOp::DespawnNpc {
        npc_index: 7,
        count: 1,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x33),
        value: true,
    },
    SceneOp::RemoveItem { item: 0x8D },
    SceneOp::SetDialogueTree { rom_addr: TREE_4 },
    SceneOp::SetSavedMusic {
        id: MUSIC_MOTABIA_TOWN,
    },
    SceneOp::SetMapLoadFlags {
        set: 0x88,
        clear: 0,
    },
    SceneOp::StartBattle { index: 1 },
    SceneOp::Return { value: 1 },
];

/// The Alshline barrier and Zema Igglanova battle setup.
pub static ALSHLINE: Scene = Scene {
    name: "Cutscene_Alshline",
    event: ArcEventIndexes::ALSHLINE,
    ops: ALSHLINE_OPS,
};

/// `Cutscene_ZemaIgglanovaDefeated` — retail `$074556..$0745DD`, 136 bytes,
/// 10 ops.
static ZEMA_IGGLANOVA_DEFEATED_OPS: &[SceneOp] = &[
    SceneOp::PlaySound {
        id: MUSIC_MOTABIA_TOWN,
    },
    SceneOp::SetSavedMusic {
        id: MUSIC_MOTABIA_TOWN,
    },
    SceneOp::InitVramAndCram,
    SceneOp::FadeIn,
    SceneOp::SetDialogueTree { rom_addr: TREE_3 },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x69)),
        window: DialogueWindow::Cutscene,
    },
    SceneOp::AddMoney { amount: 1000 },
    SceneOp::SetFlag {
        flag: Flag::event(0x37),
        value: true,
    },
    SceneOp::LoadMap {
        map: 0x24,
        prev_map: 0xFFFF,
        start_x: 0x3C,
        start_y: 0x20,
        facing: Direction::Up,
        align: 4,
        clear_load_flags: 0x08,
    },
    SceneOp::Return { value: 0 },
];

/// The post-battle decision to go to Birth Valley.
pub static ZEMA_IGGLANOVA_DEFEATED: Scene = Scene {
    name: "Cutscene_ZemaIgglanovaDefeated",
    event: ArcEventIndexes::ZEMA_IGGLANOVA_DEFEATED,
    ops: ZEMA_IGGLANOVA_DEFEATED_OPS,
};

/// `Event_ZemaServantBattle` — retail `$07311E..$07313D`, 32 bytes, 4 ops.
static ZEMA_SERVANT_BATTLE_OPS: &[SceneOp] = &[
    SceneOp::SetFlag {
        flag: Flag::event(0xB2),
        value: true,
    },
    SceneOp::SetMapLoadFlags {
        set: 0x08,
        clear: 0,
    },
    SceneOp::StartBattle { index: 0x14 },
    SceneOp::Return { value: 1 },
];

/// The Zema servant battle hand-off.
pub static ZEMA_SERVANT_BATTLE: Scene = Scene {
    name: "Event_ZemaServantBattle",
    event: ArcEventIndexes::ZEMA_SERVANT_BATTLE,
    ops: ZEMA_SERVANT_BATTLE_OPS,
};

/// `Event_ZemaOldMan` — retail `$07313E..$073165`, 40 bytes, 3 ops.
static ZEMA_OLD_MAN_OPS: &[SceneOp] = &[
    SceneOp::FaceOppositeOf {
        actor: NPC_2,
        of: LEADER,
    },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x6C)),
        window: DialogueWindow::Standard,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0xB3),
        value: true,
    },
];

/// The first Zema old-man conversation.
pub static ZEMA_OLD_MAN: Scene = Scene {
    name: "Event_ZemaOldMan",
    event: ArcEventIndexes::ZEMA_OLD_MAN,
    ops: ZEMA_OLD_MAN_OPS,
};

/// `Event_ZemaOldManAfterMission` — retail `$073166..$07318D`, 40 bytes,
/// 3 ops.
static ZEMA_OLD_MAN_AFTER_MISSION_OPS: &[SceneOp] = &[
    SceneOp::FaceOppositeOf {
        actor: NPC_2,
        of: LEADER,
    },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x6D)),
        window: DialogueWindow::Standard,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0xB7),
        value: true,
    },
];

/// The later Zema old-man conversation; included because it is a live tree-4
/// control on the same map, not because its daughter mission belongs to the
/// linear Birth Valley walk.
pub static ZEMA_OLD_MAN_AFTER_MISSION: Scene = Scene {
    name: "Event_ZemaOldManAfterMission",
    event: ArcEventIndexes::ZEMA_OLD_MAN_AFTER_MISSION,
    ops: ZEMA_OLD_MAN_AFTER_MISSION_OPS,
};

/// `Event_MeetingSaya` — retail `$06BC6A..$06BE77`, 526 bytes, 12 ops.
///
/// The retail routine's object choreography uses temporary `$C480` and
/// `$C4C0` structs that are not map NPCs. `ObjectAnimation` retains those
/// writes; the one ordinary NPC movement remains executable and visible to
/// the field comparator.
static MEETING_SAYA_OPS: &[SceneOp] = &[
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x0E)),
        window: DialogueWindow::Standard,
    },
    SceneOp::ObjectAnimation {
        slot: 7,
        object_id: 0x190,
        art_tile: 0,
        frames: 1,
    },
    SceneOp::MoveActorTo {
        actor: NPC_0,
        x: 0x190,
        y: 0x1F0,
        wait: true,
    },
    SceneOp::Face {
        actor: NPC_0,
        facing: Direction::Left,
    },
    SceneOp::MoveActorTo {
        actor: NPC_0,
        x: 0x190,
        y: 0x210,
        wait: true,
    },
    SceneOp::RunDialogueResume,
    SceneOp::Face {
        actor: NPC_0,
        facing: Direction::Down,
    },
    SceneOp::ObjectAnimation {
        slot: 7,
        object_id: 0x1A0,
        art_tile: 0,
        frames: 90,
    },
    SceneOp::Wait { ticks: 30 },
    SceneOp::RunDialogueResume,
    SceneOp::SetFlag {
        flag: Flag::event(0x12),
        value: true,
    },
    SceneOp::Return { value: 1 },
];

/// Saya's first meeting. It is outside the three map-event lists named in
/// the arc brief, but the Tonoe route can pass through its `$16` trigger and
/// the event pointer is live, so it stays registered and documented.
pub static MEETING_SAYA: Scene = Scene {
    name: "Event_MeetingSaya",
    event: ArcEventIndexes::MEETING_SAYA,
    ops: MEETING_SAYA_OPS,
};

/// `Event_TonoeBasementDoor` — retail `$06F1A4..$06F2E9`, 326 bytes, 19 ops.
static TONOE_BASEMENT_DOOR_OPS: &[SceneOp] = &[
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x24)),
        window: DialogueWindow::Standard,
    },
    SceneOp::BranchFlag {
        flag: Flag::event(0x30),
        if_set: 2,
        if_clear: 17,
    },
    SceneOp::RunDialogueResume,
    SceneOp::Wait { ticks: 40 },
    SceneOp::MoveActorToActor {
        actor: LEADER,
        target: GRYZ_ACTOR,
        wait: true,
    },
    SceneOp::Face {
        actor: GRYZ_ACTOR,
        facing: Direction::Up,
    },
    SceneOp::Wait { ticks: 40 },
    SceneOp::RunDialogueResume,
    SceneOp::PlaySound {
        id: SFX_DOOR_OPENED,
    },
    SceneOp::ObjectAnimation {
        slot: 0,
        object_id: 0,
        art_tile: 0,
        frames: 47,
    },
    SceneOp::Wait { ticks: 1 },
    SceneOp::DespawnNpc {
        npc_index: 0,
        count: 3,
    },
    SceneOp::Wait { ticks: 1 },
    SceneOp::SetStepOffset { value: 1 },
    SceneOp::Face {
        actor: GRYZ_ACTOR,
        facing: Direction::Up,
    },
    SceneOp::RunDialogueResume,
    SceneOp::SetFlag {
        flag: Flag::event(0x31),
        value: true,
    },
    SceneOp::Return { value: 0 },
    SceneOp::Return { value: 1 },
];

/// Gryz opens Tonoe's basement door.
pub static TONOE_BASEMENT_DOOR: Scene = Scene {
    name: "Event_TonoeBasementDoor",
    event: ArcEventIndexes::TONOE_BASEMENT_DOOR,
    ops: TONOE_BASEMENT_DOOR_OPS,
};
