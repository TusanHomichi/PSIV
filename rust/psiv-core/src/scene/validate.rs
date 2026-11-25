//! Load-time validation for [`SceneOp`] programs (#793).
//!
//! Compiled retail scenes and dynamically loaded data scenes share one
//! validator so malformed scene programs fail closed at load time with the
//! offending op index and reason instead of faulting mid-cutscene.

use std::fmt;

use crate::roster::CHARACTER_COUNT;
use crate::scene::{
    ActorRef, DialogueSource, FifthOp, GuildOp, ItemOp, ObjectOp, RockyOp, SceneOp, WreckageOp,
};
use crate::scene_presentation::{CameraSubject, PresentationOp};
use crate::scene_types::GetOffOp;
use crate::scenes::WORLD_COUNT;
use crate::state::{CharId, Flag, PARTY_SLOTS};

/// Why a [`SceneOp`] program was rejected at load time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneValidationError {
    /// Zero-based index of the offending instruction in the scene's op slice.
    pub op_index: usize,
    /// Human-readable reason the instruction failed validation.
    pub reason: String,
}

impl SceneValidationError {
    fn new(op_index: usize, reason: impl Into<String>) -> Self {
        Self {
            op_index,
            reason: reason.into(),
        }
    }
}

impl fmt::Display for SceneValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scene op {}: {}", self.op_index, self.reason)
    }
}

impl std::error::Error for SceneValidationError {}

fn check_jump(op_index: usize, target: usize, len: usize) -> Result<(), SceneValidationError> {
    if target > len {
        return Err(SceneValidationError::new(
            op_index,
            format!("jump target {target} exceeds scene length {len}"),
        ));
    }
    Ok(())
}

fn check_skip(op_index: usize, skip: u16, len: usize) -> Result<(), SceneValidationError> {
    let dest = op_index
        .checked_add(1)
        .and_then(|next| next.checked_add(usize::from(skip)));
    if !dest.is_some_and(|target| target <= len) {
        return Err(SceneValidationError::new(
            op_index,
            format!("relative skip of {skip} ops lands past scene length {len}"),
        ));
    }
    Ok(())
}

fn check_flag(op_index: usize, flag: Flag) -> Result<(), SceneValidationError> {
    let cap = flag.bank.capacity();
    if flag.id >= cap {
        return Err(SceneValidationError::new(
            op_index,
            format!(
                "flag {:?}({}) is out of bounds (capacity {cap})",
                flag.bank, flag.id
            ),
        ));
    }
    Ok(())
}

fn check_party_slot(op_index: usize, slot: usize) -> Result<(), SceneValidationError> {
    if slot >= PARTY_SLOTS {
        return Err(SceneValidationError::new(
            op_index,
            format!(
                "party slot {slot} is out of bounds (max {})",
                PARTY_SLOTS - 1
            ),
        ));
    }
    Ok(())
}

fn check_char(op_index: usize, who: CharId) -> Result<(), SceneValidationError> {
    if usize::from(who.0) >= CHARACTER_COUNT {
        return Err(SceneValidationError::new(
            op_index,
            format!(
                "character id {} is out of bounds (max {})",
                who.0,
                CHARACTER_COUNT - 1
            ),
        ));
    }
    Ok(())
}

fn check_actor(op_index: usize, actor: ActorRef) -> Result<(), SceneValidationError> {
    match actor {
        ActorRef::PartyMember(slot) => check_party_slot(op_index, slot),
        ActorRef::Character(who) => check_char(op_index, who),
        ActorRef::Npc(_) => Ok(()),
    }
}

fn check_camera_subject(
    op_index: usize,
    subject: CameraSubject,
) -> Result<(), SceneValidationError> {
    match subject {
        CameraSubject::At { .. } => Ok(()),
        CameraSubject::Actor(actor)
        | CameraSubject::ActorX { actor, .. }
        | CameraSubject::ActorShifted { actor, .. } => check_actor(op_index, actor),
    }
}

