use super::*;
use crate::battle::SliceRolls;
use crate::collision::CollisionGrid;
use crate::map::{MapId, Npc, NpcId};

fn map_with_npc(rows: &[&str], cell: Cell) -> FieldMap {
    let height = u16::try_from(rows.len()).unwrap();
    let width = u16::try_from(rows[0].len()).unwrap();
    let cells: Vec<u8> = rows
        .iter()
        .flat_map(|row| row.chars())
        .map(|ch| if ch == '#' { 8 } else { 0 })
        .collect();
    let grid = CollisionGrid::new(width, height, cells).unwrap();
    FieldMap::new(
        MapId(0),
        grid,
        vec![],
        vec![Npc::new(NpcId(2), cell, Direction::Down)],
    )
    .unwrap()
}

/// A roll whose low three bits pick `index` and whose masked value is the
/// pause. Since one roll supplies both, they cannot be chosen apart.
const fn roll_for(index: u16) -> u16 {
    index
}

#[test]
fn the_remap_table_is_symmetric() {
    let mut counts = [0usize; 5]; // stand, up, down, left, right
    for index in 0..8u8 {
        match direction_for_command(COMMAND_FOR_INDEX[usize::from(index)]) {
            None => counts[0] += 1,
            Some(Direction::Up) => counts[1] += 1,
            Some(Direction::Down) => counts[2] += 1,
            Some(Direction::Left) => counts[3] += 1,
            Some(Direction::Right) => counts[4] += 1,
        }
    }
    assert_eq!(counts, [4, 1, 1, 1, 1], "50% stand, 12.5% each direction");
}

#[test]
fn a_wanderer_steps_and_commits_its_destination_immediately() {
    let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    // index 2 -> command $02 -> down.
    let draws = [roll_for(2)];
    let mut rolls = SliceRolls::new(&draws);

    set.tick(&mut map, &mut rolls, &[], |_| true);

    assert_eq!(
        map.npcs()[0].cell,
        Cell::new(1, 2),
        "the map moves on the frame the step starts"
    );
    assert_eq!(map.npcs()[0].facing, Direction::Down);
    assert!(set.get(0).unwrap().is_stepping());
    assert!(
        map.is_walkable(Cell::new(1, 1)),
        "and the cell it left is free at once"
    );
}

#[test]
fn a_step_takes_thirty_two_frames() {
    let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    let draws = [roll_for(2)];
    let mut rolls = SliceRolls::new(&draws);

    set.tick(&mut map, &mut rolls, &[], |_| true);
    for frame in 1..WANDER_STEP_FRAMES {
        assert!(set.get(0).unwrap().is_stepping(), "frame {frame}");
        set.tick(&mut map, &mut rolls, &[], |_| true);
    }
    assert!(!set.get(0).unwrap().is_stepping(), "lands on frame 32");
}

#[test]
fn one_roll_supplies_both_the_pause_and_the_direction() {
    let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    // $32: low three bits = 2 (down), masked with $3F = 50 (the pause).
    let draws = [0x32u16];
    let mut rolls = SliceRolls::new(&draws);

    set.tick(&mut map, &mut rolls, &[], |_| true);
    assert_eq!(
        map.npcs()[0].cell,
        Cell::new(1, 2),
        "direction from bits 0-2"
    );
    assert_eq!(set.get(0).unwrap().timer(), 0x32, "pause from bits 0-5");
    assert_eq!(rolls.drawn(), 1, "one draw, not two");
}

#[test]
fn type_three_pauses_about_twice_as_long() {
    assert_eq!(WanderKind::Type2.pause_mask(), 0x3F);
    assert_eq!(WanderKind::Type3.pause_mask(), 0x7F);

    let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
    let mut two = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    let mut three = WanderSet::build(&map, &[(0, WanderKind::Type3)]).unwrap();
    let draws = [0x7Au16];
    let mut a = SliceRolls::new(&draws);
    let mut b = SliceRolls::new(&draws);

    let mut copy = map.clone();
    two.tick(&mut copy, &mut a, &[], |_| true);
    three.tick(&mut map, &mut b, &[], |_| true);

    assert_eq!(two.get(0).unwrap().timer(), 0x3A, "$7A & $3F");
    assert_eq!(three.get(0).unwrap().timer(), 0x7A, "$7A & $7F");
}

