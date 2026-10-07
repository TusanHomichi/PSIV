//! The Vahal Fort and Weapon Plant ops and scenes (issue #82), run on
//! synthetic maps: the platform ride, the conveyor carry, the party-state ops
//! and the nineteen transcriptions' effect sequences. The retail bytes behind
//! each number are checked by `tests/test_vahal_events.py`; the runtime half
//! (live layout, collision, the trigger that fires them) is
//! `rust/psiv-runtime/src/scene_vahal_tests.rs`.

use super::*;
use crate::battle::{StatPair, StatTriple, Stats};
use crate::scene::{DialogueId, SceneFault};
use crate::{CollisionGrid, Direction, EventIndex, Flag, MapId, PixelPos, scene_for};

fn stats() -> Stats {
    Stats {
        name_bytes: [0; 6],
        profession: 0,
        level: 1,
        experience: 0,
        curr_hp: 20,
        max_hp: 25,
        curr_tp: 10,
        max_tp: 10,
        status: 0,
        strength: StatTriple::uniform(8),
        mental: StatTriple::uniform(6),
        agility: StatTriple::uniform(7),
        dexterity: StatTriple::uniform(5),
        attack: StatPair::default(),
        defence: StatPair::default(),
        mental_defence: StatPair::default(),
        element_props: [0; 14],
        element_shadow: [0; 14],
        equipment: [0; 4],
        techniques: [0; 16],
        skills: [1, 2, 3, 4, 0, 0, 0, 0],
        curr_skill_uses: [0; 8],
        max_skill_uses: [0; 8],
        enemy_id: 0,
        gain_exp_flag: false,
        weapon_elements: Default::default(),
        physical_prop_save: 0,
    }
}

const CHAZ: u8 = 0;
const HAHN: u8 = 2;
const RUNE: u8 = 3;
const GRYZ: u8 = 4;
const RIKA: u8 = 5;
const DEMI: u8 = 6;
const WREN: u8 = 7;
const RAJA: u8 = 8;
const KYRA: u8 = 9;
const SETH: u8 = 10;

/// A party of `members` in slot order, each with a roster seat.
fn state_with(members: &[u8]) -> GameState {
    let mut state = GameState::default();
    let mut slots = [None; PARTY_SLOTS];
    for (slot, &id) in members.iter().enumerate() {
        slots[slot] = Some(CharId(id));
        state.roster_mut().seat(CharId(id), stats()).unwrap();
    }
    state.set_party(slots);
    state
}

/// The party's actors, stacked on `at`.
fn cast_at(members: usize, at: Cell) -> Vec<ScriptedActor> {
    (0..members)
        .map(|slot| ScriptedActor::new(ActorRef::PartyMember(slot), at, Direction::Down))
        .collect()
}

fn big_map() -> FieldMap {
    FieldMap::new(
        MapId(0xCA),
        CollisionGrid::filled(96, 96, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap()
}

/// What running a scene to its end produced.
struct Played {
    effects: Vec<SceneEffect>,
    /// Every actor's cell at the end of each tick that opened or resumed a
    /// dialogue, in order.
    at_dialogue: Vec<Vec<(ActorRef, Cell)>>,
    /// Ticks the runner was ticked, the last one included.
    ticks: usize,
    runner: SceneRunner,
}

impl Played {
    fn faults(&self) -> Vec<SceneFault> {
        self.effects
            .iter()
            .filter_map(|e| match e {
                SceneEffect::Faulted(fault) => Some(*fault),
                _ => None,
            })
            .collect()
    }

    fn leader(&self) -> ScriptedActor {
        *self.runner.actor(ActorRef::PartyMember(0)).unwrap()
    }

    fn writes(&self) -> Vec<Vec<(u32, u32, u16)>> {
        self.effects
            .iter()
            .filter_map(|e| match e {
                SceneEffect::MapChunksWritten { chunks } => Some(chunks.clone()),
                _ => None,
            })
            .collect()
    }

    fn dialogues(&self) -> Vec<u16> {
        self.effects
            .iter()
            .filter_map(|e| match e {
                SceneEffect::DialogueOpen(DialogueId(id)) => Some(*id),
                _ => None,
            })
            .collect()
    }

    fn sounds(&self) -> Vec<u8> {
        self.effects
            .iter()
            .filter_map(|e| match e {
                SceneEffect::Presentation {
                    op: SceneOp::PlaySound { id },
                } => Some(*id),
                _ => None,
            })
            .collect()
    }

    fn flag_writes(&self) -> Vec<(Flag, bool)> {
        self.effects
            .iter()
            .filter_map(|e| match e {
                SceneEffect::FlagChanged { flag, value } => Some((*flag, *value)),
                _ => None,
            })
            .collect()
    }
}

/// Runs `event`'s scene to its end, closing every dialogue at once.
fn play(
    event: u16,
    state: &mut GameState,
    map: &FieldMap,
    cast: Vec<ScriptedActor>,
    chunks: &dyn Fn(PixelPos) -> Option<u16>,
) -> Played {
    let scene = scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} unregistered"));
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::default());
    let mut effects: Vec<SceneEffect> = Vec::new();
    let mut at_dialogue = Vec::new();
    let mut ticks = 0;
    let mut input = SceneInput::None;
    while !runner.is_finished() {
        ticks += 1;
        assert!(ticks < 20_000, "${event:04X} did not finish");
        let fresh = runner.tick_with(map, state, input, Some(chunks));
        input = if fresh.iter().any(|e| {
            matches!(
                e,
                SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume
            )
        }) {
            at_dialogue.push(runner.actors().iter().map(|a| (a.actor, a.cell)).collect());
            SceneInput::DialogueClosed
        } else {
            SceneInput::None
        };
        effects.extend(fresh);
    }
    Played {
        effects,
        at_dialogue,
        ticks,
        runner,
    }
}