fn check_presentation(op_index: usize, op: &PresentationOp) -> Result<(), SceneValidationError> {
    match op {
        PresentationOp::SetCharacterVisible { who, .. } => check_char(op_index, *who),
        PresentationOp::RajaSickResetRaja { character, .. } => check_char(op_index, *character),
        PresentationOp::RajaSickArrangeParty {
            reference,
            companions,
        } => {
            check_char(op_index, *reference)?;
            if companions.is_empty() {
                return Err(SceneValidationError::new(
                    op_index,
                    "RajaSickArrangeParty companions slice must not be empty",
                ));
            }
            for &who in companions.as_ref() {
                check_char(op_index, who)?;
            }
            Ok(())
        }
        PresentationOp::RykrosPaletteCycle {
            words_per_frame,
            delays,
            ..
        } => {
            if *words_per_frame == 0 {
                return Err(SceneValidationError::new(
                    op_index,
                    "RykrosPaletteCycle words_per_frame must be nonzero",
                ));
            }
            if delays.is_empty() {
                return Err(SceneValidationError::new(
                    op_index,
                    "RykrosPaletteCycle delays slice must not be empty",
                ));
            }
            Ok(())
        }
        PresentationOp::AddMacro { slot } => {
            if *slot >= CHARACTER_COUNT {
                return Err(SceneValidationError::new(
                    op_index,
                    format!(
                        "AddMacro character id {slot} is out of bounds (max {})",
                        CHARACTER_COUNT - 1
                    ),
                ));
            }
            Ok(())
        }
        PresentationOp::BarrierBeamUpdates { .. }
        | PresentationOp::FieldReload { .. }
        | PresentationOp::PlaceNameWindow
        | PresentationOp::FadeToRed { .. }
        | PresentationOp::FadeFromRed { .. }
        | PresentationOp::ReloadMapChunks
        | PresentationOp::LoadSceneAsset { .. }
        | PresentationOp::RebuildSprites
        | PresentationOp::Quake { .. }
        | PresentationOp::WindowDestroy { .. }
        | PresentationOp::SavePartySpriteX { .. }
        | PresentationOp::RestorePartySpriteX
        | PresentationOp::ShadowHighlight { .. }
        | PresentationOp::SetPaletteWords { .. }
        | PresentationOp::SetGameMode { .. }
        | PresentationOp::ClearHeldInput
        | PresentationOp::WindowCreate { .. }
        | PresentationOp::LoadWindowTiles { .. }
        | PresentationOp::LoadPortrait { .. }
        | PresentationOp::DrawPortrait { .. }
        | PresentationOp::SetObjectDestination { .. }
        | PresentationOp::WalkObject { .. }
        | PresentationOp::DriftObject { .. }
        | PresentationOp::SetObjectFacingWord { .. }
        | PresentationOp::SetDialoguePortrait { .. }
        | PresentationOp::PaletteCopy { .. }
        | PresentationOp::ClearPlanes
        | PresentationOp::EnableDisplay
        | PresentationOp::EndingFinaleFieldPrep { .. }
        | PresentationOp::DmaPlanesLoop { .. }
        | PresentationOp::PaletteIncreaseTone { .. }
        | PresentationOp::PaletteRestoreTone { .. }
        | PresentationOp::AddPaletteWord { .. }
        | PresentationOp::PaletteToneFlash { .. }
        | PresentationOp::GaruberkEyeArtCycle { .. }
        | PresentationOp::VariablePaletteFade { .. }
        | PresentationOp::TwoLinePaletteFade { .. }
        | PresentationOp::PaletteRampFromTable { .. }
        | PresentationOp::PaletteCycleFromTable { .. }
        | PresentationOp::CounterPaletteCycle { .. }
        | PresentationOp::PaletteGrowIn { .. }
        | PresentationOp::NpcPixelPosition { .. }
        | PresentationOp::RajaSickTemporaryObject { .. }
        | PresentationOp::EndingCreditsAssets { .. }
        | PresentationOp::EndingCreditsStage { .. }
        | PresentationOp::EndingCreditsPaletteRamp { .. }
        | PresentationOp::EndingStaffRollFadeOut { .. }
        | PresentationOp::EndingStaffRollFadeIn { .. }
        | PresentationOp::EndingFinale { .. } => Ok(()),
    }
}

