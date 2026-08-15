//! The ten opening-act scenes other than the intro.
//!
//! Each is transcribed from its `docs/scenes/` document, which disassembled it
//! from cartridge bytes. Op order and count match those documents.

use super::{ALYS, CHAZ, HAHN};
use crate::geom::Direction;
use crate::scene::{ActorRef, Axis, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// NPC-Alys on PiataAcademy_F1: field object `$FFFFC4C0` = `$C300 + 7*$40`,
/// so map NPC index 7.
const NPC_ALYS: ActorRef = ActorRef::Npc(7);
/// NPC slot 0 of the current map — `$FFFFC300`, which the clone calls both
/// `Field_Obj_Secondary` and `Hahn_Near_Basement`. On map `$12` that is Hahn,
/// on map `$14` the principal.
const NPC_0: ActorRef = ActorRef::Npc(0);
/// The party leader's field object, `Character_1`.
const LEADER: ActorRef = ActorRef::PartyMember(0);

const MUSIC_STOP: u8 = 0xFB;
const MUSIC_MOTAVIA_TOWN: u8 = 0x84;
const MUSIC_SUSPICION: u8 = 0x9F;
const MUSIC_MYSTERY: u8 = 0xAB;
const MUSIC_IN_THE_CAVE: u8 = 0x8A;

/// `Event_PiataChazAlone` — retail `$073ECE..$073EDF`, 18 bytes, 2 ops.
///
/// Chaz notices he has lost Alys. Doc: `02_PiataChazAlone.md`.
static PIATA_CHAZ_ALONE_OPS: &[SceneOp] = &[
    // tree 1 entry $6D — "Oops! I wandered around and now I've gotten
    // separated from Alys."
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x6D)),
        window: DialogueWindow::Standard,
    },
    // EventFlag_PiataChazControl — set when you gain control of Chaz in Piata.
    SceneOp::SetFlag {
        flag: Flag::event(0x15),
        value: true,
    },
];

/// See [`PIATA_CHAZ_ALONE_OPS`].
pub static PIATA_CHAZ_ALONE: Scene = Scene {
    name: "Event_PiataChazAlone",
    event: EventIndexes::PIATA_CHAZ_ALONE,
    ops: PIATA_CHAZ_ALONE_OPS,
};

/// `Event_AlysFound` — retail `$06B2D8..$06B3B9`, 226 bytes, 12 ops.
///
/// Alys aligns with Chaz, turns, they talk, and **Alys takes the lead**. Doc:
/// `03_AlysFound.md`. The clone kept this scene's prologue and epilogue and cut
/// the middle out, so it looks like retail while omitting both the dialogue
/// call and the flag set that stops the trigger re-firing forever.
static ALYS_FOUND_OPS: &[SceneOp] = &[
    // 0: `cmp.w $34(a3),d0` — skip the alignment step when already level.
    SceneOp::BranchIfAligned {
        a: ActorRef::Character(CHAZ),
        b: NPC_ALYS,
        axis: Axis::Y,
        if_aligned: 2,
        if_not: 1,
    },
    // 1: movement command 2 = one 16-pixel step down, spun until idle.
    SceneOp::MoveActorCommand {
        actor: NPC_ALYS,
        command: 2,
        wait: true,
    },
    // 2: FacingDir_Right.
    SceneOp::Face {
        actor: NPC_ALYS,
        facing: Direction::Right,
    },
    // 3: `move.b $14(a4),d0` — the entry index is her object's dialogue_id.
    SceneOp::RunDialogue {
        source: DialogueSource::NpcDialogueId(NPC_ALYS),
        window: DialogueWindow::Standard,
    },
    // 4: `move.w #$0100,$F40A` — Alys slot 1, Chaz slot 2.
    SceneOp::SetParty {
        slots: [Some(ALYS), Some(CHAZ), None, None, None],
    },
    // 5: `trap #1`, 32 words — Chaz's field-object struct into Character_2.
    //    The party slots were written at op 4; this moves the object.
    SceneOp::CopyCharSlot { from: 0, to: 1 },
    // 6: Chaz's art tile in his new object slot.
    SceneOp::SetArtTile {
        actor: ActorRef::Character(CHAZ),
        tile: 0x53C,
    },
    // 7: build field-Alys at NPC-Alys's position: obj id 8, art tile $534.
    SceneOp::PromoteNpcToChar {
        npc: 7,
        char_id: ALYS,
        slot: 0,
        art_tile: 0x534,
        facing: Direction::Left,
    },
    // 8: Event_AddMacro with d0 = CharID_Alys.
    SceneOp::JoinParty { slot: 0, who: ALYS },
    // 9: `clr.w` + `trap #0` with d7 = $F — one object.
    SceneOp::DespawnNpc {
        npc_index: 7,
        count: 1,
    },
    // 10: camera to the leader's position at speed 2.
    SceneOp::MoveCamera {
        x: 0x260,
        y: 0xF0,
        speed: 2,
    },
    // 11: EventFlag_AlysFound. The clone omits this; retail ends on it.
    SceneOp::SetFlag {
        flag: Flag::event(0x08),
        value: true,
    },
];

