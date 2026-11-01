//! `$94`'s two camera pans through the real runtime (the PR's review; record
//! in `docs/scenes/123_StripClub.md`). State tests from constructed saves: they
//! read the pack as the other scene-map tests do.
use super::*;
use psiv_core::{CharId, GameState, Input, PixelPos, RetailLocation, RetailSave, SceneOp};

const STRIP_CLUB: u16 = 0x05A;
const STRIPPERS_DANCE: Flag = Flag::temp(0x1B);
/// `($220,$1C0)` less the routine's `$98`/`$58`.
const STAGE_VIEW: (i32, i32) = (0x188, 0x168);
/// `y <= $240`: the runtime's cell row 37 stands at pixel `$240`
/// (`PixelPos::from_cell`).
const TRIGGER_ROW: u16 = 37;

fn party_below_the_stage(column: u16) -> Runtime {
    let data = GameData::load(&pack_dir()).unwrap();
    let mut game = GameState::new();
    game.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
    Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: STRIP_CLUB,
                char_x: column * 16,
                char_y: (TRIGGER_ROW + 1) * 16,
            },
        },
        StepFrames::default(),
    )
    .unwrap()
}

fn view(rt: &Runtime) -> (i32, i32) {
    let (x, y) = rt.camera().raw();
    (x >> 16, y >> 16)
}

fn home_of(rt: &Runtime) -> (i32, i32) {
    let at = PixelPos::from_cell(rt.scene_party_actor(0).unwrap().cell);
    (at.x - psiv_core::HOME_X, at.y - psiv_core::HOME_Y)
}

/// Walks up until `$94` starts; the frame that started it has run no op.
fn step_into_the_scene(rt: &mut Runtime) {
    for _ in 0..200 {
        checked_tick(rt, Input::Direction(Direction::Up));
        if rt.scene_active() {
            assert_eq!(rt.scene_event(), Some(EventIndex(0x94)));
            return;
        }
    }
    panic!("the trigger row did not fire");
}

/// One tick of the scene: the presentation ops it emitted.
fn scene_frame(rt: &mut Runtime) -> Vec<SceneOp> {
    checked_tick(rt, Input::Neutral)
        .into_iter()
        .filter_map(|event| match event {
            RuntimeEvent::ScenePresentation { op } => Some(op),
            _ => None,
        })
        .collect()
}

fn is_pan(op: &SceneOp) -> bool {
    matches!(op, SceneOp::PanCamera { .. })
}

fn created(op: &SceneOp, id: u16) -> bool {
    matches!(op, SceneOp::CreateFieldObject { object_id, .. } if *object_id == id)
}

/// One pan, from the frame its op ran: frames until the camera stood on
/// `target`, X before Y, one pixel a frame. Returns the ops of the last frame.
fn follow_pan(rt: &mut Runtime, target: (i32, i32), mut before: (i32, i32)) -> (u32, Vec<SceneOp>) {
    let mut frames = 0;
    loop {
        let ops = scene_frame(rt);
        frames += 1;
        let now = view(rt);
        assert!(rt.scene_active(), "the pan holds the pad");
        assert!(!rt.game().is_set(STRIPPERS_DANCE), "the flag is last");
        if before.0 != target.0 {
            assert_eq!(now.1, before.1, "Y waits for X");
        }
        assert_eq!(
            (now.0 - before.0).abs() + (now.1 - before.1).abs(),
            i32::from(now != before),
            "a pixel a frame"
        );
        before = now;
        if now == target {
            return (frames, ops);
        }
        assert!(
            ops.is_empty(),
            "nothing runs during the pan, frame {frames}: {ops:?}"
        );
        assert!(frames < 2_000, "the pan never arrived");
    }
}

/// Runs the scene to the return pan's op, checking the view stays on the stage
/// from the outbound pan's end.
fn to_the_return_pan(rt: &mut Runtime, stage: (i32, i32)) {
    for _ in 0..2_000 {
        let ops = scene_frame(rt);
        assert_eq!(view(rt), stage, "the view waits on the stage");
        if ops.iter().any(is_pan) {
            return;
        }
    }
    panic!("no return pan");
}

/// Runs the scene to the return pan's op, seating the camera at `seat` while
/// the scene sits in the wait before it: a seat after the op would cancel the
/// glide the op started.
fn to_the_return_pan_seated_at(rt: &mut Runtime, seat: (i32, i32)) {
    for _ in 0..2_000 {
        let ops = scene_frame(rt);
        if ops.iter().any(|op| created(op, 0x348)) {
            rt.set_camera(seat.0, seat.1);
        }
        if ops.iter().any(is_pan) {
            return;
        }
    }
    panic!("no return pan");
}

