//! Lowering from `psiv-data`'s serialized [`DataSceneOp`] DTOs into
//! `psiv-core`'s [`SceneOp`] representation.

use std::borrow::Cow;

use psiv_core::{
    ActorAnimation, ActorRef, Axis, CameraSubject, Cell, CharId, CoordCmp, DialogueId,
    DialogueSource, DialogueWindow, Direction, Flag, MoneyTest, NpcDrift, SceneOp, SystemLine,
};
use psiv_data::{
    DataActorAnimation, DataActorRef, DataAxis, DataCameraSubject, DataCoordCmp,
    DataDialogueSource, DataDialogueWindow, DataDirection, DataFlag, DataFlagBank, DataMoneyTest,
    DataSceneOp,
};

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

pub(super) fn convert_op(op: &DataSceneOp) -> SceneOp {
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