#[test]
fn an_off_screen_wanderer_is_frozen_and_draws_no_rolls() {
    let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    let draws = [roll_for(2)];
    let mut rolls = SliceRolls::new(&draws);

    for _ in 0..100 {
        set.tick(&mut map, &mut rolls, &[], |_| false);
    }

    assert_eq!(map.npcs()[0].cell, Cell::new(1, 1), "never moved");
    assert_eq!(set.get(0).unwrap().timer(), 0, "never counted down");
    assert_eq!(rolls.drawn(), 0, "and never consumed the shared generator");
}

#[test]
fn the_leash_keeps_a_wanderer_within_two_cells() {
    let rows = ["..........", "..........", "..........", ".........."];
    let mut map = map_with_npc(&rows, Cell::new(5, 1));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    // Always roll "left", with a zero pause so it rolls every other frame.
    let draws = [0x08u16 | 3]; // low bits 3 -> command $04 -> left
    let mut rolls = SliceRolls::new(&draws);

    for _ in 0..400 {
        set.tick(&mut map, &mut rolls, &[], |_| true);
    }

    assert_eq!(
        map.npcs()[0].cell,
        Cell::new(3, 1),
        "two cells left of spawn and no further"
    );
    assert_eq!(set.get(0).unwrap().leash().x, 0, "the leash is spent");
}

#[test]
fn terrain_refuses_a_move_but_still_turns_the_object() {
    let mut map = map_with_npc(&["....", ".#..", "...."], Cell::new(1, 2));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    // index 1 -> command $01 -> up, into the wall at (1, 1).
    let draws = [roll_for(1)];
    let mut rolls = SliceRolls::new(&draws);

    set.tick(&mut map, &mut rolls, &[], |_| true);

    assert_eq!(map.npcs()[0].cell, Cell::new(1, 2), "did not move");
    assert_eq!(
        map.npcs()[0].facing,
        Direction::Up,
        "but turned on the spot"
    );
    assert!(!set.get(0).unwrap().is_stepping());
}

#[test]
fn a_terrain_refusal_still_spends_the_leash() {
    // The cartridge commits the boundary before checking terrain, so the
    // box drifts. Reproduced deliberately.
    let mut map = map_with_npc(&["....", ".#..", "...."], Cell::new(1, 2));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    let draws = [roll_for(1)];
    let mut rolls = SliceRolls::new(&draws);

    assert_eq!(set.get(0).unwrap().leash().y, 2);
    set.tick(&mut map, &mut rolls, &[], |_| true);
    assert_eq!(
        set.get(0).unwrap().leash().y,
        1,
        "the refused move consumed leash budget anyway"
    );
}

#[test]
fn a_wanderer_will_not_walk_into_the_party() {
    let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    let draws = [roll_for(2)];
    let mut rolls = SliceRolls::new(&draws);

    set.tick(&mut map, &mut rolls, &[Cell::new(1, 2)], |_| true);

    assert_eq!(map.npcs()[0].cell, Cell::new(1, 1), "the party blocks it");
    assert_eq!(map.npcs()[0].facing, Direction::Down, "it still turns");
}

#[test]
fn two_wanderers_do_not_share_a_cell() {
    let grid = CollisionGrid::filled(6, 4, 0).unwrap();
    let mut map = FieldMap::new(
        MapId(0),
        grid,
        vec![],
        vec![
            Npc::new(NpcId(2), Cell::new(1, 1), Direction::Down),
            Npc::new(NpcId(2), Cell::new(1, 2), Direction::Down),
        ],
    )
    .unwrap();
    let mut set =
        WanderSet::build(&map, &[(0, WanderKind::Type2), (1, WanderKind::Type2)]).unwrap();
    // Both roll "down"; the first moves into (1,2) only if it is free, and
    // it is not until the second moves out of it.
    let draws = [roll_for(2)];
    let mut rolls = SliceRolls::new(&draws);

    set.tick(&mut map, &mut rolls, &[], |_| true);

    let cells: Vec<Cell> = map.npcs().iter().map(|n| n.cell).collect();
    assert_eq!(cells[0], Cell::new(1, 1), "blocked by the one below it");
    assert_eq!(cells[1], Cell::new(1, 3), "which moved on its own tick");
}

#[test]
fn a_bit_clear_object_is_not_occupancy_for_a_wanderer_either() {
    let grid = CollisionGrid::filled(6, 4, 0).unwrap();
    let mut map = FieldMap::new(
        MapId(0),
        grid,
        vec![],
        vec![
            Npc::new(NpcId(2), Cell::new(1, 1), Direction::Down),
            Npc::new(NpcId(0x50), Cell::new(1, 2), Direction::Down).with_interactable(false),
        ],
    )
    .unwrap();
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    let draws = [roll_for(2)];
    let mut rolls = SliceRolls::new(&draws);

    set.tick(&mut map, &mut rolls, &[], |_| true);

    assert_eq!(
        map.npcs()[0].cell,
        Cell::new(1, 2),
        "it walks onto the bit-clear object, as the party would"
    );
}

