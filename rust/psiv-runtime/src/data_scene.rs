//! Bridge between `psiv-data`'s serialized scene DTOs and `psiv-core`'s
//! [`SceneOp`] runner, with load-time structural and pack cross-reference
//! validation (#793).
//!
//! This module is the runtime half of the bounded data-driven scenes spike. It
//! compiles a [`DataSceneProgram`] into an owned `Arc<[SceneOp]>`, runs
//! `psiv-core`'s structural validator, and cross-checks map IDs, dialogue
//! trees/entries, map NPC bounds, and live chunk patch references against the
//! loaded [`GameData`] pack so malformed scene data fails closed at load time.

use std::borrow::Cow;
use std::collections::{BTreeSet, VecDeque};
use std::fmt;
use std::sync::Arc;

use psiv_core::{
    ActorAnimation, ActorRef, Axis, CameraSubject, Cell, CharId, CoordCmp, DialogueId,
    DialogueSource, DialogueWindow, Direction, EventIndex, Flag, MoneyTest, NpcDrift, SceneOp,
    SceneValidationError, SystemLine, validate_scene_ops,
};
use psiv_data::{
    DataActorAnimation, DataActorRef, DataAxis, DataCameraSubject, DataCoordCmp,
    DataDialogueSource, DataDialogueWindow, DataDirection, DataFlag, DataFlagBank, DataMoneyTest,
    DataSceneError, DataSceneOp, DataSceneProgram, GameData, MapId, MapRecord,
};

/// Why a data-defined scene failed to load or validate against the active pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataSceneLoadError {
    /// Wire envelope or JSON parsing failed in `psiv-data`.
    Envelope(DataSceneError),
    /// Core structural or operand validation failed in `psiv-core`.
    Validation(SceneValidationError),
    /// A reference to pack metadata (map, dialogue tree/entry, NPC, or chunk)
    /// could not be resolved against the loaded [`GameData`].
    PackCrossRef {
        /// Offending instruction index, or `None` for program-header metadata.
        op_index: Option<usize>,
        /// Human-readable description of the unresolved or out-of-range reference.
        reason: String,
    },
}

impl DataSceneLoadError {
    fn pack_header(reason: impl Into<String>) -> Self {
        Self::PackCrossRef {
            op_index: None,
            reason: reason.into(),
        }
    }

    fn pack_op(op_index: usize, reason: impl Into<String>) -> Self {
        Self::PackCrossRef {
            op_index: Some(op_index),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for DataSceneLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Envelope(err) => err.fmt(f),
            Self::Validation(err) => err.fmt(f),
            Self::PackCrossRef {
                op_index: Some(index),
                reason,
            } => write!(f, "scene op {index}: {reason}"),
            Self::PackCrossRef {
                op_index: None,
                reason,
            } => write!(f, "scene header: {reason}"),
        }
    }
}

impl std::error::Error for DataSceneLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Envelope(err) => Some(err),
            Self::Validation(err) => Some(err),
            Self::PackCrossRef { .. } => None,
        }
    }
}

impl From<DataSceneError> for DataSceneLoadError {
    fn from(err: DataSceneError) -> Self {
        Self::Envelope(err)
    }
}

impl From<SceneValidationError> for DataSceneLoadError {
    fn from(err: SceneValidationError) -> Self {
        Self::Validation(err)
    }
}

/// A validated, runtime-ready scene compiled from a [`DataSceneProgram`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledDataScene {
    /// Human-readable scene identifier for diagnostics.
    pub name: String,
    /// Event index this scene handles (existing retail override or new id).
    pub event: EventIndex,
    /// Validated `psiv-core` op buffer executed by [`psiv_core::SceneRunner`].
    pub ops: Arc<[SceneOp]>,
}

/// Parses, translates, and validates a JSON data-defined scene against `data`.
pub fn compile_data_scene_json(
    json: &str,
    data: &GameData,
) -> Result<CompiledDataScene, DataSceneLoadError> {
    let program = DataSceneProgram::from_json(json)?;
    compile_data_scene(&program, data)
}

