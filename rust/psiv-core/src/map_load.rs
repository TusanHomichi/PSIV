//! What a map load does to persistent state.
//!
//! Map-load routines clear selected temporary flags in the `$F140` bank.
//! Oracle tape 18 measured a destination-load clear and Xanafalgue respawn.
//! Chest flags live separately at `$F120`; the earlier claim that this clear
//! also re-armed a chest was withdrawn, as detailed below.
//!
//! # The mechanism
//!
//! `MapDataManager` (`0x051B38`, `ps4.asm:107815`) is called from
//! `GameMode_LoadFieldMap` and walks the map record's `$FFFF`-terminated list
//! of entry ids, dispatching each through `MapDataManagerJmpTbl`:
//!
//! ```text
//! MapDataManager:
//!     move.w  (a0), d0
//!     cmpi.w  #$FFFF, d0
//!     beq.s   +                       ; end of this map's list
//!     lea     (MapDataManagerJmpTbl).l, a1
//!     add.w   d0, d0
//!     add.w   d0, d0
//!     jsr     (a1,d0.w)
//!     ...
//!     lea     $2(a0), a0
//!     bra.s   MapDataManager
//! ```
//!
//! Some of those routines clear specific `$F140` flags. So the answers to the
//! three questions the measurement raised are:
//!
//! - **Which routine**: whichever `MapDataMan_*` entries the *destination*
//!   map's list names. There is no single clearing routine.
//! - **Which ids**: an explicit literal list per routine. Not a range, not a
//!   bulk pass, and not derived from anything — seven routines name twenty
//!   ids between them, and every id is written out as an immediate.
//! - **Destination-load-tied?** Yes, and the Xanafalgue case shows it plainly:
//!   `MapDataMan_NearPiataBasement` lives on map `$012`
//!   (`PiataAcademyNearBasement`), the map you *arrive* on, not on the basement
//!   you left. `MapDataManager` runs during that map's load, which is what the
//!   oracle measured as load+39.
//!
//! # The table
//!
//! | entry | routine | clears |
//! |---|---|---|
//! | `$14` | `MapDataMan_MovingPlatforms` | `$09 $0A $0D $0E $0F $10` |
//! | `$17` | `MapDataMan_Terminals` | `$0B $0C $11 $12` |
//! | `$18` | `MapDataMan_NearPiataBasement` | `$13` |
//! | `$3D` | `MapDataMan_GaruberkTowerPart4` | `$15` |
//! | `$3E` | `MapDataMan_GaruberkTowerPart5` | `$17` |
//! | `$47` | `MapDataMan_LeRoofRoom` | `$19` |
//! | `$84` | `MapDataMan_ClrBioPlantAlarm` | `$08` |
//!
//! Two clear sites are deliberately **not** here because they are not map load:
//! `FieldObj_EsperGuard` clears `$1A` while the guards animate, which belongs
//! to that object's routine; and `MapUpdate_ClrChestFlag` (`MapUpdateJmpTbl`
//! entry `$38`) clears `$A9` every frame but the disassembly annotates it "Not
//! referenced" — dead code, and chasing it would be a waste.
//!
//! # These clears are temp flags only
//!
//! Every id above is a `$F140` id, and `$F140` carries **only** temp event
//! flags. An earlier revision of this engine believed `$F140` was the chest
//! bank and concluded that these clears un-loot chests — that the Xanafalgue's
//! flag being `$13` re-armed `ChestFlag_GrbrkTwMoonSlashr`. It does not. Chest
//! flags are `$F120` bits (see `state.rs`), a different array, and nothing in
//! the cartridge ever clears one.
//!
//! What tape 18 measured stands: the clear happens, and the Xanafalgue
//! respawns. The chest consequence was an inference from the wrong bank model
//! and is withdrawn.

use crate::state::{Flag, GameState};

/// `MapDataManagerJmpTbl` entries that clear flags, and what each clears.
///
/// Transcribed from the seven routines named in the module docs. The entry id
/// is the index the map record stores, so a caller matches this against the
/// map's own `MapDataManager` list — which the pack already extracts as
/// `map_effects[].entry`.
const MAP_LOAD_FLAG_CLEARS: [(u8, &[u8]); 7] = [
    // Vahal Fort and Weapon Plant moving platforms, reset so the platforms
    // start where the map art draws them.
    (0x14, &[0x09, 0x0A, 0x0D, 0x0E, 0x0F, 0x10]),
    // The conveyor terminals in the same two dungeons.
    (0x17, &[0x0B, 0x0C, 0x11, 0x12]),
    // The Xanafalgue. Runs on the map outside the basement, not in it.
    (0x18, &[0x13]),
    (0x3D, &[0x15]),
    (0x3E, &[0x17]),
    (0x47, &[0x19]),
    (0x84, &[0x08]),
];