#[test]
fn the_same_seed_gives_the_same_positions() {
    let rows = ["........", "........", "........", "........"];
    let replay = || {
        let mut map = map_with_npc(&rows, Cell::new(4, 2));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        let mut rolls = crate::battle::Lcg41::new(0x1234_5678);
        let mut trail = Vec::new();
        for _ in 0..2_000 {
            set.tick(&mut map, &mut rolls, &[], |_| true);
            trail.push(map.npcs()[0].cell);
        }
        (trail, set)
    };

    let (first, set_a) = replay();
    let (second, set_b) = replay();
    assert_eq!(first, second, "same seed, same walk");
    assert_eq!(set_a, set_b);
    assert!(
        first.windows(2).any(|pair| pair[0] != pair[1]),
        "and it actually wandered"
    );

    // Never outside the 5x5 box around the spawn.
    for cell in &first {
        assert!(
            cell.x.abs_diff(4) <= 2 && cell.y.abs_diff(2) <= 2,
            "strayed to ({}, {})",
            cell.x,
            cell.y
        );
    }
}

#[test]
fn the_step_trace_matches_the_oracles_object_columns() {
    // Pinned against `oracle/logs/02_walk_timing.csv` slot o00, frames
    // 6962-6994: timer reads 0, the next frame rolls and reloads to 10
    // (and 10 & 7 = 2 = down, which is how the remap table gets confirmed
    // from hardware), ydur runs $0F80 down to 0 in $80 steps over exactly
    // 32 frames, and y advances 240 -> 256 half a pixel at a time.
    let mut map = map_with_npc(&["....", "....", "....", "....", "...."], Cell::new(1, 1));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    let draws = [10u16];
    let mut rolls = SliceRolls::new(&draws);

    set.tick(&mut map, &mut rolls, &[], |_| true);
    assert_eq!(
        set.get(0).unwrap().timer(),
        10,
        "the reload the oracle logs"
    );
    assert_eq!(map.npcs()[0].facing, Direction::Down, "10 & 7 = 2 = down");

    let origin = crate::trigger::PixelPos::from_cell(Cell::new(1, 1));
    let mut durations = Vec::new();
    let mut pixels = Vec::new();
    // Oracle frames 6963..6993: the 31 frames with distance still to run.
    for _ in 0..(WANDER_STEP_FRAMES - 1) {
        let w = set.get(0).unwrap();
        durations.push(w.step_durations().1);
        pixels.push(origin.y + w.travelled_px().1);
        set.tick(&mut map, &mut rolls, &[], |_| true);
    }

    assert_eq!(
        &durations[..4],
        &[0x0F80, 0x0F00, 0x0E80, 0x0E00],
        "$80 a frame, the first sample already decremented"
    );
    assert_eq!(
        durations.last(),
        Some(&0x0080),
        "the last frame with distance left"
    );
    // Oracle frames 6963-6967 read y = 240, 241, 241, 242, 242.
    assert_eq!(
        &pixels[..5],
        &[
            origin.y,
            origin.y + 1,
            origin.y + 1,
            origin.y + 2,
            origin.y + 2
        ],
        "half a pixel a frame, truncated"
    );

    // The last of those ticks is frame 6994, arrival: the duration reads
    // zero and the timer is still the reload, because the arriving frame
    // does no timer work.
    assert_eq!(set.get(0).unwrap().step_durations(), (0, 0), "arrived");
    assert_eq!(map.npcs()[0].cell, Cell::new(1, 2), "one cell down");
    assert_eq!(
        set.get(0).unwrap().timer(),
        10,
        "untouched for the whole step"
    );

    // Frame 6995: the first frame to decrement it again.
    set.tick(&mut map, &mut rolls, &[], |_| true);
    assert_eq!(set.get(0).unwrap().timer(), 9);
}