fn no_layout(_: PixelPos) -> Option<u16> {
    None
}

// ---------------------------------------------------------------------------
// The small ops
// ---------------------------------------------------------------------------

static TOGGLE_TWICE: &[SceneOp] = &[
    SceneOp::ToggleFlag {
        flag: Flag::temp(0x0B),
    },
    SceneOp::ToggleFlag {
        flag: Flag::temp(0x0B),
    },
];

#[test]
fn toggle_flag_flips_the_flag_and_a_second_toggle_puts_it_back() {
    let map = big_map();
    let mut state = GameState::default();
    let mut once = SceneRunner::new(&TOGGLE_TWICE[..1], vec![], StepFrames::default());
    once.tick(&map, &mut state, SceneInput::None);
    assert!(
        state.is_set(Flag::temp(0x0B)),
        "bchg on a clear bit sets it"
    );
    let mut twice = SceneRunner::new(TOGGLE_TWICE, vec![], StepFrames::default());
    let effects = twice.tick(&map, &mut state, SceneInput::None);
    assert!(
        state.is_set(Flag::temp(0x0B)),
        "two more toggles of a set flag leave it set"
    );
    let writes: Vec<_> = effects
        .iter()
        .filter_map(|e| match e {
            SceneEffect::FlagChanged { value, .. } => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(
        writes,
        [false, true],
        "the set flag first clears, then sets"
    );
}

static PARTY_BRANCH: &[SceneOp] = &[
    SceneOp::BranchIfPartyMember {
        who: CharId(DEMI),
        if_present: 1,
        if_absent: 2,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x01),
        value: true,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x02),
        value: true,
    },
];

#[test]
fn branch_if_party_member_follows_find_character_slot() {
    let map = big_map();
    for (members, present) in [(&[CHAZ, DEMI][..], true), (&[CHAZ, RUNE][..], false)] {
        let mut state = state_with(members);
        let mut runner = SceneRunner::new(PARTY_BRANCH, vec![], StepFrames::default());
        runner.tick(&map, &mut state, SceneInput::None);
        // Present: op 1 and then op 2 run (fallthrough); absent: op 2 only.
        assert_eq!(state.is_set(Flag::event(0x01)), present, "{members:?}");
        assert!(state.is_set(Flag::event(0x02)));
    }
}

static FACE_PARTY: &[SceneOp] = &[SceneOp::FaceParty {
    facing: Direction::Up,
}];

#[test]
fn face_party_turns_every_present_character_and_skips_empty_slots() {
    let map = big_map();
    let mut state = state_with(&[CHAZ, RUNE, RIKA]);
    let mut runner = SceneRunner::new(
        FACE_PARTY,
        cast_at(3, Cell::new(5, 5)),
        StepFrames::default(),
    );
    let effects = runner.tick(&map, &mut state, SceneInput::None);
    for slot in 0..3 {
        assert_eq!(
            runner.actor(ActorRef::PartyMember(slot)).unwrap().facing,
            Direction::Up
        );
    }
    let faced = effects
        .iter()
        .filter(|e| matches!(e, SceneEffect::ActorFaced { .. }))
        .count();
    assert_eq!(faced, 3, "slots 3 and 4 hold no object");
    assert!(!effects.iter().any(|e| matches!(e, SceneEffect::Faulted(_))));
}

