//! Specification of the caterpillar: followers tracing the leader's path.
//!
//! The rule transcribed from `FieldObj_Move` (`ps4.asm:93415`) is that on the
//! frame a member begins a step, the member behind it is told to walk to the
//! cell it is *leaving*. Everything below is that one rule observed from the
//! outside.

mod common;

use common::{grid, map_with};
use psiv_core::{
    Cell, CellRect, Direction, Effect, FieldMap, Input, MAX_PARTY_MEMBERS, MapError, MapId,
    MemberView, Npc, NpcId, Party, StepFrames, Warp, WarpTrigger,
};

const FRAMES: u8 = 8;

fn open_map() -> FieldMap {
    map_with(
        &[
            "........", "........", "........", "........", "........", "........", "........",
            "........",
        ],
        vec![],
        vec![],
    )
}

fn party_of(map: &FieldMap, x: u16, y: u16, followers: usize) -> Party {
    Party::new(
        map,
        Cell::new(x, y),
        Direction::Down,
        StepFrames::new(FRAMES).unwrap(),
        followers,
    )
    .expect("valid party")
}

/// Holds `dir` for one whole step.
fn step(party: &mut Party, map: &FieldMap, dir: Direction) -> Vec<Effect> {
    let mut effects = Vec::new();
    for _ in 0..FRAMES {
        effects.extend(party.tick(map, Input::Direction(dir)));
    }
    effects
}

fn cells(party: &Party) -> Vec<Cell> {
    party.members().iter().map(|m| m.cell).collect()
}

// ---------------------------------------------------------------------------
// Unspooling from a stacked start
// ---------------------------------------------------------------------------

#[test]
fn a_new_party_starts_stacked_on_the_leader() {
    let map = open_map();
    let party = party_of(&map, 3, 3, 3);

    assert_eq!(party.len(), 4);
    assert_eq!(cells(&party), vec![Cell::new(3, 3); 4]);
    for member in party.members() {
        assert_eq!(member.facing, Direction::Down);
        assert!(!member.is_stepping);
        assert_eq!(member.render_offset_16ths, (0, 0));
    }
}

#[test]
fn the_party_unspools_one_member_per_step() {
    // Retail stacks the whole party on the spawn tile; the line forms as the
    // leader walks, one member joining per step.
    let map = open_map();
    let mut party = party_of(&map, 0, 0, 3);

    step(&mut party, &map, Direction::Right);
    assert_eq!(
        cells(&party),
        vec![
            Cell::new(1, 0),
            Cell::new(0, 0),
            Cell::new(0, 0),
            Cell::new(0, 0)
        ],
        "only the first follower has moved off the spawn"
    );

    step(&mut party, &map, Direction::Right);
    assert_eq!(
        cells(&party),
        vec![
            Cell::new(2, 0),
            Cell::new(1, 0),
            Cell::new(0, 0),
            Cell::new(0, 0)
        ]
    );

    step(&mut party, &map, Direction::Right);
    assert_eq!(
        cells(&party),
        vec![
            Cell::new(3, 0),
            Cell::new(2, 0),
            Cell::new(1, 0),
            Cell::new(0, 0)
        ],
        "fully unspooled: one cell apart, single file"
    );
}

// ---------------------------------------------------------------------------
// Following a path
// ---------------------------------------------------------------------------

#[test]
fn followers_walk_the_leaders_path_in_a_straight_line() {
    let map = open_map();
    let mut party = party_of(&map, 0, 4, 2);

    for _ in 0..5 {
        step(&mut party, &map, Direction::Right);
    }

    assert_eq!(
        cells(&party),
        vec![Cell::new(5, 4), Cell::new(4, 4), Cell::new(3, 4)]
    );
    for member in party.members() {
        assert_eq!(member.facing, Direction::Right);
    }
}

#[test]
fn followers_trace_the_corner_rather_than_cutting_it() {
    // The whole point of a caterpillar: the follower walks *through* the corner
    // cell the leader turned on, it does not take the diagonal shortcut.
    let map = open_map();
    let mut party = party_of(&map, 1, 1, 2);

    // Unspool along the row first.
    step(&mut party, &map, Direction::Right);
    step(&mut party, &map, Direction::Right);
    assert_eq!(
        cells(&party),
        vec![Cell::new(3, 1), Cell::new(2, 1), Cell::new(1, 1)]
    );

    // Leader turns down at (3, 1).
    step(&mut party, &map, Direction::Down);
    assert_eq!(
        cells(&party),
        vec![Cell::new(3, 2), Cell::new(3, 1), Cell::new(2, 1)],
        "the first follower steps onto the corner cell"
    );

    step(&mut party, &map, Direction::Down);
    assert_eq!(
        cells(&party),
        vec![Cell::new(3, 3), Cell::new(3, 2), Cell::new(3, 1)],
        "the second follower now occupies the corner"
    );
}

