//! Scene-level outcomes and completion contracts.

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, CharId, Direction, Flag, GameState, SceneEffect, SceneInput, SceneOp, SceneRunner,
    ScriptedActor, StepFrames,
};

const FRAMES: u8 = 8;

fn frames() -> StepFrames {
    StepFrames::new(FRAMES).unwrap()
}

// ---------------------------------------------------------------------------
// Small scene semantics
// ---------------------------------------------------------------------------

#[test]
fn face_is_a_direct_write_not_a_step() {
    // Scenes are not bound by the walker's commit-a-whole-cell rule.
    let map = map_with(&["....", "....", "...."], vec![], vec![]);
    let mut state = GameState::new();
    static TURN: &[SceneOp] = &[
        SceneOp::Face {
            actor: ActorRef::Npc(0),
            facing: Direction::Right,
        },
        SceneOp::End,
    ];
    let mut runner = SceneRunner::new(
        TURN,
        vec![ScriptedActor::new(
            ActorRef::Npc(0),
            psiv_core::Cell::new(1, 1),
            Direction::Down,
        )],
        frames(),
    );

    runner.tick(&map, &mut state, SceneInput::None);
    let actor = runner.actor(ActorRef::Npc(0)).unwrap();
    assert_eq!(actor.facing, Direction::Right);
    assert_eq!(
        actor.cell,
        psiv_core::Cell::new(1, 1),
        "facing must not move anyone"
    );
    assert!(!actor.is_walking());
}

#[test]
fn the_transcribed_party_words_match_the_oracles_observations() {
    // Observed on tape: 00FFFFFF (Chaz alone), 0001FFFF (Chaz then Alys),
    // 0100FFFF (Alys leading, post-AlysFound).
    let mut state = GameState::new();

    state.set_party([Some(CharId(0)), None, None, None, None]);
    assert_eq!(state.party_slot(0), Some(CharId(0)));
    assert_eq!(state.party_len(), 1);

    state.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
    assert_eq!(state.party_len(), 2);

    state.set_party([Some(CharId(1)), Some(CharId(0)), None, None, None]);
    assert_eq!(state.party_slot(0), Some(CharId(1)), "Alys leads");
    assert_eq!(state.party_slot(1), Some(CharId(0)));
}

// ---------------------------------------------------------------------------
// Per-scene outcomes — the load-bearing effects each doc identifies
// ---------------------------------------------------------------------------

/// Runs a scene to completion, answering every block the moment it appears.
fn run_scene(name: &str, state: &mut GameState, cast: Vec<ScriptedActor>) -> Vec<SceneEffect> {
    let map = map_with(
        &[
            "................",
            "................",
            "................",
            "................",
            "................",
            "................",
        ],
        vec![],
        vec![],
    );
    let scene = psiv_core::SCENES
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("{name} is not registered"));
    let mut runner = SceneRunner::new(scene.ops, cast, frames());
    let mut log: Vec<SceneEffect> = Vec::new();

    for _ in 0..20_000 {
        // Unblock whatever the previous tick asked for.
        let input = match log.last() {
            Some(SceneEffect::DialogueOpen(_))
            | Some(SceneEffect::DialogueOpenFromNpc { .. })
            | Some(SceneEffect::DialogueResume) => SceneInput::DialogueClosed,
            Some(SceneEffect::BattleRequested { .. }) => SceneInput::BattleFinished {
                outcome: psiv_core::battle::Outcome::Victory,
            },
            Some(SceneEffect::ChoiceRequested) => SceneInput::Choice(true),
            Some(SceneEffect::MapRequested { .. }) => SceneInput::MapLoaded,
            _ => SceneInput::None,
        };
        log.extend(runner.tick(&map, state, input));
        if runner.is_finished() {
            break;
        }
    }
    assert!(
        runner.is_finished(),
        "{name} did not finish; last effects {:?}",
        &log[log.len().saturating_sub(3)..]
    );
    assert!(
        !log.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
        "{name} faulted: {:?}",
        log.iter().find(|e| matches!(e, SceneEffect::Faulted(_)))
    );
    log
}