/// See [`ALYS_FOUND_OPS`].
pub static ALYS_FOUND: Scene = Scene {
    name: "Event_AlysFound",
    event: EventIndexes::ALYS_FOUND,
    ops: ALYS_FOUND_OPS,
};

/// `Cutscene_PiataPrincipal` — retail `$073EE0..$073F21`, 66 bytes, 10 ops.
///
/// The principal hires the party. A **cutscene**: it returns 0, and
/// `FieldRoutine_Cutscene` reloads the map on a zero return, which is how the
/// office comes back with post-briefing NPC state. Doc: `04_PiataPrincipal.md`.
static PIATA_PRINCIPAL_OPS: &[SceneOp] = &[
    SceneOp::PlaySound { id: MUSIC_STOP },
    SceneOp::InitVramAndCram,
    SceneOp::PlaySound {
        id: MUSIC_SUSPICION,
    },
    SceneOp::FadeIn,
    SceneOp::SetRenderSpritesInCutscene { enabled: true },
    // Event_GetAndRunDialogue5 — the other window setup.
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x17)),
        window: DialogueWindow::Cutscene,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x09),
        value: true,
    },
    SceneOp::PlaySound { id: MUSIC_STOP },
    SceneOp::SetSavedMusic { id: 0 },
    // d0 = 0 -> the caller reloads the map.
    SceneOp::Return { value: 0 },
];

/// See [`PIATA_PRINCIPAL_OPS`].
pub static PIATA_PRINCIPAL: Scene = Scene {
    name: "Cutscene_PiataPrincipal",
    event: EventIndexes::PIATA_PRINCIPAL,
    ops: PIATA_PRINCIPAL_OPS,
};

/// `Event_SuspicionOnPrincipal` — retail `$06BEE8..$06BEF7`, 16 bytes, 2 ops.
///
/// Alys says the principal is hiding something. Structurally identical to
/// `Event_PiataChazAlone`. Doc: `05_SuspicionOnPrincipal.md`.
static SUSPICION_ON_PRINCIPAL_OPS: &[SceneOp] = &[
    // tree 1 entry $6E — "Something smells fishy here."
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x6E)),
        window: DialogueWindow::Standard,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x0E),
        value: true,
    },
];

/// See [`SUSPICION_ON_PRINCIPAL_OPS`].
pub static SUSPICION_ON_PRINCIPAL: Scene = Scene {
    name: "Event_SuspicionOnPrincipal",
    event: EventIndexes::SUSPICION_ON_PRINCIPAL,
    ops: SUSPICION_ON_PRINCIPAL_OPS,
};