static SKILL: &[SceneOp] = &[SceneOp::SetCharacterSkill {
    who: CharId(WREN),
    slot: 5,
    skill: 0x0E,
}];
static BAD_SKILL_SLOT: &[SceneOp] = &[SceneOp::SetCharacterSkill {
    who: CharId(WREN),
    slot: 8,
    skill: 0x0E,
}];

#[test]
fn set_character_skill_writes_one_skill_byte_and_nothing_else() {
    let map = big_map();
    let mut state = state_with(&[CHAZ, WREN]);
    let before = state.roster().get(CharId(WREN)).unwrap().clone();
    let mut runner = SceneRunner::new(SKILL, vec![], StepFrames::default());
    let effects = runner.tick(&map, &mut state, SceneInput::None);
    let after = state.roster().get(CharId(WREN)).unwrap();
    assert_eq!(after.skills, [1, 2, 3, 4, 0, 0x0E, 0, 0]);
    let mut expected = before;
    expected.skills[5] = 0x0E;
    assert_eq!(*after, expected, "only the skill byte moved");
    assert!(effects.contains(&SceneEffect::RosterChanged { who: CharId(WREN) }));
    // Negative controls: a slot past the array, and a character with no seat.
    let mut runner = SceneRunner::new(BAD_SKILL_SLOT, vec![], StepFrames::default());
    let effects = runner.tick(&map, &mut state, SceneInput::None);
    assert!(effects.contains(&SceneEffect::Faulted(SceneFault::BadWrite)));
    let mut unseated = state_with(&[CHAZ]);
    let mut runner = SceneRunner::new(SKILL, vec![], StepFrames::default());
    let effects = runner.tick(&map, &mut unseated, SceneInput::None);
    assert!(effects.contains(&SceneEffect::Faulted(SceneFault::BadWrite)));
}

// ---------------------------------------------------------------------------
// The platform ride
// ---------------------------------------------------------------------------

/// One platform, spelled out from its retail tables
/// (`docs/field/PLATFORMS_AND_BELTS.md`) independently of the scene source:
/// the chunk pairs, the distance and which way each flag state rides.
struct Platform {
    event: u16,
    flag: u16,
    /// The leader's standing cell on the start chunk with the flag clear.
    clear_cell: Cell,
    /// Whether the clear state rides down (the set state rides back up).
    clear_down: bool,
    /// Start chunk pair `(x, y)` and its ids, flag clear / set.
    start: [((u32, u32), (u16, u16)); 2],
    /// End chunk pair, flag clear / set.
    end: [(u32, u32); 2],
    /// Frames of ride, two pixels each.
    frames: u16,
    /// The first free object slot of the platform's map.
    slot: usize,
}

const PLATFORMS: [Platform; 6] = [
    Platform {
        event: 0x15,
        flag: 0x09,
        clear_cell: Cell::new(45, 34),
        clear_down: true,
        start: [((22, 17), (0xC2, 0xC3)), ((22, 22), (0xC0, 0xC1))],
        end: [(22, 22), (22, 17)],
        frames: 80,
        slot: 3,
    },
    // The second Vahal Fort platform rises from the bottom when clear.
    Platform {
        event: 0x16,
        flag: 0x0A,
        clear_cell: Cell::new(45, 58),
        clear_down: false,
        start: [((22, 29), (0xC0, 0xC1)), ((22, 24), (0xC0, 0xC1))],
        end: [(22, 24), (22, 29)],
        frames: 80,
        slot: 3,
    },
    Platform {
        event: 0x17,
        flag: 0x0D,
        clear_cell: Cell::new(29, 42),
        clear_down: true,
        start: [((14, 21), (0xC2, 0xC3)), ((14, 28), (0xC0, 0xC1))],
        end: [(14, 28), (14, 21)],
        frames: 112,
        slot: 5,
    },
    Platform {
        event: 0x18,
        flag: 0x0E,
        clear_cell: Cell::new(35, 22),
        clear_down: true,
        start: [((17, 11), (0xC2, 0xC3)), ((17, 18), (0xC0, 0xC1))],
        end: [(17, 18), (17, 11)],
        frames: 112,
        slot: 5,
    },
    Platform {
        event: 0x19,
        flag: 0x0F,
        clear_cell: Cell::new(57, 22),
        clear_down: true,
        start: [((28, 11), (0xC2, 0xC3)), ((28, 18), (0xC0, 0xC1))],
        end: [(28, 18), (28, 11)],
        frames: 112,
        slot: 5,
    },
    Platform {
        event: 0x1A,
        flag: 0x10,
        clear_cell: Cell::new(63, 42),
        clear_down: true,
        start: [((31, 21), (0xC2, 0xC3)), ((31, 28), (0xC0, 0xC1))],
        end: [(31, 28), (31, 21)],
        frames: 112,
        slot: 5,
    },
];

