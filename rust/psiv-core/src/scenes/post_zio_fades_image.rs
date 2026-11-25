//! #657: direct US sites, the aftermath's shared helper and executed branches.
use super::*;
use crate::scene::{ActorRef, SceneEffect, SceneInput, ScriptedActor};
use crate::scene_runner::SceneRunner;
use crate::{Cell, CharId, CollisionGrid, Direction, FieldMap, GameState, MapId};
use Fixed::{In, Init, Out};

const SABOTAGE: &str = "Cutscene_SpaceshipSabotage";
const CRASH: &str = "Cutscene_CrashLaanding";
const AFTERMATH: &str = "Cutscene_DarkForce1Defeated";

/// Instruction meanings are the expectations; instruction encodings are
/// decoded from the private image rather than copied into byte fixtures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Call(u32),
    Subroutine(u32),
    CompareByte { value: u8, address: u32 },
    NotEqual(u32),
}

fn word_at(rom: &[u8], at: usize) -> u16 {
    u16::from_be_bytes(rom[at..at + 2].try_into().unwrap())
}

/// The absolute-long call, word-relative BSR/BNE and immediate byte compare
/// to absolute-short RAM used at these boundaries (68000 instruction forms).
fn decoded_flow(rom: &[u8], at: usize) -> Option<Flow> {
    match word_at(rom, at) {
        0x4EB9 => Some(Flow::Call(u32::from_be_bytes(
            rom[at + 2..at + 6].try_into().unwrap(),
        ))),
        opcode @ (0x6100 | 0x6600) => {
            let displacement = i32::from(word_at(rom, at + 2) as i16);
            let target = u32::try_from(at as i32 + 2 + displacement).ok()?;
            Some(if opcode == 0x6100 {
                Flow::Subroutine(target)
            } else {
                Flow::NotEqual(target)
            })
        }
        0x0C38 => Some(Flow::CompareByte {
            value: u8::try_from(word_at(rom, at + 2)).ok()?,
            address: i32::from(word_at(rom, at + 4) as i16) as u32,
        }),
        _ => None,
    }
}

/// Call targets, compared world/RAM operand and the two branch destinations.
/// The helper's world-2 diversion is bypassed with the caller's world 3.
const CONTROL_FLOW: &[(usize, Flow)] = &[
    (0x07722A, Flow::Call(0x064C1E)),
    (0x064C4A, Flow::Subroutine(0x0643B8)),
    (0x064C4E, Flow::Subroutine(0x0644AE)),
    (
        0x064C1E,
        Flow::CompareByte {
            value: 2,
            address: 0xFFFF_F400,
        },
    ),
    (0x064C24, Flow::NotEqual(0x064C4A)),
    (0x07620C, Flow::NotEqual(0x0764F8)),
];

#[test]
fn direct_calls_are_the_verified_us_sites() {
    let Some(rom) = rom() else { return };
    for (name, span, expected) in [
        (
            SABOTAGE,
            (0x07607E, 0x07658A),
            &[
                (0x07607E, Init),
                (0x076084, In),
                (0x0762B6, Out),
                (0x076344, In),
                (0x0763CA, Out),
                (0x0763F4, Init),
                (0x0763FA, In),
                (0x076524, Out),
            ][..],
        ),
        (
            CRASH,
            (0x07658A, 0x077150),
            &[
                (0x07658A, Init),
                (0x076590, In),
                (0x07672A, Init),
                (0x076764, In),
                (0x07677A, Out),
                (0x076804, In),
                (0x07681A, Out),
                (0x076906, In),
                (0x076B0E, Init),
                (0x076B22, In),
                (0x076C0C, In),
            ][..],
        ),
        (
            AFTERMATH,
            (0x0771D2, 0x07734C),
            &[
                (0x0771D2, Init),
                (0x0771D8, In),
                (0x077214, Init),
                (0x07726E, Init),
                (0x077274, In),
            ][..],
        ),
    ] {
        assert_eq!(retail_calls(rom, &[span]), expected, "{name}");
        assert!(!KNOWN.iter().any(|(known, _)| *known == name));
    }
    for &(at, meaning) in CONTROL_FLOW {
        assert_eq!(
            decoded_flow(rom, at),
            Some(meaning),
            "instruction at ${at:06X}"
        );
    }
    assert_eq!(
        retail_calls(rom, &[(0x064C1E, 0x064D00)]),
        [(0x064C8E, In), (0x064CFA, Out)]
    );
    // The helper's BSRs enter only asset/sprite setup, with no fixed fade.
    // Sabotage calls the menu and ship setup routines in this same span.
    assert!(retail_calls(rom, &[(0x063F00, 0x064568)]).is_empty());
}

