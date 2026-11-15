//! The ship's route as each scene that runs it runs it (#654).
//!
//! `Cutscene_InsideSpaceship` (`$07606C`) is `InitVRAMAndCRAM`, `Pal_FadeIn`
//! and `jmp loc_63BC4`; `Cutscene_LeRoofAgain` and `Cutscene_FindingAirCastle`
//! end their own bytes with `jmp Cutscene_InsideSpaceship` (`$0784B0`,
//! `$077A62`), and all three carry [`INSIDE_SPACESHIP_ROUTE`] as their last
//! ops. [`trace`] runs a whole scene through the [`SceneRunner`] down one way
//! out of the menu and records its fixed fades, the menu and the flight's map
//! handoffs in order, so the fade census can compare the route with
//! `loc_63BC4`'s calls on each way and the tests below pin each caller's run.

use super::FlightLeg;
use super::fixed_fades_image::Fixed;
use super::flight::{INSIDE_SPACESHIP_ROUTE, MOTA_SPACEPORT, ZELAN};
use crate::collision::CollisionGrid;
use crate::geom::{Cell, Direction};
use crate::map::{FieldMap, MapId};
use crate::scene::{ActorRef, SceneEffect, SceneInput, SceneOp, ScriptedActor};
use crate::scene_runner::SceneRunner;
use crate::state::{CharId, GameState};

/// The ops of [`INSIDE_SPACESHIP_ROUTE`] that `Cutscene_InsideSpaceship`'s own
/// bytes transcribe: `InitVRAMAndCRAM` and `Pal_FadeIn` (`$07606C`,
/// `$076072`). The rest is `loc_63BC4`'s.
pub(super) const ENTRY_OPS: usize = 2;

/// A way out of the destination menu (`ps4.asm:133616-133726`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Way {
    /// Speak on a row at the Mota or Dezo Spaceport: `loc_64568`, the long
    /// takeoff (`:133669-133674`).
    Spaceport,
    /// Speak on a row anywhere else: `loc_6488A`.
    Elsewhere,
    /// Cancel: `loc_63E5E` (`:133692-133726`).
    Cancel,
}

pub(super) const WAYS: [Way; 3] = [Way::Spaceport, Way::Elsewhere, Way::Cancel];

/// One thing a traced run does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    /// A fixed fade op: `InitVramAndCram` (its `FadeOut` folded in),
    /// `FadeIn` or `FadeOut`.
    Fade(Fixed),
    /// A `FlightFadeIn`.
    FlightFadeIn,
    /// The destination menu.
    Menu,
    /// The post-fade panel/chunk teardown and world commit.
    Destroy,
    Chunks,
    Commit,
    /// A flight map load.
    Leg(FlightLeg),
    /// `SFXID_SpaceshipPropelled`, which only the spaceports' long takeoff
    /// requests (`loc_6462A`).
    Propelled,
}

/// `SFXID_SpaceshipPropelled` (`ps4.constants.asm:1023`).
const SFX_SPACESHIP_PROPELLED: u8 = 0xE3;