fn party_cast() -> Vec<ScriptedActor> {
    vec![
        ScriptedActor::new(
            ActorRef::PartyMember(0),
            psiv_core::Cell::new(2, 2),
            Direction::Down,
        ),
        ScriptedActor::new(
            ActorRef::PartyMember(1),
            psiv_core::Cell::new(2, 3),
            Direction::Down,
        ),
        ScriptedActor::new(
            ActorRef::Character(CharId(0)),
            psiv_core::Cell::new(2, 2),
            Direction::Down,
        ),
        ScriptedActor::new(
            ActorRef::Character(CharId(1)),
            psiv_core::Cell::new(2, 3),
            Direction::Down,
        ),
        ScriptedActor::new(
            ActorRef::Character(CharId(2)),
            psiv_core::Cell::new(3, 2),
            Direction::Down,
        ),
        ScriptedActor::new(
            ActorRef::Npc(0),
            psiv_core::Cell::new(4, 2),
            Direction::Down,
        ),
        ScriptedActor::new(
            ActorRef::Npc(7),
            psiv_core::Cell::new(2, 3),
            Direction::Down,
        ),
    ]
}

#[test]
fn piata_chaz_alone_sets_the_control_flag() {
    let mut state = GameState::new();
    run_scene("Event_PiataChazAlone", &mut state, party_cast());
    assert!(
        state.is_set(Flag::event(0x15)),
        "EventFlag_PiataChazControl"
    );
}

#[test]
fn suspicion_on_principal_sets_its_flag() {
    let mut state = GameState::new();
    run_scene("Event_SuspicionOnPrincipal", &mut state, party_cast());
    assert!(
        state.is_set(Flag::event(0x0E)),
        "EventFlag_PrincipalSuspicious"
    );
}

#[test]
fn the_principal_cutscene_sets_its_flag_and_returns_zero_to_reload_the_map() {
    let mut state = GameState::new();
    let log = run_scene("Cutscene_PiataPrincipal", &mut state, party_cast());
    assert!(
        state.is_set(Flag::event(0x09)),
        "EventFlag_PrincipalMeeting"
    );
    assert!(
        log.contains(&SceneEffect::Returned { value: 0 }),
        "a zero return is what makes the caller reload the office"
    );
}

#[test]
fn meeting_hahn_joins_slot_three_pays_a_hundred_and_clears_three_objects() {
    let mut state = GameState::new();
    state.set_party([Some(CharId(1)), Some(CharId(0)), None, None, None]);
    let log = run_scene("Event_MeetingHahn", &mut state, party_cast());

    assert_eq!(state.party_slot(2), Some(CharId(2)), "Hahn in slot 3");
    assert_eq!(state.party_len(), 3);
    assert_eq!(state.money(), 100);
    assert!(state.is_set(Flag::event(0x0A)), "EventFlag_HahnJoined");
    assert!(
        log.contains(&SceneEffect::NpcDespawned {
            npc_index: 0,
            count: 3
        }),
        "the block clear covers Hahn and both InvisibleBlocks"
    );
}

#[test]
fn basement_containers_swaps_the_music_around_the_line() {
    let mut state = GameState::new();
    let log = run_scene("Event_BasementContainers", &mut state, party_cast());
    assert!(
        state.is_set(Flag::event(0x0D)),
        "EventFlag_BasementContainers"
    );

    let sounds: Vec<u8> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::Presentation {
                op: SceneOp::PlaySound { id },
            } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(sounds, vec![0xAB, 0x8A], "Mystery, then back to InTheCave");
}

#[test]
fn the_igglanova_scene_sets_its_flag_before_the_fight_and_guards_re_entry() {
    let mut state = GameState::new();
    let log = run_scene("Event_IgglanovaBattle", &mut state, party_cast());
    assert!(state.is_set(Flag::event(0x0B)), "set on the way in");
    assert!(log.contains(&SceneEffect::BattleRequested { index: 0 }));
    assert!(log.contains(&SceneEffect::Returned { value: 1 }));

    // Second entry: the guard returns immediately, no battle.
    let log = run_scene("Event_IgglanovaBattle", &mut state, party_cast());
    assert!(
        !log.iter()
            .any(|e| matches!(e, SceneEffect::BattleRequested { .. })),
        "losing or fleeing must not let the fight re-trigger"
    );
    assert!(log.contains(&SceneEffect::Returned { value: 1 }));
}

#[test]
fn after_igglanova_pauses_the_dialogue_twice_and_sets_its_flag() {
    let mut state = GameState::new();
    let log = run_scene("Event_AfterIgglanova", &mut state, party_cast());
    assert!(state.is_set(Flag::event(0x0F)), "EventFlag_AfterIgglanova");
    assert_eq!(
        log.iter()
            .filter(|e| **e == SceneEffect::DialogueResume)
            .count(),
        2,
        "the conversation pauses twice while the actors restage"
    );
}

#[test]
fn the_confession_pays_three_hundred_and_opens_the_gates() {
    let mut state = GameState::new();
    run_scene("Event_PrincipalConfession", &mut state, party_cast());
    assert_eq!(state.money(), 300);
    assert!(
        state.is_set(Flag::event(0x0C)),
        "EventFlag_PrincipalConfession is what RunEvent_ReenterPiata tests"
    );
}