#[test]
fn a_follower_faces_the_way_it_walks_not_the_way_the_leader_faces() {
    // `FieldObj_GetAutoInput` synthesises the follower's direction from its own
    // destination, so at a corner the two disagree — which is exactly what the
    // sprite should show.
    let map = open_map();
    let mut party = party_of(&map, 1, 1, 1);

    step(&mut party, &map, Direction::Right);
    step(&mut party, &map, Direction::Down);

    let members = party.members();
    assert_eq!(members[0].facing, Direction::Down);
    assert_eq!(
        members[1].facing,
        Direction::Right,
        "the follower is still walking right into the corner"
    );
}

// ---------------------------------------------------------------------------
// Lockstep and animation
// ---------------------------------------------------------------------------

#[test]
fn the_party_steps_in_lockstep_once_unspooled() {
    let map = open_map();
    let mut party = party_of(&map, 0, 0, 2);
    step(&mut party, &map, Direction::Right);
    step(&mut party, &map, Direction::Right);

    // Mid-step, every member is walking with the same sub-cell offset.
    for tick in 1..FRAMES {
        party.tick(&map, Input::Direction(Direction::Right));
        let members = party.members();
        assert!(
            members.iter().all(|m| m.is_stepping),
            "tick {tick}: every member should be mid-step"
        );
        let offsets: Vec<_> = members.iter().map(|m| m.render_offset_16ths).collect();
        assert!(
            offsets.iter().all(|&o| o == offsets[0]),
            "tick {tick}: offsets drifted apart: {offsets:?}"
        );
    }

    party.tick(&map, Input::Direction(Direction::Right));
    assert!(party.members().iter().all(|m| !m.is_stepping));
}

#[test]
fn the_party_stops_together_when_the_input_stops() {
    let map = open_map();
    let mut party = party_of(&map, 0, 0, 2);
    for _ in 0..3 {
        step(&mut party, &map, Direction::Right);
    }
    let settled = cells(&party);

    for _ in 0..40 {
        party.tick(&map, Input::Neutral);
    }

    assert_eq!(
        cells(&party),
        settled,
        "nobody drifts after the leader stops"
    );
    assert!(party.members().iter().all(|m| !m.is_stepping));
}

#[test]
fn a_blocked_leader_moves_nobody() {
    let map = map_with(&["####", "#..#", "#..#", "####"], vec![], vec![]);
    let mut party = party_of(&map, 1, 1, 2);

    let effects = step(&mut party, &map, Direction::Up);

    assert!(effects.is_empty());
    assert_eq!(cells(&party), vec![Cell::new(1, 1); 3]);
    assert_eq!(
        party.leader().facing(),
        Direction::Up,
        "the leader still turns"
    );
}

#[test]
fn turning_on_the_spot_does_not_move_the_followers() {
    // Facing a wall is free for the leader and must not tug the line.
    let map = map_with(&["####", "#..#", "#..#", "####"], vec![], vec![]);
    let mut party = party_of(&map, 1, 1, 1);
    step(&mut party, &map, Direction::Right);
    let before = cells(&party);

    for dir in [Direction::Up, Direction::Left, Direction::Down] {
        party.tick(&map, Input::Direction(dir));
    }

    assert_eq!(cells(&party), before);
}

#[test]
fn a_one_frame_step_still_drags_the_line() {
    // With StepFrames(1) a step begins and completes inside one tick, so the
    // "did the leader begin a step" test has to catch that case too.
    let map = open_map();
    let mut party = Party::new(
        &map,
        Cell::new(0, 0),
        Direction::Down,
        StepFrames::new(1).unwrap(),
        2,
    )
    .expect("valid party");

    for _ in 0..3 {
        party.tick(&map, Input::Direction(Direction::Right));
    }

    assert_eq!(
        cells(&party),
        vec![Cell::new(3, 0), Cell::new(2, 0), Cell::new(1, 0)]
    );
}

// ---------------------------------------------------------------------------
// Map entry and warps
// ---------------------------------------------------------------------------

