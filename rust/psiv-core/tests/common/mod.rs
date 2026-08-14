//! Test fixtures: synthetic maps drawn as ASCII so the specification tests
//! read like maps instead of like index arithmetic.

// Each integration test binary links this module separately, so any helper a
// given binary does not call reads as dead code there. This is the standard
// shape for shared test support, not an unused-code smell.
#![allow(dead_code)]

use psiv_core::{
    Cell, CollisionGrid, Direction, Effect, FieldMap, FieldState, Input, MapId, Npc, NpcId,
    StepFrames, Warp,
};

/// ASCII legend for the 4-bit collision types.
///
/// ```text
/// .  0x0 normal        D  0x1 map change     +  0x2 recovery
/// #  0x8 solid         ~  0x9 water          s  0xA sand
/// i  0xB ice           $  0xC shop           3..7,d,e,f  unnamed values
/// ```
pub fn collision_value(symbol: char) -> u8 {
    match symbol {
        '.' => 0x0,
        'D' => 0x1,
        '+' => 0x2,
        '#' => 0x8,
        '~' => 0x9,
        's' => 0xA,
        'i' => 0xB,
        '$' => 0xC,
        '3'..='7' => symbol as u8 - b'0',
        'd' => 0xD,
        'e' => 0xE,
        'f' => 0xF,
        other => panic!("unknown collision symbol {other:?}"),
    }
}

/// Builds a grid from ASCII rows. All rows must be the same length.
pub fn grid(rows: &[&str]) -> CollisionGrid {
    let height = u16::try_from(rows.len()).expect("test grid height");
    let width = u16::try_from(rows[0].chars().count()).expect("test grid width");
    let mut cells = Vec::with_capacity(usize::from(width) * usize::from(height));
    for row in rows {
        assert_eq!(
            row.chars().count(),
            usize::from(width),
            "ragged test grid row {row:?}"
        );
        cells.extend(row.chars().map(collision_value));
    }
    CollisionGrid::new(width, height, cells).expect("valid test grid")
}

/// A map with no warps and no NPCs.
pub fn map(rows: &[&str]) -> FieldMap {
    FieldMap::new(MapId(0), grid(rows), vec![], vec![]).expect("valid test map")
}

/// A map with warps and NPCs.
pub fn map_with(rows: &[&str], warps: Vec<Warp>, npcs: Vec<Npc>) -> FieldMap {
    FieldMap::new(MapId(0), grid(rows), warps, npcs).expect("valid test map")
}

/// An NPC facing down, for tests that only care that it blocks.
pub fn npc(id: u16, x: u16, y: u16) -> Npc {
    Npc::new(NpcId(id), Cell::new(x, y), Direction::Down)
}

/// A party standing at `(x, y)` facing down, stepping at the default rate.
pub fn party(map: &FieldMap, x: u16, y: u16) -> FieldState {
    FieldState::new(map, Cell::new(x, y), Direction::Down, StepFrames::default())
        .expect("valid party placement")
}

/// A party standing at `(x, y)` facing down with a custom step duration.
pub fn party_at_rate(map: &FieldMap, x: u16, y: u16, frames: u8) -> FieldState {
    FieldState::new(
        map,
        Cell::new(x, y),
        Direction::Down,
        StepFrames::new(frames).expect("non-zero step duration"),
    )
    .expect("valid party placement")
}

/// Runs `ticks` ticks of the same input and returns every effect, in order.
pub fn run(state: &mut FieldState, map: &FieldMap, input: Input, ticks: usize) -> Vec<Effect> {
    let mut effects = Vec::new();
    for _ in 0..ticks {
        effects.extend(state.tick(map, input));
    }
    effects
}

/// Holds `dir` for exactly one full step at the default rate.
pub fn walk_one_step(state: &mut FieldState, map: &FieldMap, dir: Direction) -> Vec<Effect> {
    let frames = usize::from(state.step_frames().get());
    run(state, map, Input::Direction(dir), frames)
}