/// The flag ids a `MapDataManager` entry clears when its map loads.
///
/// Empty for the great majority of entries — of the 131 in the table, seven
/// clear anything.
#[must_use]
pub fn flag_clears_for_entry(entry: u8) -> &'static [u8] {
    MAP_LOAD_FLAG_CLEARS
        .iter()
        .find(|(id, _)| *id == entry)
        .map_or(&[], |(_, ids)| *ids)
}

/// Applies every map-load flag clear for a map whose `MapDataManager` list is
/// `entries`, returning the flags actually cleared.
///
/// Call this when the destination map loads, not when the source map is left.
/// The return value is for the bridge: an object gated on one of these flags
/// has to be rebuilt, which is what makes the Xanafalgue reappear.
pub fn apply_map_load(state: &mut GameState, entries: &[u8]) -> Vec<Flag> {
    let mut cleared = Vec::new();
    for entry in entries {
        for id in flag_clears_for_entry(*entry) {
            let flag = Flag::temp(u16::from(*id));
            if state.is_set(flag) {
                let _ = state.clear(flag);
                cleared.push(flag);
            }
        }
    }
    cleared
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_seven_entries_clear_anything() {
        let clearing = (0u8..=0xFF)
            .filter(|e| !flag_clears_for_entry(*e).is_empty())
            .count();
        assert_eq!(clearing, 7);
        assert!(flag_clears_for_entry(0x00).is_empty());
        assert!(flag_clears_for_entry(0x03).is_empty(), "the despawn entry");
    }

    #[test]
    fn the_map_outside_the_basement_clears_the_xanafalgue_flag() {
        // Entry $18 is MapDataMan_NearPiataBasement, which sits on map $012 —
        // the destination, not the basement.
        assert_eq!(flag_clears_for_entry(0x18), &[0x13]);
    }

    #[test]
    fn arriving_clears_the_flag_and_reports_what_it_cleared() {
        let mut state = GameState::new();
        state.set(Flag::temp(0x13)).unwrap();

        let cleared = apply_map_load(&mut state, &[0x03, 0x18]);
        assert_eq!(cleared, vec![Flag::temp(0x13)]);
        assert!(state.is_clear(Flag::temp(0x13)));
    }

    #[test]
    fn a_flag_that_was_not_set_is_not_reported_as_cleared() {
        // The bridge rebuilds on what actually changed, so an unconditional
        // clear would make every load look like a state change.
        let mut state = GameState::new();
        let cleared = apply_map_load(&mut state, &[0x18]);
        assert!(cleared.is_empty());
    }

    #[test]
    fn no_map_load_can_ever_clear_a_chest_flag() {
        // Permanence, pinned as a property rather than described in a comment.
        //
        // Nothing in the cartridge clears a `$F120` bit: the clear door at
        // `0x0576B2` has one caller, `MapUpdate_ClrChestFlag`, which the
        // disassembly annotates "Not referenced" and which clears the unused id
        // `$A9`. A chest, once opened, stays open for the life of the save.
        //
        // Map load is the only thing in this engine that clears flags at all,
        // so it is the only place that could violate it. Every entry, against
        // every chest flag.
        let mut state = GameState::new();
        for id in 0..=0xFFu16 {
            state.set(Flag::chest(id)).unwrap();
        }
        let before = state.snapshot().event_flags;

        for entry in 0..=0xFFu8 {
            apply_map_load(&mut state, &[entry]);
        }

        assert_eq!(
            state.snapshot().event_flags,
            before,
            "a map load cleared a chest flag"
        );
        for id in 0..=0xFFu16 {
            assert!(state.is_set(Flag::chest(id)), "chest ${id:02X}");
        }
    }

    #[test]
    fn a_map_with_no_clearing_entries_leaves_everything_alone() {
        let mut state = GameState::new();
        state.set(Flag::temp(0x13)).unwrap();
        assert!(apply_map_load(&mut state, &[0x00, 0x03, 0x05]).is_empty());
        assert!(state.is_set(Flag::temp(0x13)));
    }

    #[test]
    fn leaving_the_bio_plant_clears_the_alarm_without_touching_the_chest() {
        // Entry $84 clears temp $08, `TempEveFlag_BioPlantAlarm`. Chest $08 is
        // `ChestFlag_Alshline` and lives in a different array; an earlier
        // revision had these as one bit and concluded the alarm re-armed the
        // chest. It does not.
        let mut state = GameState::new();
        state.set(Flag::temp(0x08)).unwrap();
        state.set(Flag::chest(0x08)).unwrap();

        let cleared = apply_map_load(&mut state, &[0x84]);
        assert_eq!(cleared, vec![Flag::temp(0x08)]);
        assert!(state.is_clear(Flag::temp(0x08)));
        assert!(state.is_set(Flag::chest(0x08)), "the chest stays looted");
    }
}