fn pair(at: (u32, u32), ids: (u16, u16)) -> Vec<(u32, u32, u16)> {
    vec![(at.0, at.1, ids.0), (at.0 + 1, at.1, ids.1)]
}

/// Each of the six platforms, in both flag states: the party ends exactly the
/// platform's distance away, the flag flips, the start pair and end pair are
/// written in that order, and the platform object rides and is cleared.
#[test]
fn every_platform_carries_the_party_to_the_other_end_and_flips_its_flag() {
    let map = big_map();
    for platform in PLATFORMS {
        for set in [false, true] {
            let mut state = state_with(&[CHAZ, RUNE, RIKA]);
            let flag = Flag::temp(platform.flag);
            // Two pixels a frame, sixteen pixels a cell.
            let rows = i32::from(platform.frames) / 8;
            let clear_sign = if platform.clear_down { 1 } else { -1 };
            let (from_row, travel) = if set {
                state.set(flag).unwrap();
                // The set state starts where the clear state ends.
                (
                    i32::from(platform.clear_cell.y) + clear_sign * rows,
                    -clear_sign * rows,
                )
            } else {
                (i32::from(platform.clear_cell.y), clear_sign * rows)
            };
            let from = Cell::new(platform.clear_cell.x, from_row as u16);
            let played = play(
                platform.event,
                &mut state,
                &map,
                cast_at(3, from),
                &no_layout,
            );
            let label = format!("${:02X} set={set}", platform.event);
            assert_eq!(played.faults(), vec![], "{label}");
            // Rigid party: all three at the same cell, no sub-cell slide.
            let expected = Cell::new(from.x, (from_row + travel) as u16);
            for slot in 0..3 {
                let member = played.runner.actor(ActorRef::PartyMember(slot)).unwrap();
                assert_eq!(member.cell, expected, "{label} slot {slot}");
                assert_eq!(member.render_offset_16ths(StepFrames::default()), (0, 0));
            }
            // The flag flipped exactly once.
            assert_eq!(state.is_set(flag), !set, "{label}");
            assert_eq!(played.flag_writes(), vec![(flag, !set)], "{label}");
            // Start pair, then end pair, in that order.
            let which = usize::from(set);
            let (start_at, ids) = platform.start[which];
            let writes = played.writes();
            assert_eq!(writes.len(), 2, "{label}");
            assert_eq!(writes[0], pair(start_at, ids), "{label}");
            assert_eq!(
                writes[1],
                pair(platform.end[which], (0xCC, 0xCD)),
                "{label}"
            );
            // The platform object rides two pixels a frame the way the party
            // does, on the first free slot, then goes.
            let rides: Vec<_> = played
                .effects
                .iter()
                .filter_map(|e| match e {
                    SceneEffect::Presentation {
                        op:
                            SceneOp::StepFieldObject {
                                slot,
                                step_y,
                                frames,
                                ..
                            },
                    } => Some((*slot, *step_y, *frames)),
                    _ => None,
                })
                .collect();
            let expected_step = if travel > 0 { 0x2_0000 } else { -0x2_0000 };
            assert_eq!(
                rides,
                vec![(platform.slot, expected_step, platform.frames)],
                "{label}"
            );
            assert!(played.effects.contains(&SceneEffect::NpcDespawned {
                npc_index: platform.slot,
                count: 1
            }));
            assert_eq!(played.sounds(), vec![0xE7, 0xFC], "{label}");
            // Sixty frames' settle, then the ride.
            assert!(
                played.ticks >= usize::from(platform.frames) + 60,
                "{label}: {} ticks",
                played.ticks
            );
        }
    }
}

