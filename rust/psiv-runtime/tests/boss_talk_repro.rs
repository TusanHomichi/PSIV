//! Live-QA regression: the map-$17 interaction area must win before the
//! invisible blocker probe and hand the scene-owned battle to the runtime.

use std::path::Path;

use psiv_core::{Cell, Direction, Input, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, RuntimeEvent};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

#[test]
fn the_igglanova_area_starts_the_scene_before_the_blocker() {
    if !Path::new(PACK).join("battle").is_dir() {
        eprintln!("pack absent; skipping");
        return;
    }
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let mut runtime = Runtime::new(
        data,
        0x017,
        Cell::new(15, 11),
        Direction::Up,
        StepFrames::default(),
    )
    .expect("runtime starts below Igglanova");
    let files = BattleFiles::load(Path::new(PACK)).expect("battle files load");
    runtime.enable_battles(&files).expect("battles enable");
    // This fixture starts after the automatic container conversation. Map
    // entry now checks that conversation before accepting a new action.
    runtime.set_event_flag(0x0D).unwrap();

    let first = runtime.tick(Input::Action);
    assert_eq!(
        first,
        vec![RuntimeEvent::SceneStartedFromInteraction {
            area: 0,
            event: 0x006B,
        }],
        "the area notification must precede the invisible-blocker probe"
    );

    let mut battle_started = false;
    for _ in 0..20 {
        for event in runtime.tick(Input::Neutral) {
            if matches!(event, RuntimeEvent::SceneBattleStarted { index: 0, .. }) {
                battle_started = true;
            }
            assert!(
                !matches!(
                    event,
                    RuntimeEvent::Interact { .. } | RuntimeEvent::InteractNothing { .. }
                ),
                "the area must consume confirm before the blocker answers: {event:?}"
            );
        }
        if battle_started {
            break;
        }
    }
    assert!(
        battle_started,
        "the scene-owned boss battle starts from the interaction area"
    );
}
