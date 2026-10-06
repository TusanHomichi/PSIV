//! What a scene's return does: a cutscene's zero reloads the field, an event's
//! never does (`rust/psiv-runtime/src/scene_return.rs`).
use super::*;
use psiv_core::{EventIndex, Input, MapId, PresentationOp, Scene, SceneOp};
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn pack() -> Option<GameData> {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return None;
    }
    Some(GameData::load(pack).unwrap())
}

/// A walkable cell on `map`, found the way a fixture would place the party.
fn spawn_on(data: &GameData, map: u16) -> Cell {
    let record = data.map(psiv_data::MapId(map)).expect("map in pack");
    let field = crate::bridge::field_map(record).expect("map converts");
    (0..field.height())
        .flat_map(|y| (0..field.width()).map(move |x| Cell::new(x, y)))
        .find(|&cell| field.is_walkable(cell))
        .expect("a walkable cell")
}

fn runtime_on(data: GameData, map: u16) -> Runtime {
    let cell = spawn_on(&data, map);
    Runtime::new(data, map, cell, Direction::Up, StepFrames::default()).unwrap()
}

/// `Cutscene_MeetingKyra`'s tail: mount, `bset #0, Map_Load_Flags`, return `d0`.
const fn mount_and_return(value: u16) -> [SceneOp; 3] {
    [
        SceneOp::SetVehicleIndex { index: 2 },
        SceneOp::SetMapLoadFlags { set: 1, clear: 0 },
        SceneOp::Return { value },
    ]
}