/// Mid-ride the party is between cells: eight frames at two pixels a frame is
/// one cell, and four frames is half of one.
#[test]
fn a_ride_moves_the_party_two_pixels_a_frame_and_rolls_into_cells() {
    let map = big_map();
    let mut state = state_with(&[CHAZ, RUNE, RIKA]);
    let scene = scene_for(EventIndex(0x15)).unwrap();
    let mut runner = SceneRunner::new(
        scene.ops,
        cast_at(3, Cell::new(45, 34)),
        StepFrames::default(),
    );
    let mut ride_ticks = 0;
    let mut samples = Vec::new();
    for _ in 0..400 {
        let effects = runner.tick(&map, &mut state, SceneInput::None);
        if effects.iter().any(|e| {
            matches!(
                e,
                SceneEffect::Presentation {
                    op: SceneOp::StepFieldObject { .. }
                }
            )
        }) {
            ride_ticks = 1;
            continue;
        }
        if ride_ticks > 0 {
            ride_ticks += 1;
            let leader = runner.actor(ActorRef::PartyMember(0)).unwrap();
            samples.push((
                ride_ticks - 1,
                leader.cell.y,
                leader.render_offset_16ths(StepFrames::default()).1,
            ));
        }
        if runner.is_finished() {
            break;
        }
    }
    let at = |frame: usize| samples.iter().find(|s| s.0 == frame).copied().unwrap();
    // `(frames into the ride, cell row, sub-cell pixels)`.
    assert_eq!(at(4), (4, 34, 8));
    assert_eq!(at(8), (8, 35, 0));
    assert_eq!(at(12), (12, 35, 8));
    assert_eq!(at(80).1, 44);
    assert_eq!(at(80).2, 0);
}

static MISMATCHED_RIDE: &[SceneOp] = &[SceneOp::RidePlatform {
    slot: 3,
    step_y: 0x2_0000,
    frames: 7,
}];

#[test]
fn a_ride_that_ends_between_cells_is_a_fault_not_a_stranded_party() {
    let map = big_map();
    let mut state = state_with(&[CHAZ]);
    let mut runner = SceneRunner::new(
        MISMATCHED_RIDE,
        cast_at(1, Cell::new(5, 5)),
        StepFrames::default(),
    );
    let effects = runner.tick(&map, &mut state, SceneInput::None);
    assert!(effects.contains(&SceneEffect::Faulted(SceneFault::BadWrite)));
    assert_eq!(
        runner.actor(ActorRef::PartyMember(0)).unwrap().cell,
        Cell::new(5, 5)
    );
}

// ---------------------------------------------------------------------------
// The conveyor carry
// ---------------------------------------------------------------------------

/// A layout whose belt chunks (`first..=last`, cycled through to mimic the
/// belts' animation) fill the chunk rectangle `cols x rows`, with chunk `0`
/// everywhere else. The probe reads a pixel as `GetChunkAndCollision` does.
fn belt_layout(
    cols: std::ops::RangeInclusive<i32>,
    rows: std::ops::RangeInclusive<i32>,
    first: u16,
) -> impl Fn(PixelPos) -> Option<u16> {
    move |at: PixelPos| {
        if at.x < 0 || at.y < 0 || at.x >= 96 * 16 || at.y >= 96 * 16 {
            return None;
        }
        let (cx, cy) = (at.x / 32, at.y / 32);
        if cols.contains(&cx) && rows.contains(&cy) {
            Some(first + ((cx + cy) % 4) as u16)
        } else {
            Some(0)
        }
    }
}

/// One belt case: the event, the leader's start cell, where the carry ends
/// and how many cells that is. The ends were derived on paper from the
/// cartridge's per-pixel test (`GetChunkAndCollision` after each frame), not
/// from the implementation: the leader finishes the step that leaves the belt,
/// so it ends on the first cell outside the belt's chunks in the belt's
/// direction, from whichever half of a chunk it started on.
struct Belt {
    event: u16,
    start: Cell,
    end: Cell,
}