#[test]
fn entering_a_map_restacks_the_party_on_the_leader() {
    let map = open_map();
    let mut party = party_of(&map, 0, 0, 3);
    for _ in 0..4 {
        step(&mut party, &map, Direction::Right);
    }
    assert_ne!(cells(&party), vec![Cell::new(4, 0); 4]);

    party
        .enter_map(&map, Cell::new(6, 6), Direction::Up)
        .expect("valid placement");

    assert_eq!(cells(&party), vec![Cell::new(6, 6); 4]);
    for member in party.members() {
        assert_eq!(member.facing, Direction::Up);
        assert!(!member.is_stepping);
    }
}

#[test]
fn entering_a_map_mid_step_cancels_every_members_step() {
    let map = open_map();
    let mut party = party_of(&map, 0, 0, 2);
    step(&mut party, &map, Direction::Right);
    step(&mut party, &map, Direction::Right);
    party.tick(&map, Input::Direction(Direction::Right));
    assert!(party.members().iter().any(|m| m.is_stepping));

    party
        .enter_map(&map, Cell::new(2, 2), Direction::Left)
        .expect("valid placement");

    assert!(party.members().iter().all(|m| !m.is_stepping));
    assert!(
        party
            .members()
            .iter()
            .all(|m| m.render_offset_16ths == (0, 0))
    );
}

#[test]
fn a_warp_carries_the_whole_party_through() {
    let town = map_with(
        &[".D..", "....", "....", "...."],
        vec![Warp::door(
            Cell::new(1, 0),
            MapId(0x13),
            Cell::new(1, 2),
            Direction::Down,
        )],
        vec![],
    );
    let shop = FieldMap::new(
        MapId(0x13),
        grid(&["....", "....", "....", "...."]),
        vec![],
        vec![],
    )
    .expect("valid map");

    let mut party = Party::new(
        &town,
        Cell::new(1, 2),
        Direction::Down,
        StepFrames::new(FRAMES).unwrap(),
        2,
    )
    .expect("valid party");
    // Unspool one member before walking into the doorway at (1, 0).
    step(&mut party, &town, Direction::Up);

    let effects = step(&mut party, &town, Direction::Up);
    let Some(&Effect::Warp {
        target_map,
        target_cell,
        facing,
        ..
    }) = effects.last()
    else {
        panic!("expected a warp, got {effects:?}");
    };
    assert_eq!(target_map, shop.id());

    party
        .enter_map(&shop, target_cell, facing)
        .expect("valid placement");

    assert_eq!(party.leader().map(), MapId(0x13));
    assert_eq!(cells(&party), vec![Cell::new(1, 2); 3]);
    assert!(
        party.members().iter().all(|m| m.facing == Direction::Down),
        "everyone arrives facing the way the transition table says"
    );
}

#[test]
fn the_party_unspools_again_after_a_warp() {
    let map = open_map();
    let mut party = party_of(&map, 0, 0, 2);
    for _ in 0..3 {
        step(&mut party, &map, Direction::Right);
    }
    party
        .enter_map(&map, Cell::new(0, 5), Direction::Right)
        .expect("valid placement");

    step(&mut party, &map, Direction::Right);

    assert_eq!(
        cells(&party),
        vec![Cell::new(1, 5), Cell::new(0, 5), Cell::new(0, 5)],
        "the line re-forms from stacked, exactly as on a fresh map load"
    );
}

// ---------------------------------------------------------------------------
// Wrapping maps
// ---------------------------------------------------------------------------

#[test]
fn the_trail_follows_the_leader_across_a_seam() {
    let map = common::torus(&["....", "....", "....", "...."]);
    let mut party = party_of(&map, 1, 1, 2);
    step(&mut party, &map, Direction::Right);
    step(&mut party, &map, Direction::Right);
    assert_eq!(
        cells(&party),
        vec![Cell::new(3, 1), Cell::new(2, 1), Cell::new(1, 1)]
    );

    step(&mut party, &map, Direction::Right);

    assert_eq!(
        cells(&party),
        vec![Cell::new(0, 1), Cell::new(3, 1), Cell::new(2, 1)],
        "the leader wrapped and the line followed"
    );
    assert!(
        party.members().iter().all(|m| m.facing == Direction::Right),
        "crossing the seam is an ordinary step"
    );
}

// ---------------------------------------------------------------------------
// Interaction with the rest of the engine
// ---------------------------------------------------------------------------

#[test]
fn npcs_still_block_the_leader_with_followers_in_tow() {
    let map = map_with(
        &["........", "........", "........", "........"],
        vec![],
        vec![Npc::new(NpcId(9), Cell::new(3, 0), Direction::Down)],
    );
    let mut party = party_of(&map, 0, 0, 2);
    for _ in 0..2 {
        step(&mut party, &map, Direction::Right);
    }

    let effects = step(&mut party, &map, Direction::Right);

    assert!(effects.is_empty(), "the NPC blocks the leader");
    assert_eq!(
        cells(&party),
        vec![Cell::new(2, 0), Cell::new(1, 0), Cell::new(0, 0)],
        "and the followers hold their positions too"
    );
}

