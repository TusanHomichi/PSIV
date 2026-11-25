//! Internal spike DTO for data-driven scene programs (#793).
//!
//! This module is intentionally an internal prototype rather than a committed
//! 1.x public mod contract: every struct and enum denies unknown fields so
//! malformed JSON fails closed at load time.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Format version for internal data-driven scene spike files.
pub const DATA_SCENE_FORMAT_VERSION: u32 = 1;

/// Failure parsing or checking the wire envelope of a [`DataSceneProgram`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataSceneError {
    /// JSON deserialization or serialization failed.
    Json(String),
    /// `format_version` did not match [`DATA_SCENE_FORMAT_VERSION`].
    UnsupportedFormatVersion {
        /// Version read from the file.
        found: u32,
        /// Only version this spike accepts.
        expected: u32,
    },
    /// Scene name was empty or whitespace-only.
    EmptyName,
}

impl fmt::Display for DataSceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(msg) => write!(f, "data scene JSON error: {msg}"),
            Self::UnsupportedFormatVersion { found, expected } => {
                write!(
                    f,
                    "unsupported data scene format_version {found} (expected {expected})"
                )
            }
            Self::EmptyName => write!(f, "data scene name must not be empty"),
        }
    }
}

impl std::error::Error for DataSceneError {}

/// A serialized scene program loaded from JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataSceneProgram {
    /// Wire format version; must equal [`DATA_SCENE_FORMAT_VERSION`].
    pub format_version: u32,
    /// Human-readable scene identifier for diagnostics.
    pub name: String,
    /// `Event_Index` value this scene handles (may override an existing event
    /// or register a new custom id).
    pub event_id: u16,
    /// Optional map id scoping load-time NPC and map-bound dialogue checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_map: Option<u16>,
    /// Flat instruction list executed by `psiv-core`'s `SceneRunner`.
    pub ops: Vec<DataSceneOp>,
}

impl DataSceneProgram {
    /// Parses a JSON string into a [`DataSceneProgram`], rejecting unknown
    /// fields, unsupported versions, and blank names.
    pub fn from_json(json: &str) -> Result<Self, DataSceneError> {
        let program: Self =
            serde_json::from_str(json).map_err(|err| DataSceneError::Json(err.to_string()))?;
        if program.format_version != DATA_SCENE_FORMAT_VERSION {
            return Err(DataSceneError::UnsupportedFormatVersion {
                found: program.format_version,
                expected: DATA_SCENE_FORMAT_VERSION,
            });
        }
        if program.name.trim().is_empty() {
            return Err(DataSceneError::EmptyName);
        }
        Ok(program)
    }

    /// Serializes this scene program to pretty-printed JSON.
    pub fn to_json(&self) -> Result<String, DataSceneError> {
        serde_json::to_string_pretty(self).map_err(|err| DataSceneError::Json(err.to_string()))
    }
}

/// Cardinal facing direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataDirection {
    /// Toward lower Y.
    Up,
    /// Toward higher Y.
    Down,
    /// Toward lower X.
    Left,
    /// Toward higher X.
    Right,
}

/// Coordinate axis for alignment and comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataAxis {
    /// Horizontal coordinate.
    X,
    /// Vertical coordinate.
    Y,
}

/// Unsigned word comparison predicate for coordinate branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataCoordCmp {
    /// `==`.
    Equal,
    /// `!=`.
    NotEqual,
    /// `<`.
    Below,
    /// `<=`.
    AtMost,
    /// `>`.
    Above,
    /// `>=`.
    AtLeast,
}

/// Flag bank selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataFlagBank {
    /// Persistent event flags (`$000..=$1FF`, chests at `$100..`).
    Event,
    /// Temporary event flags (`$000..=$FF`).
    Temp,
    /// Town discovery flags (`$000..=$7F`).
    Town,
}

/// Flag operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataFlag {
    /// Flag bank.
    pub bank: DataFlagBank,
    /// Flag bit index within the bank.
    pub id: u16,
}

/// Actor reference operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DataActorRef {
    /// Party member by slot (`0..=4`).
    PartyMember {
        /// Zero-based party slot.
        slot: usize,
    },
    /// Map field object by index in the map's NPC list.
    Npc {
        /// Zero-based NPC index.
        index: usize,
    },
    /// Party character resolved dynamically by character id.
    Character {
        /// Character roster id (`0..=10`).
        id: u8,
    },
}

/// Grid cell coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataCell {
    /// Column in 16-pixel cells.
    pub x: u16,
    /// Row in 16-pixel cells.
    pub y: u16,
}