#[test]
fn a_belt_carries_the_leader_to_the_first_cell_off_it_at_one_pixel_a_frame() {
    let map = big_map();
    let vertical = belt_layout(3..=3, 10..=14, 0xA8);
    let horizontal = belt_layout(10..=12, 5..=5, 0xAC);
    // The vertical belt is chunk column 3 (cells 6-7), chunk rows 10-14
    // (cells 20-29); the horizontal one is chunk row 5 (cells 10-11), chunk
    // columns 10-12 (cells 20-25). A cell's standing row is its own row, so
    // chunk row = cell row / 2.
    let cases = [
        // Down from a chunk's top row and from its bottom row: both end on row
        // 30, the first row of chunk row 15.
        Belt {
            event: 0x1D,
            start: Cell::new(6, 20),
            end: Cell::new(6, 30),
        },
        Belt {
            event: 0x1D,
            start: Cell::new(6, 21),
            end: Cell::new(6, 30),
        },
        Belt {
            event: 0x1D,
            start: Cell::new(6, 29),
            end: Cell::new(6, 30),
        },
        // Up ends on row 19, the last row of chunk row 9.
        Belt {
            event: 0x1E,
            start: Cell::new(7, 29),
            end: Cell::new(7, 19),
        },
        Belt {
            event: 0x1E,
            start: Cell::new(7, 28),
            end: Cell::new(7, 19),
        },
        Belt {
            event: 0x1E,
            start: Cell::new(7, 20),
            end: Cell::new(7, 19),
        },
        // Right ends on column 26, the first column of chunk column 13.
        Belt {
            event: 0x1F,
            start: Cell::new(20, 11),
            end: Cell::new(26, 11),
        },
        Belt {
            event: 0x1F,
            start: Cell::new(25, 11),
            end: Cell::new(26, 11),
        },
        // Left ends on column 19, the last column of chunk column 9.
        Belt {
            event: 0x20,
            start: Cell::new(25, 11),
            end: Cell::new(19, 11),
        },
        Belt {
            event: 0x20,
            start: Cell::new(20, 11),
            end: Cell::new(19, 11),
        },
    ];
    for case in cases {
        let layout: &dyn Fn(PixelPos) -> Option<u16> = if case.event <= 0x1E {
            &vertical
        } else {
            &horizontal
        };
        let mut state = state_with(&[CHAZ, RUNE, RIKA]);
        let played = play(case.event, &mut state, &map, cast_at(3, case.start), layout);
        let label = format!("${:02X} from {:?}", case.event, case.start);
        assert_eq!(played.faults(), vec![], "{label}");
        // The party is stacked on the leader at the end.
        for slot in 0..3 {
            assert_eq!(
                played
                    .runner
                    .actor(ActorRef::PartyMember(slot))
                    .unwrap()
                    .cell,
                case.end,
                "{label} slot {slot}"
            );
        }
        let cells = u32::from(case.end.x.abs_diff(case.start.x))
            + u32::from(case.end.y.abs_diff(case.start.y));
        // 16 frames a cell (`FieldObj_Step_Offset` 0), plus the op's own
        // ticks; never the party's 8-frame walk.
        let carry = played.ticks as u32;
        assert!(
            carry >= cells * 16 && carry <= cells * 16 + 8,
            "{label}: {carry} ticks for {cells} cells"
        );
        let facing = match case.event {
            0x1D => Direction::Down,
            0x1E => Direction::Up,
            0x1F => Direction::Right,
            _ => Direction::Left,
        };
        assert_eq!(played.leader().facing, facing, "{label}");
        assert_eq!(played.sounds(), vec![0xE7, 0xFC], "{label}");
    }
}

#[test]
fn a_belt_always_takes_at_least_one_step() {
    // The first chunk test comes after one pixel; a leader standing on a cell
    // whose chunk is already outside the belt's ids still takes the step it
    // started (`Event_ConveyorBeltDown` sets the destination before it looks).
    let map = big_map();
    let nowhere = belt_layout(50..=50, 50..=50, 0xA8);
    for (event, start, end) in [
        (0x1D, Cell::new(6, 20), Cell::new(6, 21)),
        (0x1E, Cell::new(6, 20), Cell::new(6, 19)),
        (0x1F, Cell::new(20, 11), Cell::new(21, 11)),
        (0x20, Cell::new(20, 11), Cell::new(19, 11)),
    ] {
        let mut state = state_with(&[CHAZ]);
        let played = play(event, &mut state, &map, cast_at(1, start), &nowhere);
        assert_eq!(played.faults(), vec![], "${event:02X}");
        assert_eq!(played.leader().cell, end, "${event:02X}");
    }
}