/// Translates and validates a [`DataSceneProgram`] against `data`.
///
/// Validation runs in two fail-closed stages:
/// 1. `psiv_core::validate_scene_ops` checks jump/skip bounds, flag banks,
///    party/character ranges, animation durations, and non-empty slices.
/// 2. Pack cross-reference validation checks `target_map`, `LoadMap`,
///    `SkipUnlessMap`, `SetDialogueTree`, `RunDialogue`, map-scoped NPC
///    indices, and map chunk patch coordinates/atlas IDs against `data`.
pub fn compile_data_scene(
    program: &DataSceneProgram,
    data: &GameData,
) -> Result<CompiledDataScene, DataSceneLoadError> {
    if program.format_version != psiv_data::DATA_SCENE_FORMAT_VERSION {
        return Err(DataSceneLoadError::Envelope(
            DataSceneError::UnsupportedFormatVersion {
                found: program.format_version,
                expected: psiv_data::DATA_SCENE_FORMAT_VERSION,
            },
        ));
    }
    if program.name.trim().is_empty() {
        return Err(DataSceneLoadError::Envelope(DataSceneError::EmptyName));
    }

    let ops: Vec<SceneOp> = program.ops.iter().map(convert_op).collect();
    validate_scene_ops(&ops)?;
    validate_pack_cross_refs(program, &ops, data)?;

    Ok(CompiledDataScene {
        name: program.name.clone(),
        event: EventIndex(program.event_id),
        ops: Arc::from(ops),
    })
}

fn validate_pack_cross_refs(
    program: &DataSceneProgram,
    ops: &[SceneOp],
    data: &GameData,
) -> Result<(), DataSceneLoadError> {
    let (init_map, init_tree) = if let Some(map_id) = program.target_map {
        let Some(record) = data.map(MapId(map_id)) else {
            return Err(DataSceneLoadError::pack_header(format!(
                "target_map 0x{map_id:03X} is not packed in GameData"
            )));
        };
        (Some(map_id), Some(record.dialogue_tree))
    } else {
        (None, None)
    };

    let dialogue = data.dialogue();

    // First verify context-independent pack references across every op (including
    // dead/unreachable code) so malformed map or tree addresses always fail closed.
    for (index, op) in ops.iter().enumerate() {
        match op {
            SceneOp::LoadMap { map, .. } => {
                if data.map(MapId(*map)).is_none() {
                    return Err(DataSceneLoadError::pack_op(
                        index,
                        format!("LoadMap target 0x{map:03X} is not packed in GameData"),
                    ));
                }
            }
            SceneOp::SkipUnlessMap { maps, .. } => {
                for &map_id in maps.as_ref() {
                    if !data.knows(MapId(map_id)) {
                        return Err(DataSceneLoadError::pack_op(
                            index,
                            format!("SkipUnlessMap map 0x{map_id:03X} is unknown to GameData"),
                        ));
                    }
                }
            }
            SceneOp::SetDialogueTree { rom_addr } => {
                if let Some(pack) = dialogue.as_deref()
                    && !pack
                        .trees
                        .trees
                        .iter()
                        .any(|tree| tree.rom_address() == Some(*rom_addr))
                {
                    return Err(DataSceneLoadError::pack_op(
                        index,
                        format!(
                            "SetDialogueTree ROM address 0x{rom_addr:06X} matches no dialogue tree in pack"
                        ),
                    ));
                }
            }
            _ => {}
        }
    }

    // Track reachable `(op_index, current_map, active_tree)` states across all
    // control-flow edges so branch-dependent `LoadMap` / `SetDialogueTree` ops
    // cannot mask an invalid NPC, chunk, or dialogue reference on another path.
    let mut visited: BTreeSet<(usize, Option<u16>, Option<u8>)> = BTreeSet::new();
    let mut queue: VecDeque<(usize, Option<u16>, Option<u8>)> = VecDeque::new();
    let start = (0, init_map, init_tree);
    visited.insert(start);
    queue.push_back(start);

    while let Some((index, current_map, active_tree)) = queue.pop_front() {
        if index >= ops.len() {
            continue;
        }
        let op = &ops[index];
        let (next_map, next_tree) = validate_op_in_context(
            index,
            op,
            current_map,
            active_tree,
            data,
            dialogue.as_deref(),
        )?;

        let mut push = |target: usize, map: Option<u16>, tree: Option<u8>| {
            if target < ops.len() && visited.insert((target, map, tree)) {
                queue.push_back((target, map, tree));
            }
        };

        match op {
            SceneOp::Return { .. } | SceneOp::End => {}
            SceneOp::Jump { to } => push(*to, next_map, next_tree),
            SceneOp::SkipOps { count } => {
                push(index + 1 + usize::from(*count), next_map, next_tree);
            }
            SceneOp::BranchDialogueEnd { if_ended } => {
                push(index + 1, next_map, next_tree);
                push(*if_ended, next_map, next_tree);
            }
            SceneOp::BranchFlag {
                if_set, if_clear, ..
            } => {
                push(*if_set, next_map, next_tree);
                push(*if_clear, next_map, next_tree);
            }
            SceneOp::BranchChoice { if_yes, if_no } => {
                push(*if_yes, next_map, next_tree);
                push(*if_no, next_map, next_tree);
            }
            SceneOp::BranchIfAligned {
                if_aligned, if_not, ..
            } => {
                push(*if_aligned, next_map, next_tree);
                push(*if_not, next_map, next_tree);
            }
            SceneOp::BranchIfActorGreater {
                if_greater, if_not, ..
            } => {
                push(*if_greater, next_map, next_tree);
                push(*if_not, next_map, next_tree);
            }
            SceneOp::BranchIfActorCoord {
                if_true, if_false, ..
            } => {
                push(*if_true, next_map, next_tree);
                push(*if_false, next_map, next_tree);
            }
            SceneOp::BranchIfSavedDialogueByte {
                if_equal, if_not, ..
            } => {
                push(*if_equal, next_map, next_tree);
                push(*if_not, next_map, next_tree);
            }
            SceneOp::BranchIfVehicle {
                if_mounted,
                if_on_foot,
            } => {
                push(*if_mounted, next_map, next_tree);
                push(*if_on_foot, next_map, next_tree);
            }
            SceneOp::BranchIfMoneyBelow {
                if_below, if_not, ..
            } => {
                push(*if_below, next_map, next_tree);
                push(*if_not, next_map, next_tree);
            }
            SceneOp::BranchIfInventoryFull { if_full, if_not } => {
                push(*if_full, next_map, next_tree);
                push(*if_not, next_map, next_tree);
            }
            SceneOp::BranchIfPartyMember {
                if_present,
                if_absent,
                ..
            } => {
                push(*if_present, next_map, next_tree);
                push(*if_absent, next_map, next_tree);
            }
            SceneOp::BranchOnRngLow2 { targets } => {
                for &target in targets {
                    push(target, next_map, next_tree);
                }
            }
            SceneOp::SkipUnlessMap { maps, skip } => {
                let fallthrough_ok = current_map.is_none_or(|m| maps.as_ref().contains(&m));
                let skip_ok = current_map.is_none_or(|m| !maps.as_ref().contains(&m));
                if fallthrough_ok {
                    let narrowed = match (current_map, maps.as_ref()) {
                        (None, [only]) => data
                            .map(MapId(*only))
                            .map(|r| (Some(*only), Some(r.dialogue_tree))),
                        _ => None,
                    };
                    let (ft_map, ft_tree) = narrowed.unwrap_or((next_map, next_tree));
                    push(index + 1, ft_map, ft_tree);
                }
                if skip_ok {
                    push(index + 1 + usize::from(*skip), next_map, next_tree);
                }
            }
            SceneOp::DestinationMenu { cancel_skip, .. } => {
                push(index + 1, next_map, next_tree);
                push(index + 1 + usize::from(*cancel_skip), next_map, next_tree);
            }
            _ => push(index + 1, next_map, next_tree),
        }
    }

    Ok(())
}

