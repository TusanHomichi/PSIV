//! The scene interpreter: scripted cutscenes as data.
//!
//! The cartridge's ~162 `Event_*` routines are hand-written 68k, but composed
//! almost entirely from a small vocabulary — `Event_UpdateObjFacing`,
//! `Event_GetAndRunDialogue`, `Event_MoveCamera`, `Event_AddMacro`,
//! move-object-and-wait loops, `Current_Party_Slots` writes, flag sets, NPC
//! despawns. This module implements that vocabulary once as [`SceneOp`] and
//! runs sequences of it deterministically.
//!
//! How the runner blocks lives with the machine (`scene_runner.rs`), and why scripted actor
//! motion reuses the ordinary movement model with the actor (`scene_runner/actor.rs`).

mod ops;

pub use ops::SceneOp;

pub use crate::scene_types::{
    ActorRef, Axis, DialogueId, DialogueSource, DialogueWindow, FifthOp, GuildOp, ItemOp,
    JobBoardStep, MoneyTest, OP_BUDGET_PER_TICK, ObjectOp, RideOp, RockyOp, SavedByte, SceneEffect,
    SceneFault, SceneInput, WreckageOp,
};

pub use crate::scene_runner::actor::{ActorAnimation, ScriptedActor};