#[test]
fn a_belt_with_no_layout_or_no_end_is_a_fault_and_the_party_stays_put() {
    let map = big_map();
    let start = Cell::new(6, 20);
    // No layout at all: a runner ticked without a probe.
    let mut state = state_with(&[CHAZ]);
    let scene = scene_for(EventIndex(0x1D)).unwrap();
    let mut runner = SceneRunner::new(scene.ops, cast_at(1, start), StepFrames::default());
    let mut effects = Vec::new();
    for _ in 0..4 {
        effects.extend(runner.tick(&map, &mut state, SceneInput::None));
    }
    assert!(effects.contains(&SceneEffect::Faulted(SceneFault::NoLayout)));
    assert_eq!(runner.actor(ActorRef::PartyMember(0)).unwrap().cell, start);
    // A belt that never ends: every chunk is a belt chunk.
    let endless = |at: PixelPos| (at.x >= 0 && at.y >= 0).then_some(0xA9);
    let mut state = state_with(&[CHAZ]);
    let played = play(0x1D, &mut state, &map, cast_at(1, start), &endless);
    assert_eq!(played.faults(), vec![SceneFault::NoLayout]);
    assert_eq!(played.leader().cell, start);
}

// ---------------------------------------------------------------------------
// The story scenes
// ---------------------------------------------------------------------------

#[test]
fn the_entrance_midway_and_arrival_talk_once_and_set_their_flags() {
    let map = big_map();
    for (event, entry, flag) in [(0x8D, 0, 0xB4), (0x8E, 3, 0xB5), (0x92, 7, 0xC7)] {
        let mut state = state_with(&[CHAZ]);
        let played = play(
            event,
            &mut state,
            &map,
            cast_at(1, Cell::new(5, 5)),
            &no_layout,
        );
        assert_eq!(played.faults(), vec![], "${event:02X}");
        assert_eq!(played.dialogues(), vec![entry], "${event:02X}");
        assert_eq!(played.flag_writes(), vec![(Flag::event(flag), true)]);
        assert!(state.is_set(Flag::event(flag)));
    }
}

#[test]
fn the_barrier_explains_itself_once_then_only_refuses() {
    let map = big_map();
    let mut state = state_with(&[CHAZ]);
    let first = play(
        0x90,
        &mut state,
        &map,
        cast_at(1, Cell::new(5, 5)),
        &no_layout,
    );
    assert_eq!(first.dialogues(), vec![1]);
    assert!(state.is_set(Flag::event(0xB9)));
    let again = play(
        0x90,
        &mut state,
        &map,
        cast_at(1, Cell::new(5, 5)),
        &no_layout,
    );
    assert_eq!(
        again.dialogues(),
        vec![2],
        "the flag is set: the short line"
    );
    assert_eq!(again.flag_writes(), vec![]);
}

#[test]
fn the_terminals_flip_the_belt_flag_for_the_half_the_leader_stands_in() {
    let map = big_map();
    // (event, leader cell x, flag toggled). `x = cell * 16`; the compare is
    // `cmpi.w #$1F0` / `#$300` with `bcs`, so the boundary column belongs to
    // the upper flag.
    for (event, x, flag) in [
        (0x1B, 30, 0x0C),
        (0x1B, 31, 0x0B),
        (0x1C, 47, 0x11),
        (0x1C, 48, 0x12),
    ] {
        let mut state = state_with(&[CHAZ]);
        let played = play(
            event,
            &mut state,
            &map,
            cast_at(1, Cell::new(x, 20)),
            &no_layout,
        );
        assert_eq!(played.faults(), vec![], "${event:02X} x {x}");
        assert_eq!(
            played.flag_writes(),
            vec![(Flag::temp(flag), true)],
            "${event:02X} x {x}"
        );
        assert_eq!(played.sounds(), vec![0xDB, 0xDB]);
        // Two red fades of 32 frames each.
        assert!(played.ticks >= 64, "{} ticks", played.ticks);
        // Using it again puts the belts back.
        let back = play(
            event,
            &mut state,
            &map,
            cast_at(1, Cell::new(x, 20)),
            &no_layout,
        );
        assert_eq!(back.flag_writes(), vec![(Flag::temp(flag), false)]);
    }
}