/// Dialogue entry source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DataDialogueSource {
    /// Literal entry index in the active dialogue tree.
    Entry {
        /// Entry index.
        id: u16,
    },
    /// Entry index read off a map NPC's `dialogue_id`.
    NpcDialogueId {
        /// Target NPC actor.
        actor: DataActorRef,
    },
    /// Standalone system line outside dialogue trees.
    CannotGetOff,
}

/// Dialogue window routine style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataDialogueWindow {
    /// Standard field dialogue window.
    #[default]
    Standard,
    /// Retained window (for battle/map-change handoffs).
    Retained,
    /// Cutscene window that fades out on close.
    Cutscene,
    /// Cutscene window that leaves panels up on close.
    Cutscene3,
    /// Ending intro window.
    EndingIntro,
    /// Ending auto-paced window.
    Ending,
    /// Resume cutscene-5 window.
    Cutscene5,
    /// Inline dialogue window.
    Inline,
}

/// Camera pan target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DataCameraSubject {
    /// Literal pixel coordinates.
    At {
        /// Pixel X.
        x: i32,
        /// Pixel Y.
        y: i32,
    },
    /// Track an actor's live position.
    Actor {
        /// Target actor.
        actor: DataActorRef,
    },
    /// Actor's X with literal Y.
    ActorX {
        /// Target actor.
        actor: DataActorRef,
        /// Pixel Y.
        y: i32,
    },
    /// Actor's position shifted along X.
    ActorShifted {
        /// Target actor.
        actor: DataActorRef,
        /// Pixel X delta.
        dx: i32,
    },
}

/// Purse comparison mode for `branch_if_money_below`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataMoneyTest {
    /// Wrapping difference sign bit.
    SignOfDifference,
    /// Unsigned longword compare.
    #[default]
    Unsigned,
}

/// Absolute map chunk write entry `(chunk_x, chunk_y, chunk_id)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataChunkWrite {
    /// Chunk column.
    pub x: u32,
    /// Chunk row.
    pub y: u32,
    /// Chunk id.
    pub chunk_id: u16,
}

/// Actor-relative map chunk write entry `(dx_px, dy_px, chunk_id)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataActorChunkWrite {
    /// Pixel X offset from actor.
    pub dx: i32,
    /// Pixel Y offset from actor.
    pub dy: i32,
    /// Chunk id.
    pub chunk_id: u16,
}

/// One drifting NPC record for `drift_npcs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataNpcDrift {
    /// Map NPC index.
    pub npc: usize,
    /// Signed 16.16 X increment per frame.
    pub step_x: i32,
    /// Signed 16.16 Y increment per frame.
    pub step_y: i32,
}

/// Explicit sequence animation state for `set_actor_animation`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataActorAnimation {
    /// Mapping selector word.
    pub word: u16,
    /// Starting frame index.
    pub index: u8,
    /// Initial frame countdown.
    pub duration: u8,
    /// Per-frame duration table.
    pub durations: Vec<u8>,
}

