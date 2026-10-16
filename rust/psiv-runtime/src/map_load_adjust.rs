//! The field loader's four map redirects.
//!
//! `GameMode_LoadFieldMap` calls `AdjustReshelMap`, `AdjustGumbiousMap`,
//! `AdjustZioFortNurvusMaps` and `AdjustMileMap` in that order, after the
//! destination map is chosen and before its record is read
//! (`ps4.asm:107526-107529`). Each rewrites `Field_Map_Index` (and, for two
//! of them, `Field_Map_Index_2`, the start cell and the facing) from the
//! event flags, so a story flag changes which map a warp loads. `RefreshMap`,
//! the loader a scene's `LoadMap` op takes, calls none of them
//! (`ps4.asm:121767-121840`).
//!
//! The start words are eight-pixel units; a cell is `(x / 2, y / 2 + 1)`, as
//! for a scene's `LoadMap` (`scene_runtime.rs`), and facing codes are the map
//! tables' (0 down, 4 up, 8 right, 12 left).
//!
//! The rules are a function of the flags and the load, so the runtime applies
//! them at the one place a field load picks its map and the campaign planner
//! applies the same function to a hop's arrival: the two cannot disagree.

use psiv_core::{Cell, Direction, Flag, GameState, MapId};

/// `MapID_Motavia`.
const MOTAVIA: u16 = 0x00;
/// `MapID_Dezolis`.
const DEZOLIS: u16 = 0x01;
/// `MapID_Mile`, and `MapID_MileDead` (`ps4.constants.asm:1096-1097`).
const MILE: u16 = 0x1D;
const MILE_DEAD: u16 = 0x1E;
/// `MapID_ZioFort` and `MapID_Nurvus` (`:1197`, `:1282`).
const ZIO_FORT: u16 = 0x82;
const NURVUS: u16 = 0xD7;
/// `MapID_Reshel1` (`:1400`); `Reshel1 + 1` and `+ 2` are its later states.
const RESHEL_1: u16 = 0x14D;
/// `MapID_GumbiousEntrance`, `Gumbious_B1` and `Gumbious_B2`.
const GUMBIOUS_ENTRANCE: u16 = 0x160;
const GUMBIOUS_B1: u16 = 0x163;
const GUMBIOUS_B2: u16 = 0x164;

/// `EventFlag_ZioNurvus`, `DarkForce2`, `DarkForce3Defeated` and `Reunion`
/// (`ps4.constants.asm:1552`, `:1632`, `:1646`).
const ZIO_NURVUS: u16 = 0x65;
const DARK_FORCE_2: u16 = 0x9E;
const DARK_FORCE_3_DEFEATED: u16 = 0xC6;
const REUNION: u16 = 0xDA;

/// One field load: the map, where the party stands in it, which way it faces
/// and the map it came from (`Field_Map_Index_2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldLoad {
    /// The destination map.
    pub map: MapId,
    /// The arrival cell.
    pub cell: Cell,
    /// The arrival facing.
    pub facing: Direction,
    /// `Field_Map_Index_2`: the map the party left.
    pub previous_map: u16,
}

impl FieldLoad {
    /// The load a start word pair and a facing code name.
    fn start(map: u16, (x, y): (u16, u16), facing: Direction, previous_map: u16) -> FieldLoad {
        FieldLoad {
            map: MapId(map),
            cell: Cell::new(x / 2, y / 2 + 1),
            facing,
            previous_map,
        }
    }
}

/// `GameMode_LoadFieldMap`'s four redirects applied to `load`, in the
/// cartridge's order.
#[must_use]
pub fn adjust_field_load(game: &GameState, load: FieldLoad) -> FieldLoad {
    let load = adjust_reshel(game, load);
    let load = adjust_gumbious(game, load);
    let load = adjust_zio_fort_nurvus(game, load);
    adjust_mile(game, load)
}

/// `AdjustReshelMap` (`ps4.asm:111733-111748`): `Reshel1` loads its third
/// state once Dark Force 3 is defeated and its second once Dark Force 2 is.
fn adjust_reshel(game: &GameState, mut load: FieldLoad) -> FieldLoad {
    if load.map.0 != RESHEL_1 {
        return load;
    }
    if game.is_set(Flag::event(DARK_FORCE_3_DEFEATED)) {
        load.map = MapId(RESHEL_1 + 2);
    } else if game.is_set(Flag::event(DARK_FORCE_2)) {
        load.map = MapId(RESHEL_1 + 1);
    }
    load
}

/// `AdjustGumbiousMap` (`ps4.asm:111750-111772`): once Dark Force 2 is
/// defeated the temple above ground is gone. The entrance loads `Gumbious_B2`
/// at `($3C,$44)` facing left with Dezolis as the previous map, and `B1`
/// entered from `B2` loads Dezolis at `($FA,$8A)` facing down instead.
fn adjust_gumbious(game: &GameState, mut load: FieldLoad) -> FieldLoad {
    if !game.is_set(Flag::event(DARK_FORCE_2)) {
        return load;
    }
    if load.map.0 == GUMBIOUS_ENTRANCE {
        load = FieldLoad::start(GUMBIOUS_B2, (0x3C, 0x44), Direction::Left, DEZOLIS);
    }
    if load.map.0 == GUMBIOUS_B1 && load.previous_map == GUMBIOUS_B2 {
        load = FieldLoad::start(DEZOLIS, (0xFA, 0x8A), Direction::Down, load.previous_map);
    }
    load
}