/// `Event_MeetingHahn` — retail `$06B3BA..$06B4AF`, 246 bytes, 14 ops.
///
/// Hahn hires himself on, joins in **slot 3**, and pays 100 meseta up front.
/// Doc: `06_MeetingHahn.md`. Byte-exact in the clone, which is why it was used
/// to bootstrap the primitive symbol map.
static MEETING_HAHN_OPS: &[SceneOp] = &[
    // 0: let any in-progress motion settle before staging.
    SceneOp::WaitForActor { actor: NPC_0 },
    // 1: already on the leader's column -> skip the walk.
    SceneOp::BranchIfAligned {
        a: NPC_0,
        b: LEADER,
        axis: Axis::X,
        if_aligned: 5,
        if_not: 2,
    },
    // 2: `cmp.w hahn_x, leader_x / bhi` picks command 4, else command 8.
    //    The doc writes this as one op selecting a *value*; this vocabulary
    //    has no value-selecting branch, so it is control flow over two command
    //    ops instead — same behaviour, two ops more than the doc's count.
    SceneOp::BranchIfActorGreater {
        a: LEADER,
        b: NPC_0,
        axis: Axis::X,
        if_greater: 3,
        if_not: 4,
    },
    // 3: leader is to Hahn's right -> command 4.
    SceneOp::MoveActorCommand {
        actor: NPC_0,
        command: 4,
        wait: true,
    },
    SceneOp::Jump { to: 5 },
    // 4: leader is to Hahn's left -> command 8. Which byte means which
    //    direction is an open question in the doc; the engine reports the byte
    //    rather than acting on it.
    SceneOp::MoveActorCommand {
        actor: NPC_0,
        command: 8,
        wait: true,
    },
    // 5: FacingDir_Down.
    SceneOp::Face {
        actor: NPC_0,
        facing: Direction::Down,
    },
    // 5: tree 33 entry $11 — the hiring conversation.
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x11)),
        window: DialogueWindow::Standard,
    },
    // 6: `move.b #$2,$F40C` — Current_Party_Slot_3 = CharID_Hahn.
    SceneOp::JoinParty { slot: 2, who: HAHN },
    // 7: build field-Hahn: obj id $C, art tile $544, facing down.
    SceneOp::PromoteNpcToChar {
        npc: 0,
        char_id: HAHN,
        slot: 2,
        art_tile: 0x544,
        facing: Direction::Down,
    },
    // 8: `bset #1,$ECFE` — Char_Move_Flags bit 1, the movement-order bit.
    SceneOp::SetFollowMode { bits: 0b10 },
    // 9: Event_AddMacro with d0 = CharID_Hahn.
    SceneOp::JoinParty { slot: 2, who: HAHN },
    // 10: `trap #0` with d7 = $2F — 48 longwords = **three** objects. Map $12
    //     NPC 0 is Hahn and NPCs 1-2 are the InvisibleBlock pair fencing him
    //     in; clearing one would leave two invisible walls in the corridor.
    SceneOp::DespawnNpc {
        npc_index: 0,
        count: 3,
    },
    // 11: `addi.l #100,$F438`.
    SceneOp::AddMoney { amount: 100 },
    // 12: Map_Palettes_Addr -> Palette_Table_Buffer, 48 words in two copies.
    SceneOp::ReloadMapPalette,
    // 13: EventFlag_HahnJoined.
    SceneOp::SetFlag {
        flag: Flag::event(0x0A),
        value: true,
    },
];

/// See [`MEETING_HAHN_OPS`].
pub static MEETING_HAHN: Scene = Scene {
    name: "Event_MeetingHahn",
    event: EventIndexes::MEETING_HAHN,
    ops: MEETING_HAHN_OPS,
};

/// `Event_BasementContainers` — retail `$06BC36..$06BC69`, 52 bytes, 7 ops.
///
/// The music drops to Mystery, a line plays, half a second passes, the cave
/// music returns. Doc: `07_BasementContainers.md`.
static BASEMENT_CONTAINERS_OPS: &[SceneOp] = &[
    SceneOp::PlaySound { id: MUSIC_MYSTERY },
    // A bare VInt_Prepare: one frame, no map update, so the sound driver sees
    // the new Sound_Index before the window opens.
    SceneOp::WaitFrames { frames: 1 },
    // tree 33 entry $0F — "Wh..What's this...?!"
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x0F)),
        window: DialogueWindow::Standard,
    },
    // DoMapUpdateLoop with d0 = $1D, so 30 frames, input discarded.
    SceneOp::Wait { ticks: 30 },
    SceneOp::PlaySound {
        id: MUSIC_IN_THE_CAVE,
    },
    SceneOp::WaitFrames { frames: 1 },
    SceneOp::SetFlag {
        flag: Flag::event(0x0D),
        value: true,
    },
];

/// See [`BASEMENT_CONTAINERS_OPS`].
pub static BASEMENT_CONTAINERS: Scene = Scene {
    name: "Event_BasementContainers",
    event: EventIndexes::BASEMENT_CONTAINERS,
    ops: BASEMENT_CONTAINERS_OPS,
};