#[test]
fn the_pixel_offset_floors_rather_than_truncating_toward_zero() {
    // Oracle frame 6939: slot 3 walking *down* with ydur $0F80 reads y at
    // its origin (112), while slot 2 walking *left* with xdur $0F80 already
    // reads x one pixel past its origin (639 from 640). Half a pixel floors
    // to 0 going down and to -1 going left.
    let mut map = map_with_npc(&["......", "......", "......"], Cell::new(2, 1));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    // index 2 -> down.
    let draws = [2u16];
    let mut rolls = SliceRolls::new(&draws);
    set.tick(&mut map, &mut rolls, &[], |_| true);
    assert_eq!(
        set.get(0).unwrap().travelled_px(),
        (0, 0),
        "down: floors to 0"
    );

    let mut map = map_with_npc(&["......", "......", "......"], Cell::new(2, 1));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    // index 3 -> command $04 -> left.
    let draws = [3u16];
    let mut rolls = SliceRolls::new(&draws);
    set.tick(&mut map, &mut rolls, &[], |_| true);
    assert_eq!(
        set.get(0).unwrap().travelled_px(),
        (-1, 0),
        "left: floors to -1"
    );
}

#[test]
fn a_restored_wanderer_carries_its_timer_leash_and_step() {
    let map = map_with_npc(&["......", "......", "......"], Cell::new(2, 1));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();

    set.restore(
        0,
        WanderState {
            timer: 23,
            leash: Leash {
                x_max: 4,
                y_max: 4,
                x: 4,
                y: 2,
            },
            step: Some((Direction::Left, 1, Cell::new(2, 1))),
        },
    )
    .unwrap();

    let w = set.get(0).unwrap();
    assert_eq!(w.timer(), 23);
    assert_eq!(w.leash().x, 4, "already at the leash limit, as slot 0 is");
    assert!(w.is_stepping());
    assert_eq!(w.step_durations(), (0x0F80, 0), "one frame into the step");
    assert_eq!(w.travelled_px(), (-1, 0));

    assert!(matches!(
        set.restore(
            9,
            WanderState {
                timer: 0,
                leash: Leash::default(),
                step: None
            }
        ),
        Err(MapError::NpcIndexOutOfRange { .. })
    ));
}

#[test]
fn a_roll_fires_on_the_frame_the_timer_reads_zero() {
    // Not on an expiry edge some frames later: the countdown reaches 0,
    // and the *next* frame rolls. A reload of 0 therefore rolls again
    // immediately, which is the consecutive-roll case.
    let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    // Roll 0: index 0 -> stand still, and a reload of 0.
    let draws = [0u16];
    let mut rolls = SliceRolls::new(&draws);

    for frame in 0..5 {
        set.tick(&mut map, &mut rolls, &[], |_| true);
        assert_eq!(
            set.get(0).unwrap().timer(),
            0,
            "frame {frame}: a zero reload keeps the timer at zero"
        );
    }
    assert_eq!(rolls.drawn(), 5, "so it rolls every single frame");
}

#[test]
fn only_registered_wanderers_consume_rolls() {
    // Slot 7 on PiataAcademy_F1 is NPCAlysPiata: the oracle sees her timer
    // hold 0 for all 1563 field frames while consuming nothing, because
    // her routine is not GetRandomMove. A set that does not register an
    // object must never draw for it.
    let grid = CollisionGrid::filled(6, 4, 0).unwrap();
    let mut map = FieldMap::new(
        MapId(0),
        grid,
        vec![],
        vec![
            Npc::new(NpcId(0x3C), Cell::new(1, 1), Direction::Down),
            Npc::new(NpcId(0x68), Cell::new(3, 1), Direction::Down),
        ],
    )
    .unwrap();
    // Only slot 0 wanders.
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
    let draws = [0x20u16];
    let mut rolls = SliceRolls::new(&draws);

    for _ in 0..200 {
        set.tick(&mut map, &mut rolls, &[], |_| true);
    }

    assert_eq!(
        map.npcs()[1].cell,
        Cell::new(3, 1),
        "the unregistered object never moved"
    );
    assert!(set.get(1).is_none(), "and has no wander state at all");
}

#[test]
fn build_rejects_an_index_that_names_no_object() {
    let map = map_with_npc(&["..", ".."], Cell::new(0, 0));
    assert!(matches!(
        WanderSet::build(&map, &[(1, WanderKind::Type2)]),
        Err(MapError::NpcIndexOutOfRange { index: 1, count: 1 })
    ));
    assert!(WanderSet::build(&map, &[(0, WanderKind::Type2)]).is_ok());
}