/// One serialized instruction in a [`DataSceneProgram`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum DataSceneOp {
    /// Open a dialogue line and block until it closes.
    RunDialogue {
        /// Where the dialogue index comes from.
        source: DataDialogueSource,
        /// Window setup style.
        #[serde(default)]
        window: DataDialogueWindow,
    },
    /// Resume the suspended dialogue cursor.
    RunDialogueResume,
    /// Resume the suspended dialogue cursor with a named window routine.
    RunDialogueResumeWithWindow {
        /// Window setup style.
        window: DataDialogueWindow,
    },
    /// Select a dialogue tree by ROM address.
    SetDialogueTree {
        /// Dialogue tree ROM address.
        rom_addr: u32,
    },
    /// Write a flag.
    SetFlag {
        /// Target flag.
        flag: DataFlag,
        /// New value.
        value: bool,
    },
    /// Flip a flag in place.
    ToggleFlag {
        /// Target flag.
        flag: DataFlag,
    },
    /// Unconditional jump to op index `to`.
    Jump {
        /// Target op index.
        to: usize,
    },
    /// Skip `count` following ops.
    SkipOps {
        /// Ops to skip.
        count: u16,
    },
    /// Skip `skip` ops unless the current map is listed in `maps`.
    SkipUnlessMap {
        /// Map ids that do not skip.
        maps: Vec<u16>,
        /// Ops skipped otherwise.
        skip: u16,
    },
    /// Branch on a flag value.
    BranchFlag {
        /// Flag tested.
        flag: DataFlag,
        /// Target op index when set.
        if_set: usize,
        /// Target op index when clear.
        if_clear: usize,
    },
    /// Ask a yes/no choice and branch.
    BranchChoice {
        /// Target op index on yes.
        if_yes: usize,
        /// Target op index on no.
        if_no: usize,
    },
    /// Branch when the last dialogue ended at `$FF`.
    BranchDialogueEnd {
        /// Target op index when ended.
        if_ended: usize,
    },
    /// Branch on whether two actors share a cell coordinate.
    BranchIfAligned {
        /// First actor.
        a: DataActorRef,
        /// Second actor.
        b: DataActorRef,
        /// Compared axis.
        axis: DataAxis,
        /// Target op index when aligned.
        if_aligned: usize,
        /// Target op index otherwise.
        if_not: usize,
    },
    /// Branch on whether `a`'s coordinate is greater than `b`'s.
    BranchIfActorGreater {
        /// Left-hand actor.
        a: DataActorRef,
        /// Right-hand actor.
        b: DataActorRef,
        /// Compared axis.
        axis: DataAxis,
        /// Target op index when greater.
        if_greater: usize,
        /// Target op index otherwise.
        if_not: usize,
    },
    /// Branch on an actor's pixel coordinate against a literal word.
    BranchIfActorCoord {
        /// Inspected actor.
        actor: DataActorRef,
        /// Axis read.
        axis: DataAxis,
        /// Unsigned comparison predicate.
        cmp: DataCoordCmp,
        /// Literal pixel coordinate.
        value: u16,
        /// Target op index when true.
        if_true: usize,
        /// Target op index when false.
        if_false: usize,
    },
    /// Branch on whether a character is in the active party.
    BranchIfPartyMember {
        /// Character id.
        who: u8,
        /// Target op index when present.
        if_present: usize,
        /// Target op index when absent.
        if_absent: usize,
    },
    /// Branch on whether current meseta is below `amount`.
    BranchIfMoneyBelow {
        /// Threshold amount.
        amount: u32,
        /// Comparison mode.
        #[serde(default)]
        test: DataMoneyTest,
        /// Target op index when below.
        if_below: usize,
        /// Target op index otherwise.
        if_not: usize,
    },
    /// Branch on whether all 40 inventory slots are full.
    BranchIfInventoryFull {
        /// Target op index when full.
        if_full: usize,
        /// Target op index otherwise.
        if_not: usize,
    },
    /// Branch on whether a vehicle is mounted.
    BranchIfVehicle {
        /// Target op index when mounted.
        if_mounted: usize,
        /// Target op index on foot.
        if_on_foot: usize,
    },
    /// Start an actor walking to a cell (non-blocking).
    MoveActor {
        /// Actor that walks.
        actor: DataActorRef,
        /// Destination cell.
        to: DataCell,
    },
    /// Walk an actor to a pixel position.
    MoveActorTo {
        /// Actor that walks.
        actor: DataActorRef,
        /// Target pixel X.
        x: i32,
        /// Target pixel Y.
        y: i32,
        /// Block until arrival.
        wait: bool,
    },
    /// Walk an actor to another actor's current cell.
    MoveActorToActor {
        /// Actor that walks.
        actor: DataActorRef,
        /// Actor whose cell is targeted.
        target: DataActorRef,
        /// Block until arrival.
        wait: bool,
    },
    /// Align an actor to another actor on one axis.
    MoveActorToActorAxis {
        /// Actor that walks.
        actor: DataActorRef,
        /// Actor supplying the axis coordinate.
        target: DataActorRef,
        /// Axis copied.
        axis: DataAxis,
        /// Block until arrival.
        wait: bool,
    },
    /// Walk an actor by a pixel offset from its current cell.
    MoveActorOffset {
        /// Actor that walks.
        actor: DataActorRef,
        /// Pixel X delta.
        dx: i32,
        /// Pixel Y delta.
        dy: i32,
        /// Block until arrival.
        wait: bool,
    },
    /// Issue a retail NPC movement command byte.
    MoveActorCommand {
        /// Target actor.
        actor: DataActorRef,
        /// Retail command byte.
        command: u8,
        /// Block until step completes.
        wait: bool,
    },
    /// Place an actor directly at pixel coordinates.
    PlaceActor {
        /// Target actor.
        actor: DataActorRef,
        /// Pixel X.
        x: i32,
        /// Pixel Y.
        y: i32,
    },
    /// Set an actor's destination without starting a scripted walk.
    SetActorDest {
        /// Target actor.
        actor: DataActorRef,
        /// Pixel X.
        x: i32,
        /// Pixel Y.
        y: i32,
    },
    /// Turn an actor to face a cardinal direction.
    Face {
        /// Target actor.
        actor: DataActorRef,
        /// New facing.
        facing: DataDirection,
    },
    /// Turn `actor` to face the opposite of `of`.
    FaceOppositeOf {
        /// Actor that turns.
        actor: DataActorRef,
        /// Actor whose facing is flipped.
        of: DataActorRef,
    },
    /// Turn all occupied party members to a shared facing.
    FaceParty {
        /// Shared facing.
        facing: DataDirection,
    },
    /// Select an explicit animation sequence on an actor.
    SetActorAnimation {
        /// Target actor.
        actor: DataActorRef,
        /// Sequence state.
        animation: DataActorAnimation,
    },
    /// Drift map NPCs by fixed 16.16 pixel increments for `frames` ticks.
    DriftNpcs {
        /// Per-NPC drift entries.
        drifts: Vec<DataNpcDrift>,
        /// Frame count.
        frames: u16,
    },
    /// Block for `ticks` map-update frames.
    Wait {
        /// Tick count.
        ticks: u16,
    },
    /// Block for `frames` VBlank frames without map updates.
    WaitFrames {
        /// Frame count.
        frames: u16,
    },
    /// Block until an actor finishes walking.
    WaitForActor {
        /// Actor waited on.
        actor: DataActorRef,
    },
    /// Pan camera to a target and wait for arrival.
    PanCamera {
        /// Target subject.
        subject: DataCameraSubject,
        /// Pan speed in pixels per frame.
        speed: u16,
    },
    /// Play a sound or music track.
    PlaySound {
        /// Retail sound id.
        id: u8,
    },
    /// Update saved music index.
    SetSavedMusic {
        /// Retail sound id.
        id: u8,
    },
    /// Fade palette in.
    FadeIn,
    /// Fade palette out.
    FadeOut,
    /// Clear VRAM/CRAM (preceded by fade out).
    InitVramAndCram,
    /// Load a map and place the party.
    LoadMap {
        /// Target map id.
        map: u16,
        /// Previous map id.
        prev_map: u16,
        /// Start X in 8-pixel units.
        start_x: u16,
        /// Start Y in 8-pixel units.
        start_y: u16,
        /// Arrival facing.
        facing: DataDirection,
        /// Follower alignment byte.
        align: u8,
        /// `Map_Load_Flags` bits to clear.
        clear_load_flags: u8,
    },
    /// Restore selected map chunks to the base layout.
    RestoreMapChunks {
        /// Base chunk records.
        chunks: Vec<DataChunkWrite>,
    },
    /// Write map chunks relative to an actor's position.
    WriteActorMapChunks {
        /// Reference actor.
        actor: DataActorRef,
        /// Actor-relative chunk writes.
        chunks: Vec<DataActorChunkWrite>,
    },
    /// Write map chunks at absolute chunk coordinates.
    WriteMapChunks {
        /// Absolute chunk writes.
        chunks: Vec<DataChunkWrite>,
    },
    /// Put a character in a party slot.
    JoinParty {
        /// Zero-based party slot.
        slot: usize,
        /// Character id.
        who: u8,
    },
    /// Remove a character from the party.
    RemovePartyMember {
        /// Character id.
        who: u8,
    },
    /// Replace all five party slots at once.
    SetParty {
        /// Five optional character ids.
        slots: [Option<u8>; 5],
    },
    /// Copy one character field-object struct over another.
    CopyCharSlot {
        /// Source slot.
        from: usize,
        /// Destination slot.
        to: usize,
    },
    /// Exchange two character field-object structs.
    SwapCharSlots {
        /// First slot.
        a: usize,
        /// Second slot.
        b: usize,
    },
    /// Promote an NPC field object into a party slot.
    PromoteNpcToChar {
        /// Map NPC index.
        npc: usize,
        /// Character id.
        char_id: u8,
        /// Party slot.
        slot: usize,
        /// Art tile.
        art_tile: u16,
        /// Arrival facing.
        facing: DataDirection,
    },
    /// Despawn consecutive map NPCs.
    DespawnNpc {
        /// First NPC index.
        npc_index: usize,
        /// Count of consecutive objects.
        count: usize,
    },
    /// Override a map NPC's dialogue entry id.
    SetNpcDialogue {
        /// Map NPC index.
        npc_index: usize,
        /// New dialogue entry id.
        dialogue_id: u16,
    },
    /// Set an actor's art tile.
    SetArtTile {
        /// Target actor.
        actor: DataActorRef,
        /// VRAM tile number.
        tile: u16,
    },
    /// Add meseta to the party purse.
    AddMoney {
        /// Amount added.
        amount: u32,
    },
    /// Subtract meseta from the party purse.
    SubtractMoney {
        /// Amount subtracted.
        amount: u32,
    },
    /// Add an item to inventory.
    AddItem {
        /// Item id.
        item: u8,
    },
    /// Remove an item from inventory.
    RemoveItem {
        /// Item id.
        item: u8,
    },
    /// Heal every party member by `amount` HP and clear status.
    RestorePartyHp {
        /// Healing amount.
        amount: u16,
    },
    /// Recover full party HP/TP/status/skill uses.
    RecoverStats,
    /// Start an event battle by index.
    StartBattle {
        /// Event battle index.
        index: u16,
    },
    /// Set `Char_Move_Flags` bits.
    SetFollowMode {
        /// Raw flag bits.
        bits: u8,
    },
    /// Set `FieldObj_Step_Offset` (`0..=2`).
    SetStepOffset {
        /// Step speed offset.
        value: u8,
    },
    /// Set `World_Index` (`0..=5`).
    SetWorldIndex {
        /// World index.
        world: u8,
    },
    /// Overlap party followers onto leader.
    OverlapCharacters,
    /// Set cutscene sprite suppression mode.
    SetRenderSpritesInCutscene {
        /// Literal flag value.
        enabled: bool,
    },
    /// Create a presentation panel by id.
    PanelCreate {
        /// Panel id.
        id: u16,
    },
    /// Destroy the most recently created panel.
    PanelDestroyLast,
    /// Destroy all staged panels.
    PanelDestroyAll,
    /// Flush staged planes (1 frame).
    DmaPlanes,
    /// Return from the scene with explicit `d0` value.
    Return {
        /// Return value.
        value: u16,
    },
    /// End the scene.
    End,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_scene_program_round_trips_json_including_slice_ops() {
        let program = DataSceneProgram {
            format_version: DATA_SCENE_FORMAT_VERSION,
            name: "Spike_TestScene".to_owned(),
            event_id: 0x0008,
            target_map: Some(0x0020),
            ops: vec![
                DataSceneOp::PlaySound { id: 0xE2 },
                DataSceneOp::WriteActorMapChunks {
                    actor: DataActorRef::PartyMember { slot: 0 },
                    chunks: vec![
                        DataActorChunkWrite {
                            dx: 0,
                            dy: 0,
                            chunk_id: 0x42,
                        },
                        DataActorChunkWrite {
                            dx: 0,
                            dy: 32,
                            chunk_id: 0x44,
                        },
                    ],
                },
                DataSceneOp::WaitFrames { frames: 17 },
                DataSceneOp::SkipUnlessMap {
                    maps: vec![0x0020],
                    skip: 1,
                },
                DataSceneOp::SetFlag {
                    flag: DataFlag {
                        bank: DataFlagBank::Temp,
                        id: 0,
                    },
                    value: true,
                },
                DataSceneOp::End,
            ],
        };
        let json = program.to_json().expect("serialize");
        let parsed = DataSceneProgram::from_json(&json).expect("deserialize");
        assert_eq!(parsed, program);
    }

    #[test]
    fn unknown_fields_unknown_ops_and_bad_envelope_fail_closed() {
        let unknown_top = r#"{
            "format_version": 1,
            "name": "Bad",
            "event_id": 1,
            "extra_field": true,
            "ops": [{"op": "end"}]
        }"#;
        assert!(matches!(
            DataSceneProgram::from_json(unknown_top),
            Err(DataSceneError::Json(_))
        ));

        let unknown_op = r#"{
            "format_version": 1,
            "name": "Bad",
            "event_id": 1,
            "ops": [{"op": "not_a_real_op"}]
        }"#;
        assert!(matches!(
            DataSceneProgram::from_json(unknown_op),
            Err(DataSceneError::Json(_))
        ));

        let unknown_operand = r#"{
            "format_version": 1,
            "name": "Bad",
            "event_id": 1,
            "ops": [{"op": "wait", "ticks": 4, "bogus": 9}]
        }"#;
        assert!(matches!(
            DataSceneProgram::from_json(unknown_operand),
            Err(DataSceneError::Json(_))
        ));

        let bad_version = r#"{
            "format_version": 99,
            "name": "Bad",
            "event_id": 1,
            "ops": [{"op": "end"}]
        }"#;
        assert!(matches!(
            DataSceneProgram::from_json(bad_version),
            Err(DataSceneError::UnsupportedFormatVersion {
                found: 99,
                expected: 1
            })
        ));

        let empty_name = r#"{
            "format_version": 1,
            "name": "   ",
            "event_id": 1,
            "ops": [{"op": "end"}]
        }"#;
        assert_eq!(
            DataSceneProgram::from_json(empty_name),
            Err(DataSceneError::EmptyName)
        );
    }
}
