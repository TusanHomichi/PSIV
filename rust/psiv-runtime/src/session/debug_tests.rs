//! The certification fixtures' own tests.
//!
//! The state each fixture builds is retail's, assembled from the initializer
//! with explicit edits; these cases guard the two ways that has gone wrong
//! before: a fixture whose banks are blank replays a map-entry trigger, and a
//! fixture that does not arm the field is not the state the receipt recorded.

use std::path::Path;

use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::GameData;

use crate::pad::Pad;
use crate::{Runtime, Session};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn pack_present() -> bool {
    Path::new(PACK).join("manifest.json").is_file()
}

fn pack() -> GameData {
    GameData::load(Path::new(PACK)).expect("pack loads")
}

/// The first frames the shell spends entering a scene after a landing: does
/// the state hand the story control on its own?
fn a_scene_takes_over(session: &mut Session) -> bool {
    (0..3).any(|_| {
        session.frame(Pad::NEUTRAL);
        session.runtime().scene_active()
    })
}

/// #44: the camp fixture rotted because PiataChazAlone's map-entry trigger
/// fired on a state with `$15` clear. The fixture's post-opening state must
/// load Academy F1 quietly; the blank state beside it is the negative control
/// showing the trigger really does pre-empt.
#[test]
fn the_camp_fixture_does_not_replay_piata_chaz_alone() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let data = pack();
    let blank = Runtime::from_save(
        data.clone(),
        RetailSave {
            snapshot: GameState::new().snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0xFFFF,
                map_index: 0x13,
                char_x: 0x2F0,
                char_y: 0x140,
            },
        },
        StepFrames::default(),
    )
    .expect("the blank runtime builds");
    assert!(
        a_scene_takes_over(&mut Session::new(blank)),
        "negative control: with $15 clear the map-entry trigger must fire"
    );

    let mut fixture = crate::camp_fixture(data, StepFrames::default()).expect("the fixture builds");
    assert!(!a_scene_takes_over(&mut fixture));
    let runtime = fixture.runtime();
    assert!(runtime.game().is_set(Flag::event(21)));
    assert!(runtime.game().is_set(Flag::event(7)));
    assert_eq!(
        runtime.game().party_members(),
        vec![CharId(0)],
        "tape 22's camp frame has Chaz alone"
    );
    assert_eq!(runtime.game().money(), 500);
    assert_eq!(runtime.map_id().0, 0x13);
}

/// A session on the new-game field with battles armed, the way the shell
/// builds one before a `PSIV_DEBUG_BATTLE_WINDOW` fixture.
fn armed_session() -> Option<Session> {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return None;
    }
    let files = psiv_data::BattleFiles::load(Path::new(PACK)).expect("battle files load");
    Some(
        Session::start(pack())
            .with_battles(files)
            .field()
            .expect("the pack boots"),
    )
}

fn battle_view(frame: &crate::BattleFrame) -> &crate::BattleView {
    frame.view.as_ref().expect("the fixture started a battle")
}

/// The strip and technique-window fixtures are the oracle receipts' state:
/// Alys opens first, every icon is there, her techniques read newest first
/// with the costs the receipt shows, and the technique window puts her pane
/// on the tech icon.
#[test]
fn the_window_fixtures_are_the_tape_07_state() {
    let Some(mut session) = armed_session() else {
        return;
    };
    let frame = session.debug_battle_window(0x8A, "strip,cursor=1");
    let view = battle_view(&frame);
    let Some(crate::MenuView::Commands(menu)) = view.menu.as_ref() else {
        panic!("the strip fixture opens a command window");
    };
    let strip = menu.strip.expect("a strip");
    assert_eq!(
        (strip.slot, strip.cursor, strip.present),
        (0, 1, [true; 5]),
        "Alys is slot 0 and has something under every icon"
    );
    assert_eq!(menu.actor, Some(1));
    assert!(menu.list.is_none());
    assert!(
        view.panes
            .iter()
            .all(|pane| pane.icon == 0 || pane.icon == 9)
    );

    let mut session = armed_session().unwrap();
    let frame = session.debug_battle_window(0x8A, "tech,cursor=0");
    let view = battle_view(&frame);
    let Some(crate::MenuView::Commands(menu)) = view.menu.as_ref() else {
        panic!("the technique fixture opens a command window");
    };
    let list = menu.list.as_ref().expect("a list window");
    let entries: Vec<_> = list
        .entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry.value, entry.enabled))
        .collect();
    assert_eq!(
        entries,
        [
            ("SANER", Some(6), true),
            ("SHIFT", Some(7), true),
            ("FOI", Some(3), true)
        ]
    );
    assert_eq!((list.page, list.cursor), (0, 0));
    let alys = view.panes.iter().find(|pane| pane.fighter == 1).unwrap();
    assert_eq!(
        alys.icon, 2,
        "opening the technique window writes command byte 2"
    );
}