#[test]
fn the_three_extracted_speed_records_are_complete() {
    assert_eq!(WanderSpeed::from_selector(0).unwrap().velocity_8_8(), 0x80);
    assert_eq!(WanderSpeed::from_selector(0).unwrap().frames(), 32);
    assert_eq!(WanderSpeed::from_selector(1).unwrap().velocity_8_8(), 0x100);
    assert_eq!(WanderSpeed::from_selector(1).unwrap().frames(), 16);
    assert_eq!(WanderSpeed::from_selector(2).unwrap().velocity_8_8(), 0x200);
    assert_eq!(WanderSpeed::from_selector(2).unwrap().frames(), 8);
    assert_eq!(WanderSpeed::from_selector(3), None);

    let mut map = map_with_npc(&["........", "........", "........"], Cell::new(3, 1));
    let mut set =
        WanderSet::build_with_speeds(&map, &[(0, WanderKind::Type2, WanderSpeed::Selector2)])
            .unwrap();
    let mut rolls = SliceRolls::new(&[0x0002]);
    set.tick(&mut map, &mut rolls, &[], |_| true);
    assert_eq!(set.get(0).unwrap().step_frames(), 8);
    for _ in 1..8 {
        set.tick(&mut map, &mut rolls, &[], |_| true);
    }
    assert!(!set.get(0).unwrap().is_stepping());
}

#[test]
fn every_packed_random_family_draws_in_cartridge_order() {
    let kinds = [
        WanderKind::Type2,
        WanderKind::Type3,
        WanderKind::Type4,
        WanderKind::Type28,
        WanderKind::StoreWoman,
        WanderKind::ClinicWoman,
        WanderKind::Penguin,
        WanderKind::Butterfly,
        WanderKind::MuskCat,
        WanderKind::Xanafalgue,
    ];
    let npcs: Vec<Npc> = kinds
        .iter()
        .enumerate()
        .map(|(index, _)| {
            Npc::new(
                NpcId(index as u16),
                Cell::new(2 + index as u16, 5),
                Direction::Down,
            )
        })
        .collect();
    let mut map = FieldMap::new(
        MapId(0),
        CollisionGrid::filled(24, 12, 0).unwrap(),
        vec![],
        npcs,
    )
    .unwrap();
    let pairs: Vec<(usize, WanderKind)> = kinds.iter().copied().enumerate().collect();
    let mut set = WanderSet::build(&map, &pairs).unwrap();
    let mut rolls = SliceRolls::new(&[0x0001]);

    let tape =
        crate::replay::Tape::parse(include_str!("../../../oracle/tapes/02_walk_timing.tape"))
            .unwrap();
    assert!(tape.frame_count() > 1_000, "the oracle tape is the fixture");
    for frame in tape.frames().into_iter().take(1) {
        let _input = frame.buttons.to_input();
        set.tick(&mut map, &mut rolls, &[], |_| true);
    }
    assert_eq!(
        rolls.drawn(),
        kinds.len(),
        "one shared stream draw per object"
    );
    assert_eq!(set.get(0).unwrap().timer(), 1, "GetRandomMove mask $3F");
    assert_eq!(set.get(1).unwrap().timer(), 1, "GetRandomMove2 mask $7F");
    assert_eq!(
        set.get(2).unwrap().timer(),
        0,
        "GetRandomMove3 has no reload"
    );
    assert_eq!(
        set.get(3).unwrap().leash(),
        Leash {
            x_max: 8,
            y_max: 8,
            x: 4,
            y: 3,
        }
    );
    assert_eq!(set.get(7).unwrap().leash().x_max, 16);
    assert_eq!(set.get(9).unwrap().leash().x_max, 2);
}

#[test]
fn xanafalgue_switches_from_random_wander_to_its_escape_branch() {
    let tape = crate::replay::Tape::parse(include_str!(
        "../../../oracle/tapes/18_flag_round_trip.tape"
    ))
    .unwrap();
    assert!(
        tape.frame_count() > 1_000,
        "the Xanafalgue oracle tape is the fixture"
    );
    let mut map = FieldMap::new(
        MapId(0),
        CollisionGrid::filled(40, 12, 0).unwrap(),
        vec![],
        vec![Npc::new(NpcId(0x184), Cell::new(30, 5), Direction::Down)],
    )
    .unwrap();
    let mut set = WanderSet::build(&map, &[(0, WanderKind::Xanafalgue)]).unwrap();
    let mut rolls = SliceRolls::new(&[0]);
    set.tick_with_driver_pixels(&mut map, &mut rolls, &[], (0, 0x101), |_| true);
    assert_eq!(rolls.drawn(), 0, "the escape branch does not draw RNG");
    let npc = map.npcs()[0];
    assert_eq!(npc.cell, Cell::new(29, 5));
    assert_eq!(npc.offset.x, 12, "$FFFC is four pixels left");
    for _ in 0..17 {
        set.tick_with_driver_pixels(&mut map, &mut rolls, &[], (0, 0x101), |_| true);
    }
    assert!(
        !map.npcs()[0].active,
        "the branch clears the object below $1A0"
    );
}
