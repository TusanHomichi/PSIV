//! `battle/enemy_animations.json`: retail enemy attack-object presentation.
//!
//! This file is additive to `enemies.json`. The enemy record owns stats and AI;
//! this record owns the presentation dispatch reached after `Enemy_Attack` has
//! selected that enemy's attack routine.

use serde::{Deserialize, Serialize};

/// The provenance and build guard for the retail animation extraction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAnimationSource {
    /// Retail table label.
    pub label: String,
    /// First byte of `EnemyAttackOffs`.
    pub rom_offset: String,
    /// Exclusive end of the attack pointer table.
    pub table_end: String,
    /// Number of enemy attack records.
    pub record_count: u32,
    /// Bytes per attack pointer entry.
    pub record_bytes: u32,
    /// Verified ROM size.
    pub rom_size: u32,
    /// Verified ROM SHA-256.
    pub rom_sha256: String,
    /// Must be zero: the pack is retail, not the Grand Cross build.
    pub grand_cross: u8,
    /// The reference source's build setting, retained to make the trap visible.
    pub reference_clone: String,
    /// The reference source is known to be Grand Cross.
    pub reference_grand_cross: u8,
    /// Why the reference source cannot be byte authority.
    pub reference_role: String,
    /// All pointer tables whose bytes were transcribed into this file.
    pub tables: Vec<EnemyAnimationTableSource>,
}

/// One retail pointer table included in the animation extraction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAnimationTableSource {
    /// Table label.
    pub name: String,
    /// First ROM byte.
    pub rom_offset: String,
    /// Number of entries.
    pub entry_count: u32,
    /// Bytes per entry.
    pub entry_bytes: u32,
    /// Must be zero for every table in the pack.
    pub grand_cross: u8,
}

/// The extraction census, retained in typed form for pack diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAnimationCensus {
    /// Enemy records covered.
    pub enemy_count: u32,
    /// Distinct `EnemyAttackOffs` routine targets.
    pub distinct_attack_routines: u32,
    /// Enemies with an exact direct `Sound_Index` write.
    pub exact_sfx: u32,
    /// Enemies without an exact per-enemy sound binding.
    pub generic_sfx: u32,
    /// Enemy roots with a fixed mapping record in the decoded graph.
    pub frame_sequence_records: u32,
    /// Enemy roots with a positive-duration mapping consumed by `loc_256AE`.
    pub timed_frame_sequences: u32,
    /// Enemy roots whose mapping/sprite answer remains deferred.
    pub frame_sequence_deferred: u32,
    /// Enemy roots whose movement is not represented by the current Godot body.
    pub movement_deferred: u32,
    /// Enemy roots with proven fixed-frame timing.
    pub flash_timing: u32,
    /// Enemy roots whose retail sprite-sheet composition is not yet decoded.
    pub sprite_sheet_deferred: u32,
    /// Enemy roots whose movement writes are fully normalized for playback.
    #[serde(default)]
    pub movement_exact: u32,
    /// Enemy roots with movement writes that remain only partially modeled.
    #[serde(default)]
    pub movement_partial: u32,
    /// Enemy roots with no movement clock that can be attached to a frame.
    #[serde(default)]
    pub movement_status_deferred: u32,
    /// Enemy roots whose selected mapping records compose exactly.
    #[serde(default)]
    pub sprite_sheet_exact: u32,
    /// Enemy roots with some mapping records but unresolved composition.
    #[serde(default)]
    pub sprite_sheet_partial: u32,
    /// Enemy roots whose mapping records are not renderable yet.
    #[serde(default)]
    pub sprite_sheet_status_deferred: u32,
}

/// A three-way extraction result.  `partial` is deliberately distinct from
/// `deferred`: the renderer may use an exact subset only when the sidecar says
/// so, and never by treating a missing field as a green result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAnimationEvidence {
    /// `exact`, `partial`, or `deferred`.
    pub status: String,
    /// Retail/provenance reason for the status.
    pub reason: String,
}