static MOUNT_CUTSCENE: Scene = Scene {
    name: "test cutscene: mount, bset #0, return 0",
    event: EventIndex(0x8FF0),
    ops: &mount_and_return(0),
};
static MOUNT_EVENT: Scene = Scene {
    name: "test event: mount, bset #0, return 0",
    event: EventIndex(0x0FF0),
    ops: &mount_and_return(0),
};
static MOUNT_CUTSCENE_NONZERO: Scene = Scene {
    name: "test cutscene: mount, bset #0, return 1",
    event: EventIndex(0x8FF1),
    ops: &mount_and_return(1),
};
static BARE_CUTSCENE: Scene = Scene {
    name: "test cutscene: return 0",
    event: EventIndex(0x8FF2),
    ops: &[SceneOp::Return { value: 0 }],
};
static SKIP_FADE_CUTSCENE: Scene = Scene {
    name: "test cutscene: bset #7, return 0",
    event: EventIndex(0x8FF3),
    ops: &[
        SceneOp::SetMapLoadFlags {
            set: 0x80,
            clear: 0,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// Everything a scene run produced, in tick order.
struct Run {
    /// `(tick, event)`.
    events: Vec<(usize, RuntimeEvent)>,
    /// The tick that ended the scene.
    ended: usize,
}

impl Run {
    fn reloads(&self) -> Vec<(usize, u16, u16)> {
        self.events
            .iter()
            .filter_map(|(tick, event)| match event {
                RuntimeEvent::ScenePresentation {
                    op:
                        SceneOp::Presentation {
                            op: PresentationOp::FieldReload { setup, fade },
                        },
                } => Some((*tick, *setup, *fade)),
                _ => None,
            })
            .collect()
    }
}

fn run(rt: &mut Runtime, scene: &'static Scene) -> Run {
    assert!(rt.install_runner(scene, scene.event));
    let mut events = Vec::new();
    for tick in 0..2_000 {
        let emitted = rt.tick(Input::Neutral);
        assert!(
            !emitted.iter().any(|event| matches!(
                event,
                RuntimeEvent::SceneFaulted { .. } | RuntimeEvent::MapRefreshFailed { .. }
            )),
            "{emitted:?}"
        );
        let ended = emitted.contains(&RuntimeEvent::SceneEnded);
        events.extend(emitted.into_iter().map(|event| (tick, event)));
        if ended {
            return Run {
                events,
                ended: tick,
            };
        }
    }
    panic!("{} never ended", scene.name);
}

/// The class that halted the campaign route in `esper-mansion`: a cutscene
/// that mounts and sets bit 0 must hand those bits to a load that spends them.
#[test]
fn a_cutscene_that_mounts_and_sets_bit_zero_keeps_the_machine_until_the_next_warp() {
    let Some(data) = pack() else {
        return;
    };
    let mut rt = runtime_on(data, 0x14);
    let done = run(&mut rt, &MOUNT_CUTSCENE);
    assert_eq!(done.reloads().len(), 1, "one reload: {:?}", done.events);
    assert_eq!(rt.vehicle_index(), Some(2), "the reload spares the machine");
    assert_eq!(rt.map_load_flags, 0, "and spends bit 0");
    // The next warp reads a clean byte and parks the machine.
    rt.change_map_from(MapId(0x13), spawn_on(rt.data(), 0x13), Direction::Up, 0x14)
        .unwrap();
    assert_eq!(rt.vehicle_index(), None, "the warp parks it");
}

/// The negative control for the test above: with no reload (an event, whose
/// return `loc_5A27A` never reads) bit 0 lingers, and the warp spares the
/// vehicle - exactly the "vehicle is 2, expected 0" halt.
#[test]
fn without_the_reload_bit_zero_lingers_and_the_warp_wrongly_spares_the_machine() {
    let Some(data) = pack() else {
        return;
    };
    let mut rt = runtime_on(data, 0x14);
    let done = run(&mut rt, &MOUNT_EVENT);
    assert!(done.reloads().is_empty());
    assert_eq!(rt.vehicle_index(), Some(2));
    assert_eq!(rt.map_load_flags, 1, "nothing spent bit 0");
    rt.change_map_from(MapId(0x13), spawn_on(rt.data(), 0x13), Direction::Up, 0x14)
        .unwrap();
    assert_eq!(
        rt.vehicle_index(),
        Some(2),
        "the lingering bit carries the machine through the warp"
    );
}

/// `loc_5A27A` reads no `d0`: an event does no reload on any value, and a
/// cutscene's non-zero return skips it (`bne.s .skipmapreload`).
#[test]
fn events_and_non_zero_cutscenes_do_no_reload() {
    let Some(data) = pack() else {
        return;
    };
    for scene in [&MOUNT_EVENT, &MOUNT_CUTSCENE_NONZERO] {
        let mut rt = runtime_on(data.clone(), 0x14);
        let done = run(&mut rt, scene);
        assert!(done.reloads().is_empty(), "{}: no reload", scene.name);
        assert!(
            done.events
                .iter()
                .all(|(_, e)| !matches!(e, RuntimeEvent::MapRefreshed)),
            "{}: no map refresh",
            scene.name
        );
        assert_eq!(rt.map_load_flags, 1, "{}: nothing consumed", scene.name);
    }
}

/// The principal's own end, measured organically (tape 07): the scene clears
/// the saved music word, so the load's music branch writes it and spends one
/// more `VInt_Prepare`: 54 table rows + 1 = 55 frames to control.
#[test]
fn the_principals_return_takes_the_organic_55_frames_and_one_reload_op() {
    let Some(data) = pack() else {
        return;
    };
    let mut rt = runtime_on(data, 0x14);
    rt.saved_sound_index = 0;
    let done = run(&mut rt, &BARE_CUTSCENE);
    let reloads = done.reloads();
    assert_eq!(reloads.len(), 1, "the screen blacks out once");
    let (at, setup, fade) = reloads[0];
    assert_eq!((setup, fade), (39, 16), "55 = 39 black + the 16-frame fade");
    assert_eq!(done.ended - at, 55, "control returns 55 frames on");
    assert_ne!(rt.saved_sound_index, 0, "the music branch wrote the word");

    // Same load with the word already equal to the map's: no extra frame.
    let mut rt = runtime_on(pack().unwrap(), 0x14);
    let done = run(&mut rt, &BARE_CUTSCENE);
    let (at, setup, _) = done.reloads()[0];
    assert_eq!((setup, done.ended - at), (38, 54));

    // `Cutscene_MeetingKyra`'s bit 0 keeps the branch from running at all.
    let mut rt = runtime_on(pack().unwrap(), 0x14);
    rt.saved_sound_index = 0;
    let done = run(&mut rt, &MOUNT_CUTSCENE);
    let (at, setup, _) = done.reloads()[0];
    assert_eq!((setup, done.ended - at), (38, 54), "bit 0 skips the music");
    assert_eq!(rt.saved_sound_index, 0);
}

/// `bset #7` (skip palette fade in) spends 16 fewer frames and asks the
/// renderer for no fade (`ps4.asm:107628`; measured 54 -> 38 at map `$14`).
#[test]
fn bit_seven_skips_the_fade_and_its_sixteen_frames() {
    let Some(data) = pack() else {
        return;
    };
    let mut rt = runtime_on(data, 0x14);
    let done = run(&mut rt, &SKIP_FADE_CUTSCENE);
    let (at, setup, fade) = done.reloads()[0];
    assert_eq!((setup, fade), (38, 0));
    assert_eq!(done.ended - at, 38);
    assert_eq!(rt.map_load_flags, 0, "bit 7 is spent with the load");
}

/// The retained objects keep what a scene wrote into them: a dialogue id set
/// before a reload survives the map-data walk.
#[test]
fn a_reload_keeps_the_dialogue_ids_the_scene_wrote() {
    let Some(data) = pack() else {
        return;
    };
    let mut rt = runtime_on(data, 0x14);
    assert!(!rt.map.npcs().is_empty());
    rt.effects.dialogue_overrides.insert(0, 0x1234);
    rt.refresh_field_after_battle().unwrap();
    assert_eq!(rt.effects.dialogue_overrides.get(&0), Some(&0x1234));
}