/// Plays the scene from `column` and checks both pans' boundaries.
fn play_from(column: u16) {
    let mut rt = party_below_the_stage(column);
    step_into_the_scene(&mut rt);
    let home = home_of(&rt);
    assert_eq!(rt.scene_party_actor(0).unwrap().cell.y, TRIGGER_ROW);

    // Up to the outbound pan's op, with the view still on the party.
    let start = loop {
        let at = view(&rt);
        if scene_frame(&mut rt).iter().any(is_pan) {
            break at;
        }
    };
    let distance =
        (start.0 - STAGE_VIEW.0).unsigned_abs() + (start.1 - STAGE_VIEW.1).unsigned_abs();
    let (frames, ops) = follow_pan(&mut rt, STAGE_VIEW, start);
    assert_eq!(
        frames, distance,
        "column {column}: the outbound pan's frames"
    );
    assert!(
        ops.iter().any(|op| created(op, 0x344)),
        "column {column}: the strippers on the arrival frame: {ops:?}"
    );

    to_the_return_pan(&mut rt, STAGE_VIEW);
    let distance = (STAGE_VIEW.0 - home.0).unsigned_abs() + (STAGE_VIEW.1 - home.1).unsigned_abs();
    let (frames, ops) = follow_pan(&mut rt, home, STAGE_VIEW);
    assert_eq!(frames, distance, "column {column}: the return's frames");
    assert!(
        ops.iter().any(|op| created(op, 0x35C)),
        "column {column}: the dancers on the arrival frame: {ops:?}"
    );

    // Past the arrival: one VInt, then the flag and the end of the scene, with
    // the view home and staying there.
    for _ in 0..4 {
        if !rt.scene_active() {
            break;
        }
        checked_tick(&mut rt, Input::Neutral);
    }
    assert!(!rt.scene_active(), "column {column}");
    assert!(rt.game().is_set(STRIPPERS_DANCE));
    assert_eq!(view(&rt), home);
    checked_tick(&mut rt, Input::Neutral);
    assert_eq!(view(&rt), home, "the field takes over where the pan ended");
}

/// Every column of the row in front of the stage (`$1E0 <= x <= $270`): the
/// distances differ, so the frames do, and column 34 (`x = $220`) has no X leg
/// at all.
#[test]
fn both_pans_hold_the_scene_from_every_column_of_the_trigger_row() {
    for column in 30..=39 {
        play_from(column);
    }
}

/// An empty leg takes no frames and the other runs as ever. The camera is
/// seated by hand: the trigger row cannot empty the outbound pan's Y leg.
#[test]
fn an_empty_x_leg_goes_straight_to_y_and_the_pan_still_ends() {
    let mut rt = party_below_the_stage(32);
    step_into_the_scene(&mut rt);
    let home = home_of(&rt);
    rt.set_camera(STAGE_VIEW.0, STAGE_VIEW.1 + 5);
    loop {
        if scene_frame(&mut rt).iter().any(is_pan) {
            break;
        }
    }
    let at = view(&rt);
    let (frames, ops) = follow_pan(&mut rt, STAGE_VIEW, at);
    assert_eq!(frames, 5);
    assert!(ops.iter().any(|op| created(op, 0x344)));

    // The return with an empty Y leg.
    to_the_return_pan_seated_at(&mut rt, (home.0 + 3, home.1));
    let at = view(&rt);
    assert_eq!(at, (home.0 + 3, home.1));
    let (frames, ops) = follow_pan(&mut rt, home, at);
    assert_eq!(frames, 3);
    assert!(ops.iter().any(|op| created(op, 0x35C)));
}

/// Both legs empty: the wait releases on the next frame instead of hanging,
/// with nothing out of order. (The cartridge's routine takes no frame here;
/// the record names the difference.)
#[test]
fn a_pan_with_nothing_to_do_releases_on_the_next_frame() {
    let mut rt = party_below_the_stage(32);
    step_into_the_scene(&mut rt);
    let home = home_of(&rt);
    rt.set_camera(STAGE_VIEW.0, STAGE_VIEW.1);
    loop {
        if scene_frame(&mut rt).iter().any(is_pan) {
            break;
        }
    }
    let ops = scene_frame(&mut rt);
    assert!(ops.iter().any(|op| created(op, 0x344)), "{ops:?}");

    to_the_return_pan_seated_at(&mut rt, home);
    assert_eq!(view(&rt), home);
    let ops = scene_frame(&mut rt);
    assert!(ops.iter().any(|op| created(op, 0x35C)), "{ops:?}");
    assert!(!rt.game().is_set(STRIPPERS_DANCE));
    assert!(rt.scene_active());
}