fn validate_op_in_context(
    index: usize,
    op: &SceneOp,
    current_map: Option<u16>,
    active_tree: Option<u8>,
    data: &GameData,
    dialogue: Option<&psiv_data::DialogueSet>,
) -> Result<(Option<u16>, Option<u8>), DataSceneLoadError> {
    let mut next_map = current_map;
    let mut next_tree = active_tree;
    let map_record = current_map.and_then(|id| data.map(MapId(id)));

    match op {
        SceneOp::LoadMap { map, .. } => {
            let record = data
                .map(MapId(*map))
                .expect("LoadMap target verified in pre-pass");
            next_map = Some(*map);
            next_tree = Some(record.dialogue_tree);
        }
        SceneOp::CommitDestination => {
            next_map = None;
            next_tree = None;
        }
        SceneOp::SetDialogueTree { rom_addr } => {
            if let Some(pack) = dialogue {
                let tree = pack
                    .trees
                    .trees
                    .iter()
                    .find(|tree| tree.rom_address() == Some(*rom_addr))
                    .expect("SetDialogueTree address verified in pre-pass");
                next_tree = Some(tree.tree);
            }
        }
        SceneOp::RunDialogue { source, .. } => match source {
            DialogueSource::Entry(entry_id) => {
                if let (Some(pack), Some(tree_num)) = (dialogue, active_tree)
                    && pack.entry(tree_num, entry_id.0).is_none()
                {
                    return Err(DataSceneLoadError::pack_op(
                        index,
                        format!(
                            "RunDialogue entry 0x{:02X} does not exist in dialogue tree {tree_num}",
                            entry_id.0
                        ),
                    ));
                }
            }
            DialogueSource::NpcDialogueId(actor) => {
                check_npc_actor(index, *actor, map_record)?;
            }
            DialogueSource::System(_) => {}
        },
        SceneOp::DespawnNpc { npc_index, count } => {
            if let Some(record) = map_record {
                let end = npc_index.checked_add(*count);
                if !end.is_some_and(|e| e <= record.npcs.len()) {
                    return Err(DataSceneLoadError::pack_op(
                        index,
                        format!(
                            "DespawnNpc range {}..+{} exceeds map 0x{:03X} NPC count {}",
                            npc_index,
                            count,
                            record.id.0,
                            record.npcs.len()
                        ),
                    ));
                }
            }
        }
        SceneOp::SetNpcDialogue {
            npc_index,
            dialogue_id,
        } => {
            check_npc_index(index, *npc_index, map_record)?;
            if let (Some(pack), Some(tree_num)) = (dialogue, active_tree)
                && pack.entry(tree_num, *dialogue_id).is_none()
            {
                return Err(DataSceneLoadError::pack_op(
                    index,
                    format!(
                        "SetNpcDialogue entry 0x{dialogue_id:02X} does not exist in dialogue tree {tree_num}"
                    ),
                ));
            }
        }
        SceneOp::PromoteNpcToChar { npc, .. } => {
            check_npc_index(index, *npc, map_record)?;
        }
        SceneOp::DriftNpcs { drifts, .. } => {
            for drift in drifts.as_ref() {
                check_npc_index(index, drift.npc, map_record)?;
            }
        }
        SceneOp::WriteMapChunks { chunks } => {
            if let Some(record) = map_record {
                for &(x, y, chunk_id) in chunks.as_ref() {
                    if x >= record.dimensions.width_chunks || y >= record.dimensions.height_chunks {
                        return Err(DataSceneLoadError::pack_op(
                            index,
                            format!(
                                "WriteMapChunks coordinate ({x},{y}) is outside map 0x{:03X} ({}x{} chunks)",
                                record.id.0,
                                record.dimensions.width_chunks,
                                record.dimensions.height_chunks
                            ),
                        ));
                    }
                    check_patch_chunk(index, record, Some((x, y)), chunk_id)?;
                }
            }
        }
        SceneOp::WriteActorMapChunks { actor, chunks } => {
            check_npc_actor(index, *actor, map_record)?;
            if let Some(record) = map_record {
                for &(_, _, chunk_id) in chunks.as_ref() {
                    check_patch_chunk(index, record, None, chunk_id)?;
                }
            }
        }
        SceneOp::RestoreMapChunks { chunks } => {
            if let Some(record) = map_record {
                for &(x, y, _) in chunks.as_ref() {
                    if x >= record.dimensions.width_chunks || y >= record.dimensions.height_chunks {
                        return Err(DataSceneLoadError::pack_op(
                            index,
                            format!(
                                "RestoreMapChunks coordinate ({x},{y}) is outside map 0x{:03X} ({}x{} chunks)",
                                record.id.0,
                                record.dimensions.width_chunks,
                                record.dimensions.height_chunks
                            ),
                        ));
                    }
                }
            }
        }
        SceneOp::PanCamera { subject, .. } => match *subject {
            CameraSubject::At { .. } => {}
            CameraSubject::Actor(actor)
            | CameraSubject::ActorX { actor, .. }
            | CameraSubject::ActorShifted { actor, .. } => {
                check_npc_actor(index, actor, map_record)?;
            }
        },
        SceneOp::BranchIfAligned { a, b, .. } | SceneOp::BranchIfActorGreater { a, b, .. } => {
            check_npc_actor(index, *a, map_record)?;
            check_npc_actor(index, *b, map_record)?;
        }
        SceneOp::FaceOppositeOf { actor, of } => {
            check_npc_actor(index, *actor, map_record)?;
            check_npc_actor(index, *of, map_record)?;
        }
        SceneOp::MoveActorToActor { actor, target, .. }
        | SceneOp::MoveActorToActorAxis { actor, target, .. } => {
            check_npc_actor(index, *actor, map_record)?;
            check_npc_actor(index, *target, map_record)?;
        }
        SceneOp::BranchIfActorCoord { actor, .. }
        | SceneOp::MoveActor { actor, .. }
        | SceneOp::MoveActorTo { actor, .. }
        | SceneOp::MoveActorOffset { actor, .. }
        | SceneOp::MoveActorCommand { actor, .. }
        | SceneOp::PlaceActor { actor, .. }
        | SceneOp::SetActorDest { actor, .. }
        | SceneOp::Face { actor, .. }
        | SceneOp::SetActorAnimation { actor, .. }
        | SceneOp::WaitForActor { actor }
        | SceneOp::SetArtTile { actor, .. } => {
            check_npc_actor(index, *actor, map_record)?;
        }
        _ => {}
    }

    Ok((next_map, next_tree))
}