/// `Event_IgglanovaBattle` — retail `$0722D2..$0722F5`, 36 bytes, 4 ops.
///
/// A guard, a flag, and a hand-off. Reached from an **interaction area**, not
/// the trigger table. Doc: `08_IgglanovaBattle.md`.
///
/// The flag is set on the way *in*, not on victory — so losing or fleeing does
/// not let you re-trigger the fight, and both `RunEvent_AfterIgglanova` and the
/// principal's confession become live the moment the battle starts.
static IGGLANOVA_BATTLE_OPS: &[SceneOp] = &[
    // 0: already fought -> return non-zero, which suppresses the map reload.
    SceneOp::BranchFlag {
        flag: Flag::event(0x0B),
        if_set: 3,
        if_clear: 1,
    },
    // 1: EventFlag_Igglanova, set before the fight.
    SceneOp::SetFlag {
        flag: Flag::event(0x0B),
        value: true,
    },
    // 2: `bset #3,Routine_Exit_Flags` — the dispatcher's epilogue turns this
    //    into a battle *after* the scene returns normally.
    SceneOp::StartBattle { index: 0 },
    // 3: return 1 — both the guard path and the normal path land here.
    SceneOp::Return { value: 1 },
];

/// See [`IGGLANOVA_BATTLE_OPS`].
pub static IGGLANOVA_BATTLE: Scene = Scene {
    name: "Event_IgglanovaBattle",
    event: EventIndexes::IGGLANOVA_BATTLE,
    ops: IGGLANOVA_BATTLE_OPS,
};

/// `Event_AfterIgglanova` — retail `$06D6C8..$06D783`, 188 bytes, 15 ops.
///
/// The most elaborately staged scene in the act: the conversation **pauses
/// twice** while the actors reposition, using `RunDialogueResume`. Doc:
/// `09_AfterIgglanova.md`.
static AFTER_IGGLANOVA_OPS: &[SceneOp] = &[
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x12)),
        window: DialogueWindow::Standard,
    },
    // Slow the actors down for the staging.
    SceneOp::SetStepOffset { value: 0 },
    SceneOp::Face {
        actor: ActorRef::Character(CHAZ),
        facing: Direction::Right,
    },
    SceneOp::RunDialogueResume,
    SceneOp::Face {
        actor: ActorRef::Character(CHAZ),
        facing: Direction::Up,
    },
    SceneOp::Face {
        actor: ActorRef::Character(ALYS),
        facing: Direction::Up,
    },
    SceneOp::Face {
        actor: ActorRef::Character(HAHN),
        facing: Direction::Down,
    },
    SceneOp::Wait { ticks: 30 },
    SceneOp::MoveActorTo {
        actor: ActorRef::Character(HAHN),
        x: 256,
        y: 192,
        wait: true,
    },
    SceneOp::RunDialogueResume,
    SceneOp::OverlapCharacters,
    SceneOp::MoveCamera {
        x: 256,
        y: 192,
        speed: 1,
    },
    SceneOp::Face {
        actor: LEADER,
        facing: Direction::Down,
    },
    // Back to normal walking speed.
    SceneOp::SetStepOffset { value: 1 },
    SceneOp::SetFlag {
        flag: Flag::event(0x0F),
        value: true,
    },
];

/// See [`AFTER_IGGLANOVA_OPS`].
pub static AFTER_IGGLANOVA: Scene = Scene {
    name: "Event_AfterIgglanova",
    event: EventIndexes::AFTER_IGGLANOVA,
    ops: AFTER_IGGLANOVA_OPS,
};

/// `Event_PrincipalConfession` — retail `$06D784..$06D7C1`, 62 bytes, 6 ops.
///
/// The principal confesses and pays 300 meseta. Setting
/// `EventFlag_PrincipalConfession` is what opens Piata's gates. Doc:
/// `10_PrincipalConfession.md`. The clone comments the whole body out to a bare
/// `rts`, so on the fork the party is never paid and the gates never open.
static PRINCIPAL_CONFESSION_OPS: &[SceneOp] = &[
    SceneOp::PlaySound {
        id: MUSIC_SUSPICION,
    },
    // `move.w facing_dir(leader),d0 / bchg #2,d0` — the principal turns to face
    // the leader head-on, from whichever side they approached.
    SceneOp::FaceOppositeOf {
        actor: NPC_0,
        of: LEADER,
    },
    // tree 33 entry $13, 2106 bytes — the longest entry in the tree.
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x13)),
        window: DialogueWindow::Standard,
    },
    SceneOp::AddMoney { amount: 300 },
    SceneOp::PlaySound {
        id: MUSIC_MOTAVIA_TOWN,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x0C),
        value: true,
    },
];