/// Validates a [`SceneOp`] program before execution.
///
/// Checks jump targets and relative skips are in bounds, flag/party/character
/// operands fit retail tables, slice-bearing ops carry non-empty payloads, and
/// animation sequences have valid indices. Pack-dependent references (map IDs,
/// dialogue trees, per-map NPC counts) are validated in `psiv-runtime`.
pub fn validate_scene_ops(ops: &[SceneOp]) -> Result<(), SceneValidationError> {
    if ops.is_empty() {
        return Err(SceneValidationError::new(
            0,
            "scene must contain at least one op",
        ));
    }
    let len = ops.len();
    for (index, op) in ops.iter().enumerate() {
        match op {
            SceneOp::Jump { to } => check_jump(index, *to, len)?,
            SceneOp::BranchDialogueEnd { if_ended } => check_jump(index, *if_ended, len)?,
            SceneOp::BranchFlag {
                flag,
                if_set,
                if_clear,
            } => {
                check_flag(index, *flag)?;
                check_jump(index, *if_set, len)?;
                check_jump(index, *if_clear, len)?;
            }
            SceneOp::BranchChoice { if_yes, if_no } => {
                check_jump(index, *if_yes, len)?;
                check_jump(index, *if_no, len)?;
            }
            SceneOp::BranchIfAligned {
                a,
                b,
                if_aligned,
                if_not,
                ..
            } => {
                check_actor(index, *a)?;
                check_actor(index, *b)?;
                check_jump(index, *if_aligned, len)?;
                check_jump(index, *if_not, len)?;
            }
            SceneOp::BranchIfActorGreater {
                a,
                b,
                if_greater,
                if_not,
                ..
            } => {
                check_actor(index, *a)?;
                check_actor(index, *b)?;
                check_jump(index, *if_greater, len)?;
                check_jump(index, *if_not, len)?;
            }
            SceneOp::BranchIfActorCoord {
                actor,
                if_true,
                if_false,
                ..
            } => {
                check_actor(index, *actor)?;
                check_jump(index, *if_true, len)?;
                check_jump(index, *if_false, len)?;
            }
            SceneOp::BranchIfSavedDialogueByte {
                if_equal, if_not, ..
            } => {
                check_jump(index, *if_equal, len)?;
                check_jump(index, *if_not, len)?;
            }
            SceneOp::BranchIfVehicle {
                if_mounted,
                if_on_foot,
            } => {
                check_jump(index, *if_mounted, len)?;
                check_jump(index, *if_on_foot, len)?;
            }
            SceneOp::BranchIfMoneyBelow {
                if_below, if_not, ..
            } => {
                check_jump(index, *if_below, len)?;
                check_jump(index, *if_not, len)?;
            }
            SceneOp::BranchIfInventoryFull { if_full, if_not } => {
                check_jump(index, *if_full, len)?;
                check_jump(index, *if_not, len)?;
            }
            SceneOp::BranchOnRngLow2 { targets } => {
                for &target in targets {
                    check_jump(index, target, len)?;
                }
            }
            SceneOp::BranchIfPartyMember {
                who,
                if_present,
                if_absent,
            } => {
                check_char(index, *who)?;
                check_jump(index, *if_present, len)?;
                check_jump(index, *if_absent, len)?;
            }
            SceneOp::SkipOps { count } => check_skip(index, *count, len)?,
            SceneOp::SkipUnlessMap { maps, skip } => {
                if maps.is_empty() {
                    return Err(SceneValidationError::new(
                        index,
                        "SkipUnlessMap maps slice must not be empty",
                    ));
                }
                check_skip(index, *skip, len)?;
            }
            SceneOp::DestinationMenu { cancel_skip, .. } => {
                check_skip(index, *cancel_skip, len)?;
            }
            SceneOp::RestoreMapChunks { chunks } => {
                if chunks.is_empty() {
                    return Err(SceneValidationError::new(
                        index,
                        "RestoreMapChunks chunks slice must not be empty",
                    ));
                }
            }
            SceneOp::WriteActorMapChunks { actor, chunks } => {
                check_actor(index, *actor)?;
                if chunks.is_empty() {
                    return Err(SceneValidationError::new(
                        index,
                        "WriteActorMapChunks chunks slice must not be empty",
                    ));
                }
            }
            SceneOp::WriteMapChunks { chunks } => {
                if chunks.is_empty() {
                    return Err(SceneValidationError::new(
                        index,
                        "WriteMapChunks chunks slice must not be empty",
                    ));
                }
            }
            SceneOp::DriftNpcs { drifts, frames } => {
                if drifts.is_empty() {
                    return Err(SceneValidationError::new(
                        index,
                        "DriftNpcs drifts slice must not be empty",
                    ));
                }
                if *frames == 0 {
                    return Err(SceneValidationError::new(
                        index,
                        "DriftNpcs frames must be nonzero",
                    ));
                }
            }
            SceneOp::SetActorAnimation { actor, animation } => {
                check_actor(index, *actor)?;
                if animation.durations.is_empty() || animation.durations.len() > 256 {
                    return Err(SceneValidationError::new(
                        index,
                        format!(
                            "SetActorAnimation durations length {} must be in 1..=256",
                            animation.durations.len()
                        ),
                    ));
                }
                if usize::from(animation.index) >= animation.durations.len() {
                    return Err(SceneValidationError::new(
                        index,
                        format!(
                            "SetActorAnimation index {} exceeds durations length {}",
                            animation.index,
                            animation.durations.len()
                        ),
                    ));
                }
            }
            SceneOp::SetFlag { flag, .. } | SceneOp::ToggleFlag { flag } => {
                check_flag(index, *flag)?;
            }
            SceneOp::MoveActor { actor, .. }
            | SceneOp::PlaceActor { actor, .. }
            | SceneOp::SetActorDest { actor, .. }
            | SceneOp::Face { actor, .. }
            | SceneOp::MoveActorTo { actor, .. }
            | SceneOp::MoveActorOffset { actor, .. }
            | SceneOp::MoveActorCommand { actor, .. }
            | SceneOp::WaitForActor { actor }
            | SceneOp::SetArtTile { actor, .. }
            | SceneOp::StepObjectAlongFacing { actor, .. } => check_actor(index, *actor)?,
            SceneOp::FaceOppositeOf { actor, of } => {
                check_actor(index, *actor)?;
                check_actor(index, *of)?;
            }
            SceneOp::MoveActorToActor { actor, target, .. }
            | SceneOp::MoveActorToActorAxis { actor, target, .. } => {
                check_actor(index, *actor)?;
                check_actor(index, *target)?;
            }
            SceneOp::SwapCharSlots { a, b } => {
                check_party_slot(index, *a)?;
                check_party_slot(index, *b)?;
            }
            SceneOp::CopyCharSlot { from, to } => {
                check_party_slot(index, *from)?;
                check_party_slot(index, *to)?;
            }
            SceneOp::JoinParty { slot, who } => {
                check_party_slot(index, *slot)?;
                check_char(index, *who)?;
            }
            SceneOp::RemovePartyMember { who }
            | SceneOp::ConfigureCharacter { who, .. }
            | SceneOp::SetCharacterEquipment { who, .. }
            | SceneOp::ClearCharacterStatus { who } => check_char(index, *who)?,
            SceneOp::SetParty { slots } => {
                for who in slots.iter().flatten() {
                    check_char(index, *who)?;
                }
            }
            SceneOp::PromoteNpcToChar { char_id, slot, .. } => {
                check_char(index, *char_id)?;
                check_party_slot(index, *slot)?;
            }
            SceneOp::LearnTechnique { character, .. } => {
                check_char(index, CharId(*character))?;
            }
            SceneOp::SetCharacterSkill { who, slot, .. } => {
                check_char(index, *who)?;
                if *slot >= 8 {
                    return Err(SceneValidationError::new(
                        index,
                        format!("SetCharacterSkill slot {slot} is out of bounds (max 7)"),
                    ));
                }
            }
            SceneOp::DespawnNpc { count, .. } => {
                if *count == 0 {
                    return Err(SceneValidationError::new(
                        index,
                        "DespawnNpc count must be nonzero",
                    ));
                }
            }
            SceneOp::RunDialogue { source, .. } => {
                if let DialogueSource::NpcDialogueId(actor) = source {
                    check_actor(index, *actor)?;
                }
            }
            SceneOp::PanCamera { subject, speed } => {
                check_camera_subject(index, *subject)?;
                if *speed == 0 {
                    return Err(SceneValidationError::new(
                        index,
                        "PanCamera speed must be nonzero",
                    ));
                }
            }
            SceneOp::SetStepOffset { value } => {
                if *value > 2 {
                    return Err(SceneValidationError::new(
                        index,
                        format!("SetStepOffset value {value} must be in 0..=2"),
                    ));
                }
            }
            SceneOp::SetWorldIndex { world } => {
                if *world >= WORLD_COUNT {
                    return Err(SceneValidationError::new(
                        index,
                        format!(
                            "SetWorldIndex world {world} is out of bounds (max {})",
                            WORLD_COUNT - 1
                        ),
                    ));
                }
            }
            SceneOp::CopyChunkDefinition { words, .. } => {
                if *words == 0 {
                    return Err(SceneValidationError::new(
                        index,
                        "CopyChunkDefinition words must be nonzero",
                    ));
                }
            }
            SceneOp::Item(ItemOp::Take { if_missing, .. }) => {
                check_jump(index, *if_missing, len)?;
            }
            SceneOp::FifthCharacter(FifthOp::BranchFifthSlotFree { if_free, if_taken }) => {
                check_jump(index, *if_free, len)?;
                check_jump(index, *if_taken, len)?;
            }
            SceneOp::Guild(GuildOp::SkipTreeLoad { skip, .. }) => {
                check_skip(index, *skip, len)?;
            }
            SceneOp::Rocky(RockyOp::BranchIfHolding {
                if_held, if_not, ..
            }) => {
                check_jump(index, usize::from(*if_held), len)?;
                check_jump(index, usize::from(*if_not), len)?;
            }
            SceneOp::Rocky(RockyOp::RunFast { actor }) => check_actor(index, *actor)?,
            SceneOp::GetOff(GetOffOp::BranchIfBlocked {
                if_blocked,
                if_clear,
            }) => {
                check_jump(index, usize::from(*if_blocked), len)?;
                check_jump(index, usize::from(*if_clear), len)?;
            }
            SceneOp::Wreckage(WreckageOp::BranchIfLeads {
                who,
                if_leads,
                if_follows,
            }) => {
                check_char(index, *who)?;
                check_jump(index, usize::from(*if_leads), len)?;
                check_jump(index, usize::from(*if_follows), len)?;
            }
            SceneOp::Object(ObjectOp::SpawnAt { at, .. }) => check_actor(index, *at)?,
            SceneOp::Presentation { op } => check_presentation(index, op)?,
            SceneOp::FlightPlanet
            | SceneOp::FlightPan
            | SceneOp::TakeMapTransition
            | SceneOp::ReloadMapPalette
            | SceneOp::ReplaceMapLayout { .. }
            | SceneOp::InitVramAndCram
            | SceneOp::Return { .. }
            | SceneOp::LoadPalette { .. }
            | SceneOp::LoadArt { .. }
            | SceneOp::SetCameraPos { .. }
            | SceneOp::LoadTitleImage { .. }
            | SceneOp::SetTextColour { .. }
            | SceneOp::DrawTextToPlane { .. }
            | SceneOp::IntroTextFadeUp
            | SceneOp::IntroTextFadeDown
            | SceneOp::RunDialogueResume
            | SceneOp::RunDialogueResumeWithWindow { .. }
            | SceneOp::SetDialogueTree { .. }
            | SceneOp::SavePartySlots
            | SceneOp::RestorePartySlots
            | SceneOp::RestorePartyHp { .. }
            | SceneOp::RunObjectRoutine { .. }
            | SceneOp::SetObjectRoutine { .. }
            | SceneOp::SetNpcDialogue { .. }
            | SceneOp::OverlapCharacters
            | SceneOp::PlacePartyY { .. }
            | SceneOp::AlignVehicleBoarding { .. }
            | SceneOp::AddMoney { .. }
            | SceneOp::SubtractMoney { .. }
            | SceneOp::StartBattle { .. }
            | SceneOp::SetFollowMode { .. }
            | SceneOp::PlaySound { .. }
            | SceneOp::PlayMusicIfSavedDifferent { .. }
            | SceneOp::SetSavedMusic { .. }
            | SceneOp::FadeIn
            | SceneOp::FadeOut
            | SceneOp::LoadMap { .. }
            | SceneOp::SetRenderSpritesInCutscene { .. }
            | SceneOp::WaitFrames { .. }
            | SceneOp::Wait { .. }
            | SceneOp::WaitForStart
            | SceneOp::SetVehicleIndex { .. }
            | SceneOp::PanelCreate { .. }
            | SceneOp::PanelDestroyLast
            | SceneOp::PanelDestroyAll
            | SceneOp::DmaPlanes
            | SceneOp::CreateFieldObject { .. }
            | SceneOp::CreateEclipseTorch(_)
            | SceneOp::StepFieldObject { .. }
            | SceneOp::ObjectAnimation { .. }
            | SceneOp::RemoveItem { .. }
            | SceneOp::AddItem { .. }
            | SceneOp::SetMapLoadFlags { .. }
            | SceneOp::RecoverStats
            | SceneOp::MarkGameCleared
            | SceneOp::CommitDestination
            | SceneOp::LoadFlightMap { .. }
            | SceneOp::FaceParty { .. }
            | SceneOp::Ride(_)
            | SceneOp::FifthCharacter(_)
            | SceneOp::Guild(_)
            | SceneOp::JobBoard(_)
            | SceneOp::Rocky(RockyOp::RemoveAll { .. })
            | SceneOp::GetOff(GetOffOp::PlayMapMusic)
            | SceneOp::Wreckage(WreckageOp::ResumePortraitFromParty)
            | SceneOp::Object(ObjectOp::Spawn { .. } | ObjectOp::PlacePartyFormation { .. })
            | SceneOp::End => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::scene_runner::drift::NpcDrift;
    use crate::scenes::SCENES;
    use crate::{
        ActorAnimation, Cell, CollisionGrid, Direction, FieldMap, GameState, MapId, Npc, NpcId,
        SceneInput, SceneRunner, ScriptedActor, StepFrames, SubCellOffset,
    };

    #[test]
    fn every_compiled_retail_scene_passes_validation() {
        for scene in SCENES {
            if let Err(err) = validate_scene_ops(scene.ops) {
                panic!(
                    "compiled retail scene {} failed validation: {err}",
                    scene.name
                );
            }
        }
    }

    #[test]
    fn owned_cow_scene_executes_identically_to_compiled_static_scene() {
        let compiled = crate::scene_for(crate::EventIndex(8)).expect("BioPlantDoorOpening");
        let owned: Vec<SceneOp> = compiled
            .ops
            .iter()
            .map(|op| match op {
                SceneOp::WriteActorMapChunks { actor, chunks } => SceneOp::WriteActorMapChunks {
                    actor: *actor,
                    chunks: Cow::Owned(chunks.to_vec()),
                },
                other => other.clone(),
            })
            .collect();
        validate_scene_ops(&owned).expect("owned scene validates");

        let grid = CollisionGrid::new(8, 8, vec![0; 64]).unwrap();
        let map = FieldMap::new(MapId(0x20), grid, vec![], vec![]).unwrap();
        let cast = vec![ScriptedActor::new(
            ActorRef::PartyMember(0),
            Cell::new(3, 3),
            Direction::Up,
        )];
        let mut static_runner = SceneRunner::new(compiled.ops, cast.clone(), StepFrames::default());
        let mut owned_runner = SceneRunner::from_vec(owned, cast, StepFrames::default());
        let mut static_state = GameState::new();
        let mut owned_state = GameState::new();

        for tick in 0..100 {
            let s_eff = static_runner.tick(&map, &mut static_state, SceneInput::None);
            let o_eff = owned_runner.tick(&map, &mut owned_state, SceneInput::None);
            assert_eq!(s_eff, o_eff, "effects diverged at tick {tick}");
            assert_eq!(static_state.snapshot(), owned_state.snapshot());
            assert_eq!(static_runner.is_finished(), owned_runner.is_finished());
            if static_runner.is_finished() {
                break;
            }
        }
        assert!(static_runner.is_finished());
        assert!(owned_state.is_set(Flag::temp(0)));
    }

    #[test]
    fn owned_drift_and_skip_ops_match_borrowed_execution() {
        static BORROWED: &[SceneOp] = &[
            SceneOp::SkipUnlessMap {
                maps: Cow::Borrowed(&[0x40]),
                skip: 1,
            },
            SceneOp::SetFlag {
                flag: Flag::event(0x10),
                value: true,
            },
            SceneOp::SetActorAnimation {
                actor: ActorRef::Npc(0),
                animation: ActorAnimation {
                    word: 0x10,
                    index: 0,
                    duration: 2,
                    durations: Cow::Borrowed(&[2, 2]),
                },
            },
            SceneOp::DriftNpcs {
                drifts: Cow::Borrowed(&[NpcDrift {
                    npc: 0,
                    step_x: 0x10000,
                    step_y: 0,
                }]),
                frames: 4,
            },
        ];
        let owned = vec![
            SceneOp::SkipUnlessMap {
                maps: Cow::Owned(vec![0x40]),
                skip: 1,
            },
            SceneOp::SetFlag {
                flag: Flag::event(0x10),
                value: true,
            },
            SceneOp::SetActorAnimation {
                actor: ActorRef::Npc(0),
                animation: ActorAnimation {
                    word: 0x10,
                    index: 0,
                    duration: 2,
                    durations: Cow::Owned(vec![2, 2]),
                },
            },
            SceneOp::DriftNpcs {
                drifts: Cow::Owned(vec![NpcDrift {
                    npc: 0,
                    step_x: 0x10000,
                    step_y: 0,
                }]),
                frames: 4,
            },
        ];
        validate_scene_ops(&owned).expect("owned drift scene validates");

        let npc = Npc {
            id: NpcId(1),
            cell: Cell::new(2, 2),
            offset: SubCellOffset::ALIGNED,
            facing: Direction::Down,
            active: true,
            camera_bypass: false,
            interactable: true,
            talkable: true,
        };
        let grid = CollisionGrid::new(8, 8, vec![0; 64]).unwrap();
        let map = FieldMap::new(MapId(0x40), grid, vec![], vec![npc]).unwrap();
        let cast = vec![ScriptedActor::new(
            ActorRef::Npc(0),
            Cell::new(2, 2),
            Direction::Down,
        )];
        let mut s_runner = SceneRunner::new(BORROWED, cast.clone(), StepFrames::default());
        let mut o_runner = SceneRunner::from_vec(owned, cast, StepFrames::default());
        let mut s_state = GameState::new();
        let mut o_state = GameState::new();
        for _ in 0..20 {
            assert_eq!(
                s_runner.tick(&map, &mut s_state, SceneInput::None),
                o_runner.tick(&map, &mut o_state, SceneInput::None)
            );
            if s_runner.is_finished() {
                break;
            }
        }
        assert!(o_runner.is_finished());
        assert_eq!(s_state.snapshot(), o_state.snapshot());
    }

    #[test]
    fn validator_rejects_malformed_programs_fail_closed() {
        assert_eq!(validate_scene_ops(&[]).unwrap_err().op_index, 0);

        let bad_jump = [SceneOp::Jump { to: 5 }];
        let err = validate_scene_ops(&bad_jump).unwrap_err();
        assert_eq!(err.op_index, 0);
        assert!(err.reason.contains("jump target 5"));

        let bad_skip = [SceneOp::End, SceneOp::SkipOps { count: 1 }];
        let err = validate_scene_ops(&bad_skip).unwrap_err();
        assert_eq!(err.op_index, 1);
        assert!(err.reason.contains("relative skip"));

        let bad_flag = [SceneOp::SetFlag {
            flag: Flag::town(128),
            value: true,
        }];
        assert_eq!(validate_scene_ops(&bad_flag).unwrap_err().op_index, 0);

        let bad_chunks = [SceneOp::WriteMapChunks {
            chunks: Cow::Owned(vec![]),
        }];
        assert_eq!(validate_scene_ops(&bad_chunks).unwrap_err().op_index, 0);

        let bad_slot = [SceneOp::JoinParty {
            slot: PARTY_SLOTS,
            who: CharId(0),
        }];
        assert_eq!(validate_scene_ops(&bad_slot).unwrap_err().op_index, 0);

        let bad_anim = [SceneOp::SetActorAnimation {
            actor: ActorRef::Npc(0),
            animation: ActorAnimation {
                word: 0,
                index: 2,
                duration: 1,
                durations: Cow::Owned(vec![1, 1]),
            },
        }];
        assert_eq!(validate_scene_ops(&bad_anim).unwrap_err().op_index, 0);

        let bad_camera = [SceneOp::PanCamera {
            subject: CameraSubject::At { x: 0, y: 0 },
            speed: 0,
        }];
        assert_eq!(validate_scene_ops(&bad_camera).unwrap_err().op_index, 0);
    }
}