fn check_npc_actor(
    op_index: usize,
    actor: ActorRef,
    map_record: Option<&MapRecord>,
) -> Result<(), DataSceneLoadError> {
    if let ActorRef::Npc(npc_index) = actor {
        check_npc_index(op_index, npc_index, map_record)?;
    }
    Ok(())
}

fn check_npc_index(
    op_index: usize,
    npc_index: usize,
    map_record: Option<&MapRecord>,
) -> Result<(), DataSceneLoadError> {
    if let Some(record) = map_record
        && npc_index >= record.npcs.len()
    {
        return Err(DataSceneLoadError::pack_op(
            op_index,
            format!(
                "NPC index {npc_index} exceeds map 0x{:03X} NPC count {}",
                record.id.0,
                record.npcs.len()
            ),
        ));
    }
    Ok(())
}

fn check_patch_chunk(
    op_index: usize,
    record: &MapRecord,
    at: Option<(u32, u32)>,
    chunk_id: u16,
) -> Result<(), DataSceneLoadError> {
    let Some(atlas) = record.patch_tiles.as_ref() else {
        return Err(DataSceneLoadError::pack_op(
            op_index,
            format!(
                "chunk write 0x{chunk_id:02X} requires a patch_tiles atlas on map 0x{:03X}",
                record.id.0
            ),
        ));
    };
    let in_atlas = atlas
        .tiles
        .iter()
        .any(|tile| tile.chunk_id == Some(chunk_id));
    let in_overworld = record.overworld_patches.as_ref().is_some_and(|patches| {
        patches.iter().flat_map(|patch| &patch.tiles).any(|entry| {
            entry.collision_chunk_id == chunk_id
                && at.is_none_or(|(x, y)| entry.chunk_x == x && entry.chunk_y == y)
        })
    });
    if !in_atlas && !in_overworld {
        return Err(DataSceneLoadError::pack_op(
            op_index,
            format!(
                "chunk id 0x{chunk_id:02X} is absent from map 0x{:03X}'s patch atlas",
                record.id.0
            ),
        ));
    }
    Ok(())
}

