//! US `Cutscene_ZioDefeated` preserves Chaz: Demi's status clear at
//! `$075E88` leads straight into the Motavia load at `$075E8C` (#736).

mod common;

use psiv_core::{
    CharId, Direction, EventIndex, Flag, GameState, SceneEffect, SceneInput, SceneOp, SceneRunner,
    StepFrames, scene_for,
};

fn zio_defeated_preserves_chaz(hp: u16, status: u8) {
    let map = common::map_with(&["....", "...."], vec![], vec![]);
    let scene = scene_for(EventIndex(0x800B)).expect("ZioDefeated registered");
    let mut state = GameState::new();
    let party = [CharId(0), CharId(4), CharId(5), CharId(6), CharId(3)];
    for (slot, who) in party.into_iter().enumerate() {
        state.set_party_slot(slot, Some(who)).unwrap();
        let mut stats = common::stats();
        stats.status = 1;
        state.roster_mut().seat(who, stats).unwrap();
    }
    let chaz = state.roster_mut().get_mut(CharId(0)).unwrap();
    chaz.curr_hp = hp;
    chaz.status = status;
    let before = state.roster().clone();

    let mut runner = SceneRunner::new(scene.ops, vec![], StepFrames::default());
    let mut input = SceneInput::None;
    let mut log = Vec::new();
    for _ in 0..20_000 {
        if runner.awaits_palette() {
            runner.release_palette(1);
        }
        let effects = runner.tick(&map, &mut state, input);
        assert!(
            !effects.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
            "{effects:?}"
        );
        input = if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::MapRequested { .. }))
        {
            SceneInput::MapLoaded
        } else {
            SceneInput::DialogueClosed
        };
        log.extend(effects);
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished(), "ZioDefeated returns");
    assert!(log.contains(&SceneEffect::Returned { value: 1 }));
    assert_eq!(state.party_members(), [CharId(0), CharId(5), CharId(3)]);
    for who in party {
        let mut expected = before.get(who).unwrap().clone();
        if who == CharId(4) || who == CharId(6) {
            expected.status = 0;
        }
        assert_eq!(
            state.roster().get(who),
            Some(&expected),
            "only Gryz and Demi's statuses are cleared; character {who:?}"
        );
    }
    let loads: Vec<_> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::MapRequested { op } => Some(*op),
            _ => None,
        })
        .collect();
    assert_eq!(loads.len(), 2);
    assert_eq!(
        loads[1],
        SceneOp::LoadMap {
            map: 0x00,
            prev_map: 0xAC,
            start_x: 0x6C,
            start_y: 0xB8,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        }
    );
    for flag in [0x68, 0x66, 0x61] {
        assert!(state.is_set(Flag::event(flag)));
    }
}

#[test]
fn zio_defeated_leaves_dead_chaz_dead() {
    zio_defeated_preserves_chaz(0, 0x04);
}

#[test]
fn zio_defeated_leaves_living_chaz_statused_and_unhealed() {
    zio_defeated_preserves_chaz(7, 0x01);
}
