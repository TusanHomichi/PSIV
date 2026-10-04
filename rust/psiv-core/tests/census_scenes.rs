//! Scene outcomes for the events the census moved out of the allowlist
//! (#56, #71): each is a dialogue, a flag and, for four, an event battle.
//! Every case cites the retail bytes (`docs/scenes/93_*` to `97_*`).

mod common;

use common::map_with;
use psiv_core::{
    DialogueId, DialogueWindow, EventIndex, Flag, GameState, SceneEffect, SceneInput, SceneOp,
    SceneRunner, StepFrames, scene_for,
};

struct Played {
    effects: Vec<SceneEffect>,
    windows: Vec<DialogueWindow>,
}

/// Runs `event` to its end, closing every dialogue at once and ending any
/// battle in victory; records each dialogue's window routine.
fn play(event: u16, state: &mut GameState) -> Played {
    let map = map_with(&["....", "....", "...."], vec![], vec![]);
    let scene = scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} unregistered"));
    let mut runner = SceneRunner::new(scene.ops, vec![], StepFrames::new(8).unwrap());
    let mut played = Played {
        effects: Vec::new(),
        windows: Vec::new(),
    };
    for _ in 0..200 {
        let input = match played.effects.last() {
            Some(SceneEffect::DialogueOpen(_)) => SceneInput::DialogueClosed,
            Some(SceneEffect::BattleRequested { .. }) => SceneInput::BattleFinished {
                outcome: psiv_core::battle::Outcome::Victory,
            },
            _ => SceneInput::None,
        };
        let effects = runner.tick(&map, state, input);
        if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::DialogueOpen(_)))
        {
            played.windows.push(runner.dialogue_window());
        }
        played.effects.extend(effects);
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished(), "${event:04X} did not finish");
    assert!(
        !played
            .effects
            .iter()
            .any(|e| matches!(e, SceneEffect::Faulted(_))),
        "{:?}",
        played.effects
    );
    played
}

fn dialogues(played: &Played) -> Vec<u16> {
    played
        .effects
        .iter()
        .filter_map(|e| match e {
            SceneEffect::DialogueOpen(DialogueId(id)) => Some(*id),
            _ => None,
        })
        .collect()
}

fn battles(played: &Played) -> Vec<u16> {
    played
        .effects
        .iter()
        .filter_map(|e| match e {
            SceneEffect::BattleRequested { index } => Some(u16::from(*index)),
            _ => None,
        })
        .collect()
}

fn returned(played: &Played) -> Option<u16> {
    played.effects.iter().find_map(|e| match e {
        SceneEffect::Returned { value } => Some(*value),
        _ => None,
    })
}

fn sets_event_battle_load_flag(played: &Played) -> bool {
    played.effects.iter().any(|e| {
        matches!(
            e,
            SceneEffect::Presentation {
                op: SceneOp::SetMapLoadFlags {
                    set: 0x08,
                    clear: 0
                }
            }
        )
    })
}

/// `$06DEAE`: dialogue `$0C` in the standard window, then the tail
/// `jmp EventFlags_Set` with `$73`. No battle, no return value.
#[test]
fn the_canceller_reminder_talks_and_sets_73() {
    let mut state = GameState::new();
    let played = play(0x2A, &mut state);
    assert_eq!(dialogues(&played), vec![0x0C]);
    assert_eq!(played.windows, vec![DialogueWindow::Standard]);
    assert!(state.is_set(Flag::event(0x73)));
    assert!(battles(&played).is_empty());
    assert_eq!(returned(&played), None);
}

/// `$072654`: `$1B` set, `Saved_Sound_Index = $9E`, event battle 2, return 1.
#[test]
fn the_sandworm_sets_its_flag_and_fights_battle_2() {
    let mut state = GameState::new();
    assert!(!state.is_set(Flag::event(0x1B)));
    let played = play(0x71, &mut state);
    assert!(state.is_set(Flag::event(0x1B)), "once per save");
    assert!(dialogues(&played).is_empty(), "the worm speaks no line");
    assert_eq!(battles(&played), vec![2]);
    assert!(sets_event_battle_load_flag(&played));
    assert!(played.effects.iter().any(|e| matches!(
        e,
        SceneEffect::Presentation {
            op: SceneOp::SetSavedMusic { id: 0x9E }
        }
    )));
    assert_eq!(returned(&played), Some(1));
}

/// `$072B2C`: `Event_GetAndRunDialogue2` entry `$2F` (the window stays up),
/// flag `$2F`, battle 7, return 1.
#[test]
fn fract_ooze_found_retains_its_window_then_fights_battle_7() {
    let mut state = GameState::new();
    let played = play(0x7D, &mut state);
    assert_eq!(dialogues(&played), vec![0x2F]);
    assert_eq!(played.windows, vec![DialogueWindow::Retained]);
    assert!(state.is_set(Flag::event(0x2F)));
    assert_eq!(battles(&played), vec![7]);
    assert!(sets_event_battle_load_flag(&played));
    assert_eq!(returned(&played), Some(1));
}

/// `$072FDC`: with `$AE` set, retained entry `$36`, flag `$AF`, battle `$13`.
#[test]
fn king_rappy_fights_once_ae_is_set() {
    let mut state = GameState::new();
    state.write(Flag::event(0xAE), true).unwrap();
    let played = play(0x88, &mut state);
    assert_eq!(dialogues(&played), vec![0x36]);
    assert_eq!(played.windows, vec![DialogueWindow::Retained]);
    assert!(state.is_set(Flag::event(0xAF)));
    assert_eq!(battles(&played), vec![0x13]);
    assert!(sets_event_battle_load_flag(&played));
    assert_eq!(returned(&played), Some(1));
}

/// `$073010`: with `$AE` clear, only dialogue `$35` in the standard window.
#[test]
fn king_rappy_only_talks_while_ae_is_clear() {
    let mut state = GameState::new();
    let played = play(0x88, &mut state);
    assert_eq!(dialogues(&played), vec![0x35]);
    assert_eq!(played.windows, vec![DialogueWindow::Standard]);
    assert!(!state.is_set(Flag::event(0xAF)));
    assert!(battles(&played).is_empty());
    assert_eq!(returned(&played), None);
}

/// `$0731B2`: retained entry 4, flag `$B6`, battle `$15`, return 1.
#[test]
fn the_daughter_terminal_retains_its_window_then_fights_battle_15() {
    let mut state = GameState::new();
    let played = play(0x8F, &mut state);
    assert_eq!(dialogues(&played), vec![0x04]);
    assert_eq!(played.windows, vec![DialogueWindow::Retained]);
    assert!(state.is_set(Flag::event(0xB6)));
    assert_eq!(battles(&played), vec![0x15]);
    assert!(sets_event_battle_load_flag(&played));
    assert_eq!(returned(&played), Some(1));
}