fn convert_direction(dir: DataDirection) -> Direction {
    match dir {
        DataDirection::Up => Direction::Up,
        DataDirection::Down => Direction::Down,
        DataDirection::Left => Direction::Left,
        DataDirection::Right => Direction::Right,
    }
}

fn convert_axis(axis: DataAxis) -> Axis {
    match axis {
        DataAxis::X => Axis::X,
        DataAxis::Y => Axis::Y,
    }
}

fn convert_cmp(cmp: DataCoordCmp) -> CoordCmp {
    match cmp {
        DataCoordCmp::Equal => CoordCmp::Equal,
        DataCoordCmp::NotEqual => CoordCmp::NotEqual,
        DataCoordCmp::Below => CoordCmp::Below,
        DataCoordCmp::AtMost => CoordCmp::AtMost,
        DataCoordCmp::Above => CoordCmp::Above,
        DataCoordCmp::AtLeast => CoordCmp::AtLeast,
    }
}

fn convert_flag(flag: DataFlag) -> Flag {
    match flag.bank {
        DataFlagBank::Event => Flag::event(flag.id),
        DataFlagBank::Temp => Flag::temp(flag.id),
        DataFlagBank::Town => Flag::town(flag.id),
    }
}

fn convert_actor(actor: DataActorRef) -> ActorRef {
    match actor {
        DataActorRef::PartyMember { slot } => ActorRef::PartyMember(slot),
        DataActorRef::Npc { index } => ActorRef::Npc(index),
        DataActorRef::Character { id } => ActorRef::Character(CharId(id)),
    }
}

fn convert_dialogue_source(source: DataDialogueSource) -> DialogueSource {
    match source {
        DataDialogueSource::Entry { id } => DialogueSource::Entry(DialogueId(id)),
        DataDialogueSource::NpcDialogueId { actor } => {
            DialogueSource::NpcDialogueId(convert_actor(actor))
        }
        DataDialogueSource::CannotGetOff => DialogueSource::System(SystemLine::CannotGetOff),
    }
}