/// `AdjustZioFortNurvusMaps` (`ps4.asm:111774-111800`): once Zio is fought in
/// Nurvus, `ZioFort` entered from Motavia loads `Nurvus` at `($40,$44)`
/// facing left (previous map cleared), and `ZioFort` entered from `Nurvus`
/// loads Motavia at `($70,$BC)` facing down.
fn adjust_zio_fort_nurvus(game: &GameState, load: FieldLoad) -> FieldLoad {
    if !game.is_set(Flag::event(ZIO_NURVUS)) || load.map.0 != ZIO_FORT {
        return load;
    }
    match load.previous_map {
        MOTAVIA => FieldLoad::start(NURVUS, (0x40, 0x44), Direction::Left, 0),
        NURVUS => FieldLoad::start(MOTAVIA, (0x70, 0xBC), Direction::Down, load.previous_map),
        _ => load,
    }
}

/// `AdjustMileMap` (`ps4.asm:111801-111809`): after the Reunion `Mile` loads
/// as `MileDead`, with the warp's own cell and facing.
fn adjust_mile(game: &GameState, mut load: FieldLoad) -> FieldLoad {
    if load.map.0 == MILE && game.is_set(Flag::event(REUNION)) {
        load.map = MapId(MILE_DEAD);
    }
    load
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(map: u16, previous_map: u16) -> FieldLoad {
        FieldLoad {
            map: MapId(map),
            cell: Cell::new(7, 8),
            facing: Direction::Up,
            previous_map,
        }
    }

    fn with(flags: &[u16]) -> GameState {
        let mut game = GameState::new();
        for flag in flags {
            game.set(Flag::event(*flag)).expect("event flag");
        }
        game
    }

    #[test]
    fn untouched_without_the_flags() {
        let game = with(&[]);
        for (map, previous) in [
            (RESHEL_1, 0),
            (GUMBIOUS_ENTRANCE, DEZOLIS),
            (GUMBIOUS_B1, GUMBIOUS_B2),
            (ZIO_FORT, MOTAVIA),
            (ZIO_FORT, NURVUS),
            (MILE, 0),
        ] {
            let before = load(map, previous);
            assert_eq!(adjust_field_load(&game, before), before, "map {map:#x}");
        }
    }

    #[test]
    fn reshel_loads_the_state_the_story_has_reached() {
        let before = load(RESHEL_1, 0);
        let second = adjust_field_load(&with(&[DARK_FORCE_2]), before);
        assert_eq!(second.map, MapId(RESHEL_1 + 1));
        assert_eq!((second.cell, second.facing), (before.cell, before.facing));
        // Dark Force 3 defeated wins, and `bra.s` skips the Dark Force 2 test.
        let third = adjust_field_load(&with(&[DARK_FORCE_2, DARK_FORCE_3_DEFEATED]), before);
        assert_eq!(third.map, MapId(RESHEL_1 + 2));
        // Only Reshel1 moves.
        let other = load(RESHEL_1 + 1, 0);
        assert_eq!(adjust_field_load(&with(&[DARK_FORCE_2]), other), other);
    }

    #[test]
    fn the_temple_above_ground_is_gone_after_dark_force_2() {
        let game = with(&[DARK_FORCE_2]);
        let down = adjust_field_load(&game, load(GUMBIOUS_ENTRANCE, DEZOLIS));
        assert_eq!(
            down,
            FieldLoad {
                map: MapId(GUMBIOUS_B2),
                cell: Cell::new(30, 35),
                facing: Direction::Left,
                previous_map: DEZOLIS,
            }
        );
        let out = adjust_field_load(&game, load(GUMBIOUS_B1, GUMBIOUS_B2));
        assert_eq!(
            out,
            FieldLoad {
                map: MapId(DEZOLIS),
                cell: Cell::new(125, 70),
                facing: Direction::Down,
                previous_map: GUMBIOUS_B2,
            }
        );
        // B1 from anywhere else is left alone (the temple's own stairs).
        let from_town = load(GUMBIOUS_B1, 0x161);
        assert_eq!(adjust_field_load(&game, from_town), from_town);
    }

    #[test]
    fn zio_fort_and_nurvus_swap_once_zio_is_fought() {
        let game = with(&[ZIO_NURVUS]);
        let in_ = adjust_field_load(&game, load(ZIO_FORT, MOTAVIA));
        assert_eq!(
            (in_.map, in_.cell, in_.facing, in_.previous_map),
            (MapId(NURVUS), Cell::new(32, 35), Direction::Left, 0)
        );
        let out = adjust_field_load(&game, load(ZIO_FORT, NURVUS));
        assert_eq!(
            (out.map, out.cell, out.facing),
            (MapId(MOTAVIA), Cell::new(56, 95), Direction::Down)
        );
        let elsewhere = load(ZIO_FORT, 0x83);
        assert_eq!(adjust_field_load(&game, elsewhere), elsewhere);
    }

    #[test]
    fn mile_is_dead_after_the_reunion() {
        let before = load(MILE, 0);
        let after = adjust_field_load(&with(&[REUNION]), before);
        assert_eq!(after.map, MapId(MILE_DEAD));
        assert_eq!((after.cell, after.facing), (before.cell, before.facing));
    }
}
