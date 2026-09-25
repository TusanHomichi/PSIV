//! The fixtures the four specification modules share.
//!
//! One synthetic scene (`JOIN_SCENE`) and the three helpers around it: the step
//! rate every scene case runs at, a trigger context that is not standing on an
//! object, and a runner over that synthetic scene. They live here rather than in
//! one module because the scenes, the registry and the recast case all use them.
//!
//! Nothing here derives from the cartridge: this is the shape a scene takes,
//! which the registry cases then hold the shipped scenes to.

use psiv_core::{
    ActorRef, Cell, CharId, DialogueId, DialogueSource, Direction, Flag, GameState, PixelPos,
    SceneOp, SceneRunner, ScriptedActor, StepFrames, TriggerContext,
};

/// The step rate the scene cases run at: eight frames per cell.
const FRAMES: u8 = 8;

/// `FRAMES` as the validated type a runner takes.
pub(crate) fn frames() -> StepFrames {
    StepFrames::new(FRAMES).unwrap()
}

/// A trigger context for `state`, standing at `at`, on no object.
pub(crate) fn ctx<'a>(state: &'a GameState, at: PixelPos) -> TriggerContext<'a> {
    TriggerContext {
        state,
        at,
        standing: None,
        previously_standing: None,
    }
}

/// The opening-act shape: an NPC walks over, turns, talks, joins the party,
/// despawns, and the camera moves. Synthetic, but the same op sequence
/// `Event_AlysFound` is described as using.
static JOIN_SCENE: &[SceneOp] = &[
    SceneOp::MoveActor {
        actor: ActorRef::Npc(0),
        to: Cell::new(3, 1),
    },
    SceneOp::WaitForActor {
        actor: ActorRef::Npc(0),
    },
    SceneOp::Face {
        actor: ActorRef::Npc(0),
        facing: Direction::Right,
    },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(7)),
        window: psiv_core::DialogueWindow::Standard,
    },
    SceneOp::SetParty {
        slots: [Some(CharId(1)), Some(CharId(0)), None, None, None],
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x08),
        value: true,
    },
    SceneOp::DespawnNpc {
        npc_index: 0,
        count: 1,
    },
    SceneOp::MoveCamera {
        x: 0x30,
        y: 0x10,
        speed: 2,
    },
    SceneOp::End,
];

/// A runner over `JOIN_SCENE` with its one actor standing at `(1, 1)`.
pub(crate) fn join_runner() -> SceneRunner {
    SceneRunner::new(
        JOIN_SCENE,
        vec![ScriptedActor::new(
            ActorRef::Npc(0),
            Cell::new(1, 1),
            Direction::Down,
        )],
        frames(),
    )
}