fn convert_dialogue_window(window: DataDialogueWindow) -> DialogueWindow {
    match window {
        DataDialogueWindow::Standard => DialogueWindow::Standard,
        DataDialogueWindow::Retained => DialogueWindow::Retained,
        DataDialogueWindow::Cutscene => DialogueWindow::Cutscene,
        DataDialogueWindow::Cutscene3 => DialogueWindow::Cutscene3,
        DataDialogueWindow::EndingIntro => DialogueWindow::EndingIntro,
        DataDialogueWindow::Ending => DialogueWindow::Ending,
        DataDialogueWindow::Cutscene5 => DialogueWindow::Cutscene5,
        DataDialogueWindow::Inline => DialogueWindow::Inline,
    }
}

fn convert_camera_subject(subject: DataCameraSubject) -> CameraSubject {
    match subject {
        DataCameraSubject::At { x, y } => CameraSubject::At { x, y },
        DataCameraSubject::Actor { actor } => CameraSubject::Actor(convert_actor(actor)),
        DataCameraSubject::ActorX { actor, y } => CameraSubject::ActorX {
            actor: convert_actor(actor),
            y,
        },
        DataCameraSubject::ActorShifted { actor, dx } => CameraSubject::ActorShifted {
            actor: convert_actor(actor),
            dx,
        },
    }
}

fn convert_money_test(test: DataMoneyTest) -> MoneyTest {
    match test {
        DataMoneyTest::SignOfDifference => MoneyTest::SignOfDifference,
        DataMoneyTest::Unsigned => MoneyTest::Unsigned,
    }
}

fn convert_animation(animation: &DataActorAnimation) -> ActorAnimation {
    ActorAnimation {
        word: animation.word,
        index: animation.index,
        duration: animation.duration,
        durations: Cow::Owned(animation.durations.clone()),
    }
}