/// The sleep, paralysis and seal fixture: statuses reach the strip's panes
/// the way the retail routine reads them, and the seal alone draws nothing.
#[test]
fn status_fixtures_reach_the_panes() {
    let Some(mut session) = armed_session() else {
        return;
    };
    let frame = session.debug_battle_window(0x8A, "top,status=2/8,status=1/2,status=3/16");
    let view = battle_view(&frame);
    let panes: Vec<_> = view
        .panes
        .iter()
        .map(|pane| (pane.fighter, pane.icon, pane.ink_line))
        .collect();
    // Fighter 1 Alys paralyzed, 2 Chaz asleep, 3 Hahn sealed, 4 and 5 empty.
    assert_eq!(
        panes,
        [(1, 6, 1), (2, 7, 1), (3, 0, 3), (4, 9, 3), (5, 9, 3)]
    );
}

/// The Zol slugs' Fusion through the whole shell path: a defended round runs
/// the slugs' turn, the view's slot 1 becomes the MetaSlug at position `$14`,
/// and the options come back over it with every pane back to no command.
#[test]
fn a_fusion_round_leaves_a_metaslug_in_slot_one() {
    let Some(mut session) = armed_session() else {
        return;
    };
    let frame = session.debug_battle_window(0xD2, "top,round=defend");
    assert!(battle_view(&frame).enemies.iter().all(|e| e.enemy_id == 34));
    let mut idle = None;
    for tick in 0..4_000 {
        let frame = session.frame(Pad::NEUTRAL);
        let Some(view) = frame
            .battle
            .as_ref()
            .and_then(|battle| battle.view.as_ref())
        else {
            continue;
        };
        if view.ready {
            idle = Some((tick, view.clone()));
            break;
        }
    }
    let (tick, view) = idle.expect("the options come back");
    eprintln!("fusion round: options back after {tick} frames");
    let slot_one = view.enemies.iter().find(|e| e.fighter == 6).unwrap();
    assert_eq!(
        (slot_one.enemy_id, slot_one.position, slot_one.visible),
        (36, Some(0x14), true)
    );
    assert!(view.enemies.iter().filter(|e| e.visible).count() == 1);
    assert_eq!(slot_one.name, "META SLUG");
    assert!(
        view.panes
            .iter()
            .all(|pane| pane.icon == 0 || pane.icon == 9)
    );
}

/// #52: an enemy attack animation cue rides the first frame of its beat, once.
/// The shell shows that frame's art at its start and first advances it on the
/// next frame (`ps4.asm:947-966`), so the cue frame is the start the layer's
/// sixteen frames are counted from. This pins the runtime half: the cue is in
/// the view of the beat's first frame (`remaining == total`) and nowhere else,
/// and the beat that follows the defenders' three comes after the cartridge's
/// defend beats, not before them.
#[test]
fn an_attack_animation_cue_rides_the_first_frame_of_its_beat() {
    let Some(mut session) = armed_session() else {
        return;
    };
    let frame = session.debug_battle_window(0x8A, "top,timeline=attack");
    let mut view = battle_view(&frame).clone();
    let mut cues = Vec::new();
    for tick in 0..400 {
        if !view.animations.is_empty() {
            let beat = view.current.expect("a beat is playing");
            assert_eq!(
                beat.remaining, beat.total,
                "the cue is on the beat's first frame"
            );
            assert!(matches!(beat.beat, crate::BattleBeat::Attack(_)));
            cues.push(tick);
        }
        let frame = session.frame(Pad::NEUTRAL);
        view = frame
            .battle
            .and_then(|battle| battle.view)
            .expect("the battle runs");
        if view.ready {
            break;
        }
    }
    // Three defend beats of 36 frames, then the attack.
    assert_eq!(cues, [108]);
}

/// Only the bodies that have acted are on the plane while a round plays, and
/// every body is back when the options reopen.
#[test]
fn party_bodies_return_as_their_owners_act() {
    let Some(mut session) = armed_session() else {
        return;
    };
    let frame = session.debug_battle_window(0x8A, "top,timeline=attack");
    let mut view = battle_view(&frame).clone();
    assert_eq!(view.shown, Some(vec![1]), "Alys defended first");
    for _ in 0..400 {
        let frame = session.frame(Pad::NEUTRAL);
        view = frame
            .battle
            .and_then(|battle| battle.view)
            .expect("the battle runs");
        if view.ready {
            break;
        }
    }
    assert!(view.ready);
    assert_eq!(view.shown, None, "the options draw all three bodies");
}