/// Neither a wrong instruction kind nor a changed operand may satisfy the
/// semantic pin. Mutations use a generic RTS encoding and a bit flip on the
/// local image; no retail byte run becomes a tracked expectation.
#[test]
fn semantic_flow_pins_reject_a_changed_instruction_or_operand() {
    let Some(rom) = rom() else { return };
    for &(at, meaning) in CONTROL_FLOW {
        let mut wrong_opcode = rom.to_vec();
        wrong_opcode[at..at + 2].copy_from_slice(&0x4E75_u16.to_be_bytes());
        assert_eq!(decoded_flow(&wrong_opcode, at), None, "${at:06X}");

        let mut wrong_operand = rom.to_vec();
        let length = match meaning {
            Flow::Call(_) | Flow::CompareByte { .. } => 6,
            Flow::Subroutine(_) | Flow::NotEqual(_) => 4,
        };
        wrong_operand[at + length - 1] ^= 1;
        assert_ne!(decoded_flow(&wrong_operand, at), Some(meaning), "${at:06X}");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Boundary {
    Fade(Fixed),
    Menu,
    Destroy,
    Chunks,
    Commit,
    Planet,
    Panel(u16),
    Map(u16),
    ReturnMap,
    World(u8),
    Battle(u16),
    Return(u16),
}

fn scene(name: &str) -> &'static crate::Scene {
    SCENES.iter().find(|scene| scene.name == name).unwrap()
}

/// Isolated core trace: palette waits take one frame, map/camera barriers
/// are acknowledged, dialogue is closed, and battle 8 is acknowledged as won.
/// Runtime tests own the actual palette clocks and loaded-map/party outcomes.
fn trace_scene(ops: &'static [SceneOp], cancel: bool) -> (Vec<Boundary>, GameState) {
    let map = FieldMap::new(
        MapId(0x18D),
        CollisionGrid::filled(256, 256, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap();
    let mut state = GameState::new();
    let party = [CharId(0), CharId(5), CharId(3), CharId(7)];
    state.set_party([
        Some(party[0]),
        Some(party[1]),
        Some(party[2]),
        Some(party[3]),
        None,
    ]);
    state.inventory_mut().add(0x9A).unwrap();
    let cast = party
        .into_iter()
        .enumerate()
        .flat_map(|(slot, who)| {
            [
                ScriptedActor::new(ActorRef::Character(who), Cell::new(8, 8), Direction::Down),
                ScriptedActor::new(
                    ActorRef::PartyMember(slot),
                    Cell::new(8, 8),
                    Direction::Down,
                ),
            ]
        })
        .collect();
    let mut runner = SceneRunner::new(ops, cast, crate::StepFrames::default());
    let mut seen = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..20000 {
        if runner.awaits_palette() {
            runner.release_palette(1);
        }
        let effects = runner.tick(&map, &mut state, input);
        input = SceneInput::None;
        for effect in effects {
            let boundary = match effect {
                SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume => {
                    input = SceneInput::DialogueClosed;
                    None
                }
                SceneEffect::WorldIndexSet { world } => Some(Boundary::World(world)),
                SceneEffect::MapRequested { op } => {
                    input = SceneInput::MapLoaded;
                    Some(match op {
                        SceneOp::LoadMap { map, .. } => Boundary::Map(map),
                        SceneOp::LoadFlightMap {
                            leg: crate::FlightLeg::Return,
                        } => Boundary::ReturnMap,
                        _ => panic!("unexpected map request {op:?}"),
                    })
                }
                SceneEffect::BattleRequested { index } => {
                    assert!(state.is_set(crate::Flag::event(0x71)));
                    input = SceneInput::BattleFinished {
                        outcome: crate::battle::Outcome::Victory,
                    };
                    Some(Boundary::Battle(index))
                }
                SceneEffect::Returned { value } => Some(Boundary::Return(value)),
                SceneEffect::Presentation { op } => match op {
                    SceneOp::InitVramAndCram => {
                        assert_eq!(seen.pop(), Some(Boundary::Fade(Out)));
                        Some(Boundary::Fade(Init))
                    }
                    SceneOp::FadeIn => Some(Boundary::Fade(In)),
                    SceneOp::FadeOut => Some(Boundary::Fade(Out)),
                    SceneOp::PanelCreate { id } => Some(Boundary::Panel(id)),
                    SceneOp::PanelDestroyAll => Some(Boundary::Destroy),
                    SceneOp::Presentation {
                        op: crate::PresentationOp::ReloadMapChunks,
                    } => Some(Boundary::Chunks),
                    SceneOp::DestinationMenu { .. } => {
                        input = if cancel {
                            SceneInput::DestinationCancelled
                        } else {
                            SceneInput::DestinationChosen
                        };
                        Some(Boundary::Menu)
                    }
                    SceneOp::CommitDestination => Some(Boundary::Commit),
                    SceneOp::FlightPlanet => {
                        input = SceneInput::MapLoaded;
                        Some(Boundary::Planet)
                    }
                    SceneOp::PanCamera { .. } => {
                        input = SceneInput::CameraArrived;
                        None
                    }
                    _ => None,
                },
                SceneEffect::Faulted(fault) => panic!("trace faulted: {fault:?}"),
                _ => None,
            };
            seen.extend(boundary);
        }
        if runner.is_finished() {
            return (seen, state);
        }
    }
    panic!("trace did not finish: {seen:?}");
}

#[test]
fn sabotage_selection_and_cancel_fade_at_their_own_boundaries() {
    use Boundary::*;
    let (confirmed, state) = trace_scene(scene(SABOTAGE).ops, false);
    assert_eq!(
        confirmed,
        [
            Fade(Init),
            Fade(In),
            Menu,
            Fade(Out),
            Destroy,
            Chunks,
            Commit,
            Map(0x18C),
            Fade(In),
            Fade(Out),
            Fade(Init),
            Fade(In),
            Panel(0x7A),
            Panel(0x7C),
            Destroy,
            Panel(0x7D),
            Panel(0x7E),
            Battle(8),
            Return(1)
        ]
    );
    assert!(state.is_set(crate::Flag::event(0x71)));
    let (cancelled, state) = trace_scene(scene(SABOTAGE).ops, true);
    assert_eq!(
        cancelled,
        [
            Fade(Init),
            Fade(In),
            Menu,
            Fade(Out),
            Destroy,
            Chunks,
            ReturnMap,
            Return(0)
        ]
    );
    assert!(state.is_clear(crate::Flag::event(0x71)));
}

#[test]
fn crash_and_aftermath_fade_at_their_map_and_panel_boundaries() {
    use Boundary::*;
    let (crash, state) = trace_scene(scene(CRASH).ops, false);
    assert_eq!(
        crash,
        [
            Fade(Init),
            Fade(In),
            Panel(0x80),
            Destroy,
            Chunks,
            Panel(0x86),
            Destroy,
            Fade(Init),
            Panel(0x87),
            Fade(In),
            Fade(Out),
            Map(1),
            World(1),
            Fade(In),
            Fade(Out),
            Map(0x14C),
            Fade(In),
            Fade(Init),
            Fade(In),
            Panel(0x88),
            Map(0x14C),
            Fade(In),
            Return(1)
        ]
    );
    assert_eq!(
        state.party_members(),
        [CharId(0), CharId(5), CharId(3), CharId(7), CharId(8)]
    );
    assert!(state.is_set(crate::Flag::event(0x85)) && state.is_set(crate::Flag::event(0x88)));
    let (aftermath, state) = trace_scene(scene(AFTERMATH).ops, false);
    assert_eq!(
        aftermath,
        [
            Fade(Init),
            Fade(In),
            Panel(0x8E),
            Fade(Init),
            World(3),
            Planet,
            Fade(In),
            Fade(Out),
            Map(0x18E),
            Fade(Init),
            Fade(In),
            Panel(0x92),
            Panel(0x93),
            Return(0)
        ]
    );
    assert!(state.is_set(crate::Flag::event(0x89)));
    assert!(state.inventory().slots().contains(&0x97));
    assert!(!state.inventory().slots().contains(&0x9A));
}

/// The image census rejects missing/doubled/wrong kinds; these traces also
/// reject the right kind moved across a map or panel boundary. Check both
/// sabotage ways so its linear address-order census cannot hide a bad skip.
#[test]
fn omitted_doubled_and_misplaced_fades_change_the_observable_trace() {
    for name in [SABOTAGE, CRASH, AFTERMATH] {
        let original = scene(name).ops;
        let ways: &[bool] = if name == SABOTAGE {
            &[false, true]
        } else {
            &[false]
        };
        let expected: Vec<_> = ways
            .iter()
            .map(|&cancel| trace_scene(original, cancel).0)
            .collect();
        let mut checked = 0;
        for (at, op) in original.iter().enumerate() {
            if recorded(op).is_none() {
                continue;
            }
            let mut dropped = original.to_vec();
            dropped.remove(at);
            retarget(&mut dropped, at, false);
            let mut doubled = original.to_vec();
            insert(&mut doubled, at, op.clone());
            for ops in [dropped, doubled] {
                let ops = Box::leak(ops.into_boxed_slice());
                let observed: Vec<_> = ways
                    .iter()
                    .map(|&cancel| trace_scene(ops, cancel).0)
                    .collect();
                assert_ne!(observed, expected, "{name} fade {at} alteration escaped");
                checked += 1;
            }
        }
        // Move the repaired fade across its map or panel boundary. Its
        // kind and count stay right, so the call-count census alone passes.
        let (at, to) = match name {
            SABOTAGE => (14, 12),  // takeoff In -> before its map load
            CRASH => (63, 56),     // temple In -> before its map load
            AFTERMATH => (21, 18), // post-refresh Init -> before its map load
            _ => unreachable!(),
        };
        let mut moved = original.to_vec();
        let op = moved.remove(at);
        retarget(&mut moved, at, false);
        insert(&mut moved, if to > at { to - 1 } else { to }, op);
        assert_eq!(
            native(&moved),
            native(original),
            "control preserves fade order"
        );
        let ops = Box::leak(moved.into_boxed_slice());
        let observed: Vec<_> = ways
            .iter()
            .map(|&cancel| trace_scene(ops, cancel).0)
            .collect();
        assert_ne!(observed, expected, "{name} misplaced fade escaped");
        assert_eq!(checked, 2 * native(original).len());
    }
}