/// The map a way's menu is opened on.
fn map_for(way: Way) -> FieldMap {
    let id = if way == Way::Spaceport {
        MOTA_SPACEPORT
    } else {
        ZELAN
    };
    FieldMap::new(
        MapId(id),
        CollisionGrid::filled(8, 8, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap()
}

/// Runs `ops` to its end down `way`, closing each dialogue, answering each
/// palette wait after one frame and each map load at once, and returns what
/// it did. The pan arrives at once. Panics on a fault or a run that does not finish.
pub(super) fn trace(ops: &'static [SceneOp], way: Way) -> Vec<Step> {
    let map = map_for(way);
    let mut state = GameState::new();
    state.set_party([Some(CharId(0)), None, None, None, None]);
    let cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(1, 1),
        Direction::Down,
    )];
    let mut runner = SceneRunner::new(ops, cast, crate::StepFrames::default());
    let mut steps = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..20_000 {
        if runner.awaits_palette() {
            runner.release_palette(1);
        }
        let effects = runner.tick(&map, &mut state, input);
        input = SceneInput::None;
        for effect in effects {
            match effect {
                SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume => {
                    input = SceneInput::DialogueClosed;
                }
                SceneEffect::MapRequested { op } => {
                    input = SceneInput::MapLoaded;
                    if let SceneOp::LoadFlightMap { leg } = op {
                        steps.push(Step::Leg(leg));
                    }
                }
                SceneEffect::Presentation { op } => match op {
                    SceneOp::FlightPlanet => input = SceneInput::MapLoaded,
                    SceneOp::FlightPan => input = SceneInput::CameraArrived,
                    SceneOp::DestinationMenu { .. } => {
                        steps.push(Step::Menu);
                        input = if way == Way::Cancel {
                            SceneInput::DestinationCancelled
                        } else {
                            SceneInput::DestinationChosen
                        };
                    }
                    SceneOp::PanelDestroyAll if steps.contains(&Step::Menu) => {
                        steps.push(Step::Destroy)
                    }
                    SceneOp::Presentation {
                        op: crate::PresentationOp::ReloadMapChunks,
                    } if steps.contains(&Step::Menu) => steps.push(Step::Chunks),
                    SceneOp::CommitDestination => steps.push(Step::Commit),
                    SceneOp::FadeIn => steps.push(Step::Fade(Fixed::In)),
                    SceneOp::FadeOut => steps.push(Step::Fade(Fixed::Out)),
                    // The runner emits `InitVRAMAndCRAM`'s own fade-out
                    // first (`init_vram_and_cram`): the two are one call.
                    SceneOp::InitVramAndCram => {
                        assert_eq!(
                            steps.pop(),
                            Some(Step::Fade(Fixed::Out)),
                            "InitVramAndCram without its fade-out"
                        );
                        steps.push(Step::Fade(Fixed::Init));
                    }
                    SceneOp::FlightFadeIn { .. } => steps.push(Step::FlightFadeIn),
                    SceneOp::PlaySound {
                        id: SFX_SPACESHIP_PROPELLED,
                    } => steps.push(Step::Propelled),
                    _ => {}
                },
                SceneEffect::Faulted(fault) => panic!("the scene faulted: {fault:?}"),
                _ => {}
            }
        }
        if runner.is_finished() {
            return steps;
        }
    }
    panic!("the scene did not finish down {way:?}: {steps:?}");
}

/// The fixed fades among `steps`, in order.
pub(super) fn fades(steps: &[Step]) -> Vec<Fixed> {
    steps
        .iter()
        .filter_map(|step| match step {
            Step::Fade(fixed) => Some(*fixed),
            _ => None,
        })
        .collect()
}