impl Default for EnemyAnimationEvidence {
    fn default() -> Self {
        Self {
            status: "deferred".into(),
            reason: "field absent from an older animation pack".into(),
        }
    }
}

/// One direct or object-local retail sound write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAnimationSfxWrite {
    /// Battle object id, or `null` when the attack routine writes directly.
    pub object_id: Option<u16>,
    /// ROM address of the write.
    pub rom_offset: String,
    /// `Sound_Index` or the delayed object-local sound byte.
    pub dispatch: String,
    /// Raw retail sound id.
    pub sound_id: u8,
    /// Symbolic name when the sound table names this id.
    pub sound_name: Option<String>,
}

/// One fixed mapping sequence consumed by retail's `loc_256AE` helper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAnimationFrameSequence {
    /// Object whose `$8(a4)` mapping pointer is assigned.
    pub object_id: Option<u16>,
    /// ROM address of the assignment instruction.
    pub assignment_offset: String,
    /// ROM address of the `[duration, frame_count, pointers...]` record.
    pub mapping_offset: String,
    /// Number of ticks each mapping pointer remains active.
    pub frame_duration: u8,
    /// Number of mapping pointers.
    pub frame_count: u8,
    /// `frame_duration * frame_count`.
    pub total_frames: u16,
    /// Retail mapping record pointers. Their sprite-sheet interpretation is
    /// intentionally not claimed here.
    pub mapping_pointers: Vec<String>,
    /// The helper that consumes this timing, when proven.
    pub frame_timer_helper: Option<String>,
}

/// One enemy's selected attack routine and the presentation facts it proves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAnimation {
    /// Enemy record id.
    pub enemy_id: u16,
    /// Disassembly symbol from the stable enemy vocabulary.
    pub symbol: String,
    /// Cartridge display name, when decoded.
    pub display_name: Option<String>,
    /// Resolved `EnemyAttackOffs` target.
    pub routine_offset: String,
    /// Object ids created directly by the attack routine.
    pub root_object_ids: Vec<u16>,
    /// Recursively reachable object ids, in deterministic discovery order.
    pub object_ids: Vec<u16>,
    /// First direct retail `Sound_Index` id in the reachable animation graph.
    pub sfx_id: u8,
    /// Name of `sfx_id` in the extracted sound vocabulary.
    pub sfx_name: Option<String>,
    /// The selected dispatch write.
    pub dispatch: EnemyAnimationDispatch,
    /// Every direct and object-local sound write retained for provenance.
    pub sfx_writes: Vec<EnemyAnimationSfxWrite>,
    /// Fixed timing only when the helper and record are both proven.
    pub frame_sequence: Option<EnemyAnimationFrameSequence>,
    /// Normalized movement evidence from the selected object graph.
    #[serde(default)]
    pub movement: EnemyAnimationEvidence,
    /// Mapping-to-art composition evidence from the wave-1 enemy banks.
    #[serde(default)]
    pub composition: EnemyAnimationEvidence,
    /// Kept as a compact boolean for older callers and the runtime sidecar.
    pub movement_proven: bool,
    /// Fixed-frame timing is available for the presentation flash beat.
    pub flash_timing_proven: bool,
    /// The selected mapping records are safe to render as attack frames.
    pub sprite_sheet_proven: bool,
}

/// The selected retail write for an enemy attack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAnimationDispatch {
    /// Dispatch kind.
    pub kind: String,
    /// ROM address of the write.
    pub rom_offset: String,
    /// Object that owns the write, or `null` for a routine-level write.
    pub object_id: Option<u16>,
}

/// The complete additive animation file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAnimationsFile {
    /// Pack format version.
    pub format_version: u32,
    /// Record count declared by the extractor.
    pub count: u32,
    /// Retail provenance.
    pub source: EnemyAnimationSource,
    /// One record per enemy.
    pub animations: Vec<EnemyAnimation>,
    /// Extraction census.
    pub census: EnemyAnimationCensus,
}