/// See [`PRINCIPAL_CONFESSION_OPS`].
pub static PRINCIPAL_CONFESSION: Scene = Scene {
    name: "Event_PrincipalConfession",
    event: EventIndexes::PRINCIPAL_CONFESSION,
    ops: PRINCIPAL_CONFESSION_OPS,
};

/// `Event_PiataGuardsReprimand` — retail `$0738D2..$073945`, 116 bytes, 10 ops.
///
/// The act boundary: try to leave Piata early and the guards throw you back in.
/// Doc: `11_PiataGuardsReprimand.md`.
///
/// The clone rewrites this in place to reuse Piata's own tree; retail swaps in
/// **tree 43**, says entry `$10`, and swaps **tree 1** back — even though it is
/// standing on map `$10`, which binds tree 2. Whether that mis-restore is an
/// original bug or masked by an in-flight map load is an open oracle question,
/// so it is reproduced exactly rather than "fixed".
static PIATA_GUARDS_REPRIMAND_OPS: &[SceneOp] = &[
    SceneOp::PlaySound { id: MUSIC_STOP },
    SceneOp::FadeOut,
    // Map_Start is in 8-pixel units: ($3E,$56) -> ($1F0,$2B0) px = cell
    // (31, 43), facing down, just inside the gate.
    SceneOp::LoadMap {
        map: 0x10,
        prev_map: 0x00,
        start_x: 0x3E,
        start_y: 0x56,
        facing: Direction::Down,
        align: 8,
        clear_load_flags: 0b0000_1000,
    },
    SceneOp::PlaySound {
        id: MUSIC_MOTAVIA_TOWN,
    },
    SceneOp::SetSavedMusic {
        id: MUSIC_MOTAVIA_TOWN,
    },
    SceneOp::FadeIn,
    SceneOp::SetDialogueTree {
        rom_addr: 0x001F_D7A0,
    },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x10)),
        window: DialogueWindow::Standard,
    },
    // Restores tree 1 while standing on a map that binds tree 2. See above.
    SceneOp::SetDialogueTree {
        rom_addr: 0x001D_F600,
    },
    SceneOp::Return { value: 0 },
];

/// See [`PIATA_GUARDS_REPRIMAND_OPS`].
pub static PIATA_GUARDS_REPRIMAND: Scene = Scene {
    name: "Event_PiataGuardsReprimand",
    event: EventIndexes::PIATA_GUARDS_REPRIMAND,
    ops: PIATA_GUARDS_REPRIMAND_OPS,
};

/// The `Event_Index` values the opening act's scenes answer to.
pub struct EventIndexes;

impl EventIndexes {
    /// `EventPtrs[$9F]`.
    pub const GAME_START: EventIndex = EventIndex(0x009F);
    /// `EventPtrs[$A0]`.
    pub const PIATA_CHAZ_ALONE: EventIndex = EventIndex(0x00A0);
    /// `EventPtrs[$03]`.
    pub const ALYS_FOUND: EventIndex = EventIndex(0x0003);
    /// `CutscenePtrs[$01]`, so `Event_Index = $8001`.
    pub const PIATA_PRINCIPAL: EventIndex = EventIndex(0x8001);
    /// `EventPtrs[$0F]`.
    pub const SUSPICION_ON_PRINCIPAL: EventIndex = EventIndex(0x000F);
    /// `EventPtrs[$04]`.
    pub const MEETING_HAHN: EventIndex = EventIndex(0x0004);
    /// `EventPtrs[$0C]`.
    pub const BASEMENT_CONTAINERS: EventIndex = EventIndex(0x000C);
    /// `EventPtrs[$6B]`.
    pub const IGGLANOVA_BATTLE: EventIndex = EventIndex(0x006B);
    /// `EventPtrs[$25]`.
    pub const AFTER_IGGLANOVA: EventIndex = EventIndex(0x0025);
    /// `EventPtrs[$26]`.
    pub const PRINCIPAL_CONFESSION: EventIndex = EventIndex(0x0026);
    /// `EventPtrs[$9E]`.
    pub const PIATA_GUARDS_REPRIMAND: EventIndex = EventIndex(0x009E);
}
