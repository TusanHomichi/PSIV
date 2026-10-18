//! The Soldier's Temple island: the cave's two conversations, the exit, and the
//! Aero-Prism chest's reaction (issue #81, [112](../../../../docs/scenes/112_SoldiersTemple.md)).
//!
//! Each body is `moveq #n, d0` / `jsr Event_GetAndRunDialogue` and one or two
//! flag sets (`ps4.asm:153412-153433`). Read from the US image through the
//! `EventPtrs` slots `$9A..$9D` (`$05A2B4 + 4 * n`); every instruction word was
//! compared with the clone's label bodies.

use crate::scene::{DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const fn standard(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Standard,
    }
}

const fn set_flag(id: u16) -> SceneOp {
    SceneOp::SetFlag {
        flag: Flag::event(id),
        value: true,
    }
}

/// `$009A`, `Event_SoldiersTempleCaveDialogue1`, retail `$073880..$073891`:
/// dialogue `$0E`, then `EventFlag_SethConversation1` (`$C2`). Fires on the
/// load of `IslandCave_F1` (`RunEvent_SoldiersTempleCaveDialogue1`, `$57`).
pub static CAVE_DIALOGUE_1: Scene = Scene {
    name: "Event_SoldiersTempleCaveDialogue1",
    event: EventIndex(0x009A),
    ops: &[standard(0x0E), set_flag(0xC2)],
};

/// `$009B`, `Event_SoldiersTempleCaveDialogue2`, retail `$073892..$0738A3`:
/// dialogue `$0F`, then `EventFlag_SethConversation2` (`$C3`). Fires on the
/// load of `IslandCave_F3` (`$58`).
pub static CAVE_DIALOGUE_2: Scene = Scene {
    name: "Event_SoldiersTempleCaveDialogue2",
    event: EventIndex(0x009B),
    ops: &[standard(0x0F), set_flag(0xC3)],
};

/// `$009C`, `Event_SoldiersTempleReached`, retail `$0738A4..$0738B5`: dialogue
/// `$10`, then `EventFlag_SoldiersTemple` (`$C4`). Fires on the load of
/// `SoldiersTempleOutside` (`$59`).
pub static TEMPLE_REACHED: Scene = Scene {
    name: "Event_SoldiersTempleReached",
    event: EventIndex(0x009C),
    ops: &[standard(0x10), set_flag(0xC4)],
};

/// `$009D`, `Event_AeroPrismFound`, retail `$0738B6..$0738D1`: dialogue `$11`,
/// then `EventFlag_AeroPrism2` (`$C9`) and `EventFlag_AeroPrism1` (`$C8`), in
/// that order. Fires inside `SoldiersTemple` once the Aero-Prism chest
/// (`$10D`) is open (`RunEvent_FindingAeroPrism`, `$5A`).
pub static AERO_PRISM_FOUND: Scene = Scene {
    name: "Event_AeroPrismFound",
    event: EventIndex(0x009D),
    ops: &[standard(0x11), set_flag(0xC9), set_flag(0xC8)],
};

#[cfg(test)]
mod tests {
    use crate::collision::CollisionGrid;
    use crate::map::{FieldMap, MapId};
    use crate::scene::{DialogueId, SceneEffect, SceneInput};
    use crate::scene_runner::SceneRunner;
    use crate::state::{Flag, GameState};
    use crate::trigger::EventIndex;

    /// Plays the scene registered for `event` to its end, closing each
    /// dialogue at once; returns the dialogues it opened and the flags it
    /// set, in order.
    fn play(event: u16) -> (Vec<u16>, Vec<Flag>) {
        let scene = super::super::scene_for(EventIndex(event))
            .unwrap_or_else(|| panic!("${event:04X} is not registered"));
        let map = FieldMap::new(
            MapId(0),
            CollisionGrid::filled(4, 4, 0).unwrap(),
            vec![],
            vec![],
        )
        .unwrap();
        let mut state = GameState::new();
        let mut runner = SceneRunner::new(scene.ops, vec![], crate::StepFrames::default());
        let mut dialogues = Vec::new();
        let mut flags = Vec::new();
        let mut input = SceneInput::None;
        for _ in 0..200 {
            let effects = runner.tick(&map, &mut state, input);
            input = SceneInput::None;
            for effect in effects {
                match effect {
                    SceneEffect::DialogueOpen(DialogueId(id)) => {
                        dialogues.push(id);
                        input = SceneInput::DialogueClosed;
                    }
                    SceneEffect::FlagChanged { flag, value: true } => flags.push(flag),
                    SceneEffect::Faulted(fault) => panic!("${event:04X} faulted: {fault:?}"),
                    _ => {}
                }
            }
            if runner.is_finished() {
                return (dialogues, flags);
            }
        }
        panic!("${event:04X} did not finish");
    }

    /// Each body as `ps4.asm:153412-153433` and the retail bytes
    /// `$073880..$0738D1` have it: one dialogue of the map's tree, then the
    /// flag (two for the Aero-Prism, `$C9` before `$C8`).
    #[test]
    fn each_scene_opens_its_dialogue_then_sets_its_flags() {
        let event = |id| vec![Flag::event(id)];
        assert_eq!(play(0x9A), (vec![0x0E], event(0xC2)));
        assert_eq!(play(0x9B), (vec![0x0F], event(0xC3)));
        assert_eq!(play(0x9C), (vec![0x10], event(0xC4)));
        assert_eq!(
            play(0x9D),
            (vec![0x11], vec![Flag::event(0xC9), Flag::event(0xC8)])
        );
    }
}