#[test]
fn the_principal_turns_to_face_whichever_way_the_leader_came_from() {
    let map = map_with(&["......", "......", "......"], vec![], vec![]);
    let scene = psiv_core::SCENES
        .iter()
        .find(|s| s.name == "Event_PrincipalConfession")
        .unwrap();

    for (leader_facing, expected) in [
        (Direction::Down, Direction::Up),
        (Direction::Up, Direction::Down),
        (Direction::Right, Direction::Left),
        (Direction::Left, Direction::Right),
    ] {
        let mut state = GameState::new();
        let mut runner = SceneRunner::new(
            scene.ops,
            vec![
                ScriptedActor::new(
                    ActorRef::PartyMember(0),
                    psiv_core::Cell::new(2, 2),
                    leader_facing,
                ),
                ScriptedActor::new(
                    ActorRef::Npc(0),
                    psiv_core::Cell::new(2, 1),
                    Direction::Down,
                ),
            ],
            frames(),
        );
        for _ in 0..40 {
            let input = if runner.actor(ActorRef::Npc(0)).is_some() {
                SceneInput::DialogueClosed
            } else {
                SceneInput::None
            };
            runner.tick(&map, &mut state, input);
            if runner.is_finished() {
                break;
            }
        }
        assert_eq!(
            runner.actor(ActorRef::Npc(0)).unwrap().facing,
            expected,
            "leader facing {leader_facing:?} should turn the principal {expected:?}"
        );
    }
}

#[test]
fn the_guards_reprimand_warps_into_piata_and_restores_the_wrong_tree() {
    let mut state = GameState::new();
    let log = run_scene("Event_PiataGuardsReprimand", &mut state, party_cast());

    // It sets no flag — it is pure gating.
    assert!(state.is_clear(Flag::event(0x0C)));

    let maps: Vec<u16> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::MapRequested {
                op: SceneOp::LoadMap { map, .. },
            } => Some(*map),
            _ => None,
        })
        .collect();
    assert_eq!(maps, vec![0x10], "warps into Piata");

    // Retail restores tree 1 while standing on map $10, which binds tree 2.
    // Reproduced, not fixed — see the scene's doc comment.
    let trees: Vec<u32> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::Presentation {
                op: SceneOp::SetDialogueTree { rom_addr },
            } => Some(*rom_addr),
            _ => None,
        })
        .collect();
    assert_eq!(trees, vec![0x001F_D7A0, 0x001D_F600]);
}

#[test]
fn game_start_tours_five_maps_and_hands_over_with_chaz_alone() {
    let mut state = GameState::new();
    let log = run_scene("Event_GameStart", &mut state, party_cast());

    let maps: Vec<u16> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::MapRequested {
                op: SceneOp::LoadMap { map, .. },
            } => Some(*map),
            _ => None,
        })
        .collect();
    assert_eq!(
        maps,
        vec![0x5E, 0x54, 0x00, 0x13],
        "ChazHouse, Aiedo, Motavia, PiataAcademy_F1"
    );

    assert!(state.is_set(Flag::event(0x07)), "EventFlag_PiataFirstTime");
    assert_eq!(
        state.party_slot(0),
        Some(CharId(0)),
        "Chaz alone at handover"
    );
    assert_eq!(state.party_len(), 1);
}

#[test]
fn game_start_puts_alys_in_front_for_the_walk_out() {
    // Mid-intro the party word is $0100FFFF — Alys leading — and only at
    // handover does it become Chaz alone.
    let map = map_with(&["........", "........", "........"], vec![], vec![]);
    let scene = psiv_core::SCENES
        .iter()
        .find(|s| s.name == "Event_GameStart")
        .unwrap();
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(scene.ops, party_cast(), frames());

    let mut saw_alys_leading = false;
    let mut input = SceneInput::DialogueClosed;
    for _ in 0..20_000 {
        let effects = runner.tick(&map, &mut state, input);
        if state.party_slot(0) == Some(CharId(1)) {
            saw_alys_leading = true;
        }
        input = if effects
            .iter()
            .any(|effect| matches!(effect, SceneEffect::MapRequested { .. }))
        {
            SceneInput::MapLoaded
        } else {
            SceneInput::DialogueClosed
        };
        if runner.is_finished() {
            break;
        }
    }
    assert!(saw_alys_leading, "Alys leads through the intro");
    assert_eq!(
        state.party_slot(0),
        Some(CharId(0)),
        "Chaz alone at the end"
    );
}