#[test]
fn the_chest_events_open_the_lid_teach_wren_the_skill_and_set_their_flag() {
    let map = big_map();
    for (event, chest, entry, slot, skill, flag) in
        [(0x6E, 7, 8, 5, 0x0E, 0x74), (0x6F, 8, 6, 6, 0x0F, 0x90)]
    {
        let mut state = state_with(&[CHAZ, RUNE, RIKA, WREN]);
        let mut cast = cast_at(4, Cell::new(5, 5));
        cast.push(ScriptedActor::new(
            ActorRef::Npc(chest),
            Cell::new(5, 4),
            Direction::Down,
        ));
        let played = play(event, &mut state, &map, cast, &no_layout);
        assert_eq!(played.faults(), vec![], "${event:02X}");
        assert_eq!(
            played.sounds(),
            vec![0xE1, 0xCF, 0xCF, 0xCF],
            "${event:02X}"
        );
        assert_eq!(played.dialogues(), vec![entry], "${event:02X}");
        assert!(played.effects.contains(&SceneEffect::DialogueResume));
        assert!(played.effects.contains(&SceneEffect::ActorFaced {
            actor: ActorRef::Npc(chest),
            facing: Direction::Up
        }));
        let wren = state.roster().get(CharId(WREN)).unwrap();
        assert_eq!(wren.skills[slot], skill, "${event:02X}");
        assert!(state.is_set(Flag::event(flag)), "${event:02X}");
        // The waits: 40 + 40 + 40 + 20 + 60 ticks around the dialogue.
        assert!(played.ticks >= 200, "{} ticks", played.ticks);
    }
}

fn dominators(members: &[u8]) -> (Played, GameState) {
    let map = big_map();
    let mut state = state_with(members);
    let played = play(
        0x91,
        &mut state,
        &map,
        cast_at(members.len(), Cell::new(46, 46)),
        &no_layout,
    );
    (played, state)
}

/// The walk targets the scene sets, in order, as `(character, cell)`.
fn go_targets(played: &Played) -> Vec<(u8, Cell)> {
    played
        .effects
        .iter()
        .filter_map(|e| match e {
            SceneEffect::ActorMoveStarted {
                actor: ActorRef::Character(CharId(who)),
                to,
            } => Some((*who, *to)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_dominators_aftermath_seats_the_party_and_the_fifth_member_goes_to_the_gap() {
    // Pixel targets divided by 16, plus the one-row standing shift.
    let chaz = Cell::new(0x2C0 / 16, 0x2E0 / 16 + 1);
    let rune = Cell::new(0x2E0 / 16, 0x2E0 / 16 + 1);
    let rika = Cell::new(0x300 / 16, 0x2E0 / 16 + 1);
    let wren = Cell::new(0x2E0 / 16, 0x2B0 / 16 + 1);
    let gap = Cell::new(0x2F0 / 16, 0x2E0 / 16 + 1);
    for fifth in [HAHN, GRYZ, DEMI, RAJA, KYRA, SETH] {
        let members = [CHAZ, RUNE, RIKA, WREN, fifth];
        let (played, state) = dominators(&members);
        assert_eq!(played.faults(), vec![], "fifth {fifth}");
        assert_eq!(
            go_targets(&played)[..5],
            [
                (CHAZ, chaz),
                (RUNE, rune),
                (RIKA, rika),
                (WREN, wren),
                (fifth, gap)
            ],
            "fifth {fifth}"
        );
        // The dialogue runs to its yield, then resumes after the palette.
        assert_eq!(played.dialogues(), vec![5]);
        assert!(played.effects.contains(&SceneEffect::DialogueResume));
        assert!(state.is_set(Flag::event(0xBB)), "DaughterShutDown");
        // When the first window opens, after the 120-frame walk, each
        // member stands where it was sent: the ring around the terminal.
        let ring = &played.at_dialogue[0];
        for (slot, cell) in [chaz, rune, rika, wren, gap].into_iter().enumerate() {
            let seat = ring
                .iter()
                .find(|(actor, _)| *actor == ActorRef::PartyMember(slot))
                .unwrap();
            assert_eq!(seat.1, cell, "fifth {fifth} slot {slot}");
        }
        // The leader then walks to ($2E0,$2F0), the last thing it moves.
        assert_eq!(
            played.leader().cell,
            Cell::new(0x2E0 / 16, 0x2F0 / 16 + 1),
            "fifth {fifth}"
        );
    }
}

#[test]
fn the_dominators_aftermath_with_four_members_never_names_a_fifth() {
    let (played, _) = dominators(&[CHAZ, RUNE, RIKA, WREN]);
    assert_eq!(played.faults(), vec![]);
    assert_eq!(
        go_targets(&played).len(),
        4,
        "no fifth member, no fifth walk"
    );
}