fn convert_op(op: &DataSceneOp) -> SceneOp {
    match op {
        DataSceneOp::RunDialogue { source, window } => SceneOp::RunDialogue {
            source: convert_dialogue_source(*source),
            window: convert_dialogue_window(*window),
        },
        DataSceneOp::RunDialogueResume => SceneOp::RunDialogueResume,
        DataSceneOp::RunDialogueResumeWithWindow { window } => {
            SceneOp::RunDialogueResumeWithWindow {
                window: convert_dialogue_window(*window),
            }
        }
        DataSceneOp::SetDialogueTree { rom_addr } => SceneOp::SetDialogueTree {
            rom_addr: *rom_addr,
        },
        DataSceneOp::SetFlag { flag, value } => SceneOp::SetFlag {
            flag: convert_flag(*flag),
            value: *value,
        },
        DataSceneOp::ToggleFlag { flag } => SceneOp::ToggleFlag {
            flag: convert_flag(*flag),
        },
        DataSceneOp::Jump { to } => SceneOp::Jump { to: *to },
        DataSceneOp::SkipOps { count } => SceneOp::SkipOps { count: *count },
        DataSceneOp::SkipUnlessMap { maps, skip } => SceneOp::SkipUnlessMap {
            maps: Cow::Owned(maps.clone()),
            skip: *skip,
        },
        DataSceneOp::BranchFlag {
            flag,
            if_set,
            if_clear,
        } => SceneOp::BranchFlag {
            flag: convert_flag(*flag),
            if_set: *if_set,
            if_clear: *if_clear,
        },
        DataSceneOp::BranchChoice { if_yes, if_no } => SceneOp::BranchChoice {
            if_yes: *if_yes,
            if_no: *if_no,
        },
        DataSceneOp::BranchDialogueEnd { if_ended } => SceneOp::BranchDialogueEnd {
            if_ended: *if_ended,
        },
        DataSceneOp::BranchIfAligned {
            a,
            b,
            axis,
            if_aligned,
            if_not,
        } => SceneOp::BranchIfAligned {
            a: convert_actor(*a),
            b: convert_actor(*b),
            axis: convert_axis(*axis),
            if_aligned: *if_aligned,
            if_not: *if_not,
        },
        DataSceneOp::BranchIfActorGreater {
            a,
            b,
            axis,
            if_greater,
            if_not,
        } => SceneOp::BranchIfActorGreater {
            a: convert_actor(*a),
            b: convert_actor(*b),
            axis: convert_axis(*axis),
            if_greater: *if_greater,
            if_not: *if_not,
        },
        DataSceneOp::BranchIfActorCoord {
            actor,
            axis,
            cmp,
            value,
            if_true,
            if_false,
        } => SceneOp::BranchIfActorCoord {
            actor: convert_actor(*actor),
            axis: convert_axis(*axis),
            cmp: convert_cmp(*cmp),
            value: *value,
            if_true: *if_true,
            if_false: *if_false,
        },
        DataSceneOp::BranchIfPartyMember {
            who,
            if_present,
            if_absent,
        } => SceneOp::BranchIfPartyMember {
            who: CharId(*who),
            if_present: *if_present,
            if_absent: *if_absent,
        },
        DataSceneOp::BranchIfMoneyBelow {
            amount,
            test,
            if_below,
            if_not,
        } => SceneOp::BranchIfMoneyBelow {
            amount: *amount,
            test: convert_money_test(*test),
            if_below: *if_below,
            if_not: *if_not,
        },
        DataSceneOp::BranchIfInventoryFull { if_full, if_not } => SceneOp::BranchIfInventoryFull {
            if_full: *if_full,
            if_not: *if_not,
        },
        DataSceneOp::BranchIfVehicle {
            if_mounted,
            if_on_foot,
        } => SceneOp::BranchIfVehicle {
            if_mounted: *if_mounted,
            if_on_foot: *if_on_foot,
        },
        DataSceneOp::MoveActor { actor, to } => SceneOp::MoveActor {
            actor: convert_actor(*actor),
            to: Cell::new(to.x, to.y),
        },
        DataSceneOp::MoveActorTo { actor, x, y, wait } => SceneOp::MoveActorTo {
            actor: convert_actor(*actor),
            x: *x,
            y: *y,
            wait: *wait,
        },
        DataSceneOp::MoveActorToActor {
            actor,
            target,
            wait,
        } => SceneOp::MoveActorToActor {
            actor: convert_actor(*actor),
            target: convert_actor(*target),
            wait: *wait,
        },
        DataSceneOp::MoveActorToActorAxis {
            actor,
            target,
            axis,
            wait,
        } => SceneOp::MoveActorToActorAxis {
            actor: convert_actor(*actor),
            target: convert_actor(*target),
            axis: convert_axis(*axis),
            wait: *wait,
        },
        DataSceneOp::MoveActorOffset {
            actor,
            dx,
            dy,
            wait,
        } => SceneOp::MoveActorOffset {
            actor: convert_actor(*actor),
            dx: *dx,
            dy: *dy,
            wait: *wait,
        },
        DataSceneOp::MoveActorCommand {
            actor,
            command,
            wait,
        } => SceneOp::MoveActorCommand {
            actor: convert_actor(*actor),
            command: *command,
            wait: *wait,
        },
        DataSceneOp::PlaceActor { actor, x, y } => SceneOp::PlaceActor {
            actor: convert_actor(*actor),
            x: *x,
            y: *y,
        },
        DataSceneOp::SetActorDest { actor, x, y } => SceneOp::SetActorDest {
            actor: convert_actor(*actor),
            x: *x,
            y: *y,
        },
        DataSceneOp::Face { actor, facing } => SceneOp::Face {
            actor: convert_actor(*actor),
            facing: convert_direction(*facing),
        },
        DataSceneOp::FaceOppositeOf { actor, of } => SceneOp::FaceOppositeOf {
            actor: convert_actor(*actor),
            of: convert_actor(*of),
        },
        DataSceneOp::FaceParty { facing } => SceneOp::FaceParty {
            facing: convert_direction(*facing),
        },
        DataSceneOp::SetActorAnimation { actor, animation } => SceneOp::SetActorAnimation {
            actor: convert_actor(*actor),
            animation: convert_animation(animation),
        },
        DataSceneOp::DriftNpcs { drifts, frames } => SceneOp::DriftNpcs {
            drifts: Cow::Owned(
                drifts
                    .iter()
                    .map(|d| NpcDrift {
                        npc: d.npc,
                        step_x: d.step_x,
                        step_y: d.step_y,
                    })
                    .collect(),
            ),
            frames: *frames,
        },
        DataSceneOp::Wait { ticks } => SceneOp::Wait { ticks: *ticks },
        DataSceneOp::WaitFrames { frames } => SceneOp::WaitFrames { frames: *frames },
        DataSceneOp::WaitForActor { actor } => SceneOp::WaitForActor {
            actor: convert_actor(*actor),
        },
        DataSceneOp::PanCamera { subject, speed } => SceneOp::PanCamera {
            subject: convert_camera_subject(*subject),
            speed: *speed,
        },
        DataSceneOp::PlaySound { id } => SceneOp::PlaySound { id: *id },
        DataSceneOp::SetSavedMusic { id } => SceneOp::SetSavedMusic { id: *id },
        DataSceneOp::FadeIn => SceneOp::FadeIn,
        DataSceneOp::FadeOut => SceneOp::FadeOut,
        DataSceneOp::InitVramAndCram => SceneOp::InitVramAndCram,
        DataSceneOp::LoadMap {
            map,
            prev_map,
            start_x,
            start_y,
            facing,
            align,
            clear_load_flags,
        } => SceneOp::LoadMap {
            map: *map,
            prev_map: *prev_map,
            start_x: *start_x,
            start_y: *start_y,
            facing: convert_direction(*facing),
            align: *align,
            clear_load_flags: *clear_load_flags,
        },
        DataSceneOp::RestoreMapChunks { chunks } => SceneOp::RestoreMapChunks {
            chunks: Cow::Owned(chunks.iter().map(|c| (c.x, c.y, c.chunk_id)).collect()),
        },
        DataSceneOp::WriteActorMapChunks { actor, chunks } => SceneOp::WriteActorMapChunks {
            actor: convert_actor(*actor),
            chunks: Cow::Owned(chunks.iter().map(|c| (c.dx, c.dy, c.chunk_id)).collect()),
        },
        DataSceneOp::WriteMapChunks { chunks } => SceneOp::WriteMapChunks {
            chunks: Cow::Owned(chunks.iter().map(|c| (c.x, c.y, c.chunk_id)).collect()),
        },
        DataSceneOp::JoinParty { slot, who } => SceneOp::JoinParty {
            slot: *slot,
            who: CharId(*who),
        },
        DataSceneOp::RemovePartyMember { who } => SceneOp::RemovePartyMember { who: CharId(*who) },
        DataSceneOp::SetParty { slots } => SceneOp::SetParty {
            slots: slots.map(|id| id.map(CharId)),
        },
        DataSceneOp::CopyCharSlot { from, to } => SceneOp::CopyCharSlot {
            from: *from,
            to: *to,
        },
        DataSceneOp::SwapCharSlots { a, b } => SceneOp::SwapCharSlots { a: *a, b: *b },
        DataSceneOp::PromoteNpcToChar {
            npc,
            char_id,
            slot,
            art_tile,
            facing,
        } => SceneOp::PromoteNpcToChar {
            npc: *npc,
            char_id: CharId(*char_id),
            slot: *slot,
            art_tile: *art_tile,
            facing: convert_direction(*facing),
        },
        DataSceneOp::DespawnNpc { npc_index, count } => SceneOp::DespawnNpc {
            npc_index: *npc_index,
            count: *count,
        },
        DataSceneOp::SetNpcDialogue {
            npc_index,
            dialogue_id,
        } => SceneOp::SetNpcDialogue {
            npc_index: *npc_index,
            dialogue_id: *dialogue_id,
        },
        DataSceneOp::SetArtTile { actor, tile } => SceneOp::SetArtTile {
            actor: convert_actor(*actor),
            tile: *tile,
        },
        DataSceneOp::AddMoney { amount } => SceneOp::AddMoney { amount: *amount },
        DataSceneOp::SubtractMoney { amount } => SceneOp::SubtractMoney { amount: *amount },
        DataSceneOp::AddItem { item } => SceneOp::AddItem { item: *item },
        DataSceneOp::RemoveItem { item } => SceneOp::RemoveItem { item: *item },
        DataSceneOp::RestorePartyHp { amount } => SceneOp::RestorePartyHp { amount: *amount },
        DataSceneOp::RecoverStats => SceneOp::RecoverStats,
        DataSceneOp::StartBattle { index } => SceneOp::StartBattle { index: *index },
        DataSceneOp::SetFollowMode { bits } => SceneOp::SetFollowMode { bits: *bits },
        DataSceneOp::SetStepOffset { value } => SceneOp::SetStepOffset { value: *value },
        DataSceneOp::SetWorldIndex { world } => SceneOp::SetWorldIndex { world: *world },
        DataSceneOp::OverlapCharacters => SceneOp::OverlapCharacters,
        DataSceneOp::SetRenderSpritesInCutscene { enabled } => {
            SceneOp::SetRenderSpritesInCutscene { enabled: *enabled }
        }
        DataSceneOp::PanelCreate { id } => SceneOp::PanelCreate { id: *id },
        DataSceneOp::PanelDestroyLast => SceneOp::PanelDestroyLast,
        DataSceneOp::PanelDestroyAll => SceneOp::PanelDestroyAll,
        DataSceneOp::DmaPlanes => SceneOp::DmaPlanes,
        DataSceneOp::Return { value } => SceneOp::Return { value: *value },
        DataSceneOp::End => SceneOp::End,
    }
}
