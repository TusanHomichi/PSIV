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