#[test]
fn warps_and_talking_are_unaffected_by_the_trail() {
    // A party tick returns exactly the leader's effects — followers add none.
    let map = map_with(
        &["....", "....", "....", "...."],
        vec![],
        vec![Npc::new(NpcId(9), Cell::new(1, 0), Direction::Down)],
    );
    let mut party = party_of(&map, 1, 1, 2);

    let effects = party.tick(&map, Input::Action);

    assert_eq!(effects.len(), 1);
    assert!(matches!(effects[0], Effect::InteractNothing { .. }));
}

// ---------------------------------------------------------------------------
// Bounds and determinism
// ---------------------------------------------------------------------------

#[test]
fn a_party_may_not_exceed_the_cartridges_five_slots() {
    let map = open_map();
    assert!(matches!(
        Party::new(
            &map,
            Cell::new(0, 0),
            Direction::Down,
            StepFrames::default(),
            MAX_PARTY_MEMBERS,
        ),
        Err(MapError::TooManyPartyMembers {
            requested: 6,
            max: 5
        })
    ));
}

#[test]
fn a_party_replays_identically() {
    fn script(seed: u32, len: usize) -> Vec<Input> {
        let mut s = seed;
        (0..len)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                match (s >> 16) % 6 {
                    0 => Input::Neutral,
                    1 => Input::Direction(Direction::Up),
                    2 => Input::Direction(Direction::Down),
                    3 => Input::Direction(Direction::Left),
                    4 => Input::Direction(Direction::Right),
                    _ => Input::Action,
                }
            })
            .collect()
    }

    #[rustfmt::skip]
    let rows = [
        "..D.....",
        ".#..~...",
        "........",
        "...##...",
        "........",
        "..#.....",
        "........",
        ".....$..",
    ];
    let warp = Warp {
        source: CellRect::new(2, 0, 1, 1),
        trigger: WarpTrigger::MapChange,
        target_map: MapId(0x13),
        target_cell: Cell::new(1, 1),
        facing: Direction::Down,
    };
    let npcs = vec![Npc::new(NpcId(1), Cell::new(6, 6), Direction::Up)];

    let replay = |inputs: &[Input]| {
        let map = map_with(&rows, vec![warp], npcs.clone());
        let mut party = party_of(&map, 2, 2, 3);
        let mut log: Vec<(usize, Effect)> = Vec::new();
        let mut trail: Vec<Vec<MemberView>> = Vec::new();
        for (tick, &input) in inputs.iter().enumerate() {
            for effect in party.tick(&map, input) {
                log.push((tick, effect));
            }
            trail.push(party.members());
        }
        (log, trail)
    };

    for seed in 0..24_u32 {
        let inputs = script(seed.wrapping_mul(2_654_435_761), 200);
        let (a_log, a_trail) = replay(&inputs);
        let (b_log, b_trail) = replay(&inputs);
        assert_eq!(a_log, b_log, "seed {seed}");
        assert_eq!(a_trail, b_trail, "seed {seed}");
    }
}

#[test]
fn followers_never_stray_further_than_one_cell_from_the_member_ahead() {
    // The invariant the whole design rests on: propagation only ever hands a
    // follower an adjacent cell, so the line never teleports or splits.
    let map = open_map();
    let mut party = party_of(&map, 4, 4, 3);
    let mut seed = 0x1234_5678_u32;

    for tick in 0..2_000 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let input = match (seed >> 16) % 5 {
            0 => Input::Neutral,
            1 => Input::Direction(Direction::Up),
            2 => Input::Direction(Direction::Down),
            3 => Input::Direction(Direction::Left),
            _ => Input::Direction(Direction::Right),
        };
        party.tick(&map, input);

        let members = party.members();
        for pair in members.windows(2) {
            let (ahead, behind) = (pair[0].cell, pair[1].cell);
            let distance = i32::from(ahead.x).abs_diff(i32::from(behind.x))
                + i32::from(ahead.y).abs_diff(i32::from(behind.y));
            assert!(
                distance <= 1,
                "tick {tick}: ({}, {}) and ({}, {}) are {distance} apart",
                ahead.x,
                ahead.y,
                behind.x,
                behind.y
            );
        }
    }
}