/// The ops of `ops` before the route, when it ends in the route.
pub(super) fn before_route(ops: &'static [SceneOp]) -> Option<&'static [SceneOp]> {
    ops.strip_suffix(INSIDE_SPACESHIP_ROUTE.as_slice())
}

/// Moves every relative skip that crosses `at` by the op inserted there
/// (`grew`) or removed from there.
pub(super) fn retarget(ops: &mut [SceneOp], at: usize, grew: bool) {
    // A skip at `from` passes over `from + 1 .. from + 1 + count`.
    let shift = |from: usize, count: u16| -> u16 {
        let start = from + 1;
        if !(start <= at && at < start + usize::from(count)) {
            count
        } else if grew {
            count + 1
        } else {
            count - 1
        }
    };
    for (from, op) in ops.iter_mut().enumerate() {
        match op {
            SceneOp::SkipOps { count } => *count = shift(from, *count),
            SceneOp::SkipUnlessMap { skip, .. } => *skip = shift(from, *skip),
            SceneOp::DestinationMenu { cancel_skip, .. } => {
                *cancel_skip = shift(from, *cancel_skip);
            }
            _ => {}
        }
    }
}

/// Normalize only the separate FlightFadeIn mismatch. The two menu fade-outs
/// are production ops and must not be synthesized by this census helper.
pub(super) fn route_as_called() -> Vec<SceneOp> {
    INSIDE_SPACESHIP_ROUTE
        .iter()
        .map(|op| match op {
            SceneOp::FlightFadeIn { .. } => SceneOp::FadeIn,
            op => *op,
        })
        .collect()
}

/// Inserts `op` at `at`, growing every relative skip over it.
pub(super) fn insert(ops: &mut Vec<SceneOp>, at: usize, op: SceneOp) {
    ops.insert(at, op);
    retarget(ops, at, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use Fixed::{In, Init, Out};
    use Step::{Chunks, Commit, Destroy, Fade, FlightFadeIn as F, Leg, Menu, Propelled};

    fn scene(name: &str) -> &'static [SceneOp] {
        crate::SCENES
            .iter()
            .find(|scene| scene.name == name)
            .unwrap()
            .ops
    }

    /// What every caller's route does after its own ops, down `way`. The
    /// menu's own fade-outs precede every map handoff. The flight's
    /// `Pal_FadeIn`s remain `FlightFadeIn`s for the dependent migration.
    fn route(way: Way) -> Vec<Step> {
        let mut steps = vec![Fade(Init), Fade(In), Menu, Fade(Out), Destroy, Chunks];
        steps.extend(match way {
            Way::Spaceport => vec![
                Commit,
                Leg(FlightLeg::Takeoff),
                F,
                Propelled,
                Propelled,
                Propelled,
            ],
            Way::Elsewhere => vec![Commit, Leg(FlightLeg::Takeoff), F],
            Way::Cancel => vec![Leg(FlightLeg::Return)],
        });
        if way != Way::Cancel {
            steps.extend([
                Fade(Out),
                Fade(Init),
                F,
                Fade(Out),
                Leg(FlightLeg::Transit),
                F,
                Fade(Out),
                Leg(FlightLeg::Landing),
            ]);
        }
        steps
    }

    /// Each caller's own fixed fades, before the route: Le Roof's two panel
    /// screens (`$07842A`, `$078472`) and the Air Castle's one (`$077A2E`).
    const CALLERS: [(&str, &[Step]); 3] = [
        ("Cutscene_InsideSpaceship", &[]),
        (
            "Cutscene_LeRoofAgain",
            &[Fade(Init), Fade(In), Fade(Init), Fade(In)],
        ),
        ("Cutscene_FindingAirCastle", &[Fade(Init), Fade(In)]),
    ];

    #[test]
    fn each_caller_runs_its_own_fades_then_the_route_down_every_way() {
        for (name, own) in CALLERS {
            for way in WAYS {
                let mut expected = own.to_vec();
                expected.extend(route(way));
                assert_eq!(trace(scene(name), way), expected, "{name} {way:?}");
            }
        }
    }

    /// Every caller's map test sends the spaceports to the long takeoff and
    /// everywhere else to the short one, the next op past its skip.
    #[test]
    fn every_caller_takes_the_long_takeoff_at_a_spaceport_only() {
        for (name, _) in CALLERS {
            let ops = scene(name);
            let skip = ops
                .iter()
                .position(|op| matches!(op, SceneOp::SkipUnlessMap { .. }))
                .unwrap();
            let SceneOp::SkipUnlessMap { maps, skip: count } = ops[skip] else {
                unreachable!()
            };
            assert!(maps.contains(&MOTA_SPACEPORT) && !maps.contains(&ZELAN));
            assert_eq!(
                ops[skip + 1 + usize::from(count)],
                SceneOp::LoadFlightMap {
                    leg: FlightLeg::Takeoff
                },
                "{name}: the short takeoff"
            );
        }
    }

    /// Negative controls: a route copy with one fixed fade dropped or doubled
    /// traces differently from the route down some way.
    #[test]
    fn an_omitted_or_doubled_route_fade_changes_the_trace() {
        let ops = scene("Cutscene_InsideSpaceship");
        let mut altered = 0;
        for (index, op) in ops.iter().enumerate() {
            if !matches!(
                op,
                SceneOp::InitVramAndCram | SceneOp::FadeIn | SceneOp::FadeOut
            ) {
                continue;
            }
            for double in [false, true] {
                let mut copy = ops.to_vec();
                if double {
                    insert(&mut copy, index, *op);
                } else {
                    copy.remove(index);
                    retarget(&mut copy, index, false);
                }
                let copy: &'static [SceneOp] = Box::leak(copy.into_boxed_slice());
                assert!(
                    WAYS.iter()
                        .any(|&way| fades(&trace(copy, way)) != fades(&route(way))),
                    "op {index} {} passed",
                    if double { "doubled" } else { "dropped" }
                );
                altered += 1;
            }
        }
        assert!(altered >= 12, "only {altered} alterations checked");
    }

    #[test]
    fn the_callers_end_in_the_whole_route_and_the_entry_is_its_first_ops() {
        for (name, _) in CALLERS {
            assert!(before_route(scene(name)).is_some(), "{name}");
        }
        assert_eq!(
            INSIDE_SPACESHIP_ROUTE[..ENTRY_OPS],
            [SceneOp::InitVramAndCram, SceneOp::FadeIn]
        );
    }
}
