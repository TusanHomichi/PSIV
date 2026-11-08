//! `battle/art/backgrounds.json`'s selection block: the three tables
//! `Battle_SetupBackground` (`ps4.asm:10124`) chooses a battle background
//! from, and the rule that walks them (`psiv_tools/battle_art_pack.py`).
//!
//! The rule lives here, over the pack's tables, so the runtime that knows the
//! map, the chunk under the leader and the event flags can answer which
//! background a battle draws, and the presentation layer only draws it.

use std::path::Path;

use serde::Deserialize;

use crate::DataError;
use crate::manifest::PACK_FORMAT_VERSION;

/// The file, relative to the pack.
pub const BATTLE_BACKGROUNDS_PATH: &str = "battle/art/backgrounds.json";

/// `MotaBattleBGIndexes`' entries, from `$0586EC` to
/// `FieldRoutine_Interaction` at `$0587C2`. The disassembly lists 42 under the
/// label; the rest follow under two branch-target labels (`loc_58716`,
/// `loc_5874C`). `GetMotaBattleBGIndex` (`ps4.asm:118165`) indexes the table
/// with the whole chunk byte and no bound, and Motavia's overworld uses chunk
/// ids up to 210 (#40). A pack with fewer entries was built by an older
/// extractor and is refused.
pub const MOTAVIA_TERRAIN_ENTRIES: usize = 214;

/// The selection tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleBackgrounds {
    /// `EventBattleBGIndexes`, by `Event_Battle_Index`.
    pub event_battle: Vec<u8>,
    /// `Battle_BackgroundIndexes`, by `Field_Map_Index`.
    pub field_map: Vec<u8>,
    /// What [`Self::field_map`] stores for a map that picks none.
    pub field_none: u8,
    /// `MotaBattleBGIndexes`, by Motavia chunk id, storing background + 1.
    pub motavia_terrain: Vec<u8>,
    /// `loc_6C24`: this background becomes [`Self::dark_force_to`] once
    /// `EventFlag_DarkForce2` is set.
    pub dark_force_from: u8,
    /// See [`Self::dark_force_from`].
    pub dark_force_to: u8,
}

#[derive(Deserialize)]
struct File {
    format_version: u32,
    selection: Selection,
}

#[derive(Deserialize)]
struct Selection {
    event_battle: Table,
    field_map: FieldTable,
    motavia_terrain: Table,
    dark_force_2_swap: Swap,
}

#[derive(Deserialize)]
struct Table {
    indexes: Vec<u8>,
}

#[derive(Deserialize)]
struct FieldTable {
    indexes: Vec<u8>,
    none: u8,
}

#[derive(Deserialize)]
struct Swap {
    from: u8,
    to: u8,
}

impl BattleBackgrounds {
    /// Reads `battle/art/backgrounds.json` under `pack_dir`; `Ok(None)` when
    /// the pack has no battle art at all.
    ///
    /// # Errors
    /// [`DataError::ManifestMismatch`] for a Motavia table that is not
    /// [`MOTAVIA_TERRAIN_ENTRIES`] long (an older extractor's: rebuild the
    /// pack); [`DataError::FormatVersion`] for another pack format; I/O and
    /// JSON errors name the file.
    pub fn load(pack_dir: &Path) -> Result<Option<BattleBackgrounds>, DataError> {
        let path = pack_dir.join(BATTLE_BACKGROUNDS_PATH);
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
        let file: File = serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;
        if file.format_version != PACK_FORMAT_VERSION {
            return Err(DataError::FormatVersion {
                found: file.format_version,
                expected: PACK_FORMAT_VERSION,
            });
        }
        let selection = file.selection;
        if selection.motavia_terrain.indexes.len() != MOTAVIA_TERRAIN_ENTRIES {
            return Err(DataError::ManifestMismatch {
                path,
                field: "motavia_terrain",
                manifest: format!("{MOTAVIA_TERRAIN_ENTRIES} entries; rebuild the pack"),
                record: format!("{} entries", selection.motavia_terrain.indexes.len()),
            });
        }
        Ok(Some(BattleBackgrounds {
            event_battle: selection.event_battle.indexes,
            field_map: selection.field_map.indexes,
            field_none: selection.field_map.none,
            motavia_terrain: selection.motavia_terrain.indexes,
            dark_force_from: selection.dark_force_2_swap.from,
            dark_force_to: selection.dark_force_2_swap.to,
        }))
    }

    /// `GetMotaBattleBGIndex` (`ps4.asm:118165`): the zero-based
    /// `Mota_Battle_BG_Index` for a chunk id. The table stores background + 1
    /// and keeps 0 for background 0. `None` for a chunk past the table, which
    /// no Motavia chunk is.
    #[must_use]
    pub fn motavia(&self, chunk: u8) -> Option<u8> {
        self.motavia_terrain
            .get(usize::from(chunk))
            .map(|stored| stored.saturating_sub(1))
    }

    /// `Battle_SetupBackground`'s choice, before any art is loaded.
    ///
    /// In order: `Event_Battle_Index` when one is set; on Motavia's map 0,
    /// `Mota_Battle_BG_Index` from the chunk under the leader
    /// (`FieldRoutine_Battle`, `ps4.asm:117936-117944`); otherwise the map's
    /// own byte. Then `loc_6C24` swaps background 4 for 5 once Dark Force 2's
    /// flag is set.
    ///
    /// One case is the port's, not the cartridge's: a mounted battle on a map
    /// whose byte is [`Self::field_none`] reads the Motavia table by its chunk
    /// too, and a chunk past the table takes background 0. Only debug
    /// fixtures mount a vehicle there.
    #[must_use]
    pub fn select(
        &self,
        event_battle: Option<u16>,
        map_id: u16,
        chunk: Option<u8>,
        dark_force_2: bool,
        vehicle_mounted: bool,
    ) -> Option<u8> {
        let field = self.field_map.get(usize::from(map_id)).copied();
        let selected = if let Some(event) = event_battle {
            self.event_battle.get(usize::from(event)).copied()?
        } else if map_id == 0 {
            self.motavia(chunk?)?
        } else if vehicle_mounted
            && chunk.is_some()
            && field.is_none_or(|index| index == self.field_none)
        {
            self.motavia(chunk?).unwrap_or(0)
        } else {
            let index = field?;
            if index == self.field_none {
                return None;
            }
            index
        };
        Some(if dark_force_2 && selected == self.dark_force_from {
            self.dark_force_to
        } else {
            selected
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> BattleBackgrounds {
        BattleBackgrounds {
            event_battle: vec![13, 4],
            field_map: vec![u8::MAX, 4, 21],
            field_none: u8::MAX,
            motavia_terrain: vec![0, 1, 4],
            dark_force_from: 4,
            dark_force_to: 5,
        }
    }

    #[test]
    fn selection_uses_the_event_map_and_terrain_tables_in_order() {
        let tables = tables();
        assert_eq!(tables.select(Some(0), 2, None, false, false), Some(13));
        assert_eq!(tables.select(None, 1, None, false, false), Some(4));
        assert_eq!(tables.select(None, 0, Some(2), false, false), Some(3));
        // Stored 0 and stored 1 both mean background 0.
        assert_eq!(tables.select(None, 0, Some(0), false, false), Some(0));
        assert_eq!(tables.select(None, 0, Some(1), false, false), Some(0));
        assert_eq!(tables.select(None, 2, None, false, false), Some(21));
        assert_eq!(tables.select(None, 2, None, true, false), Some(21));
    }

    #[test]
    fn map_zero_without_a_chunk_or_past_the_table_has_no_background() {
        let tables = tables();
        assert_eq!(tables.select(None, 0, None, false, false), None);
        assert_eq!(tables.select(None, 0, Some(3), false, false), None);
    }

    #[test]
    fn the_dark_force_swap_applies_after_selection() {
        let tables = tables();
        assert_eq!(tables.select(Some(1), 0, None, true, false), Some(5));
        assert_eq!(tables.select(Some(1), 0, None, false, false), Some(4));
        assert_eq!(tables.select(None, 99, None, false, false), None);
    }

    #[test]
    fn mounted_debug_maps_fall_back_to_the_motavia_table_only_when_unmapped() {
        let tables = tables();
        assert_eq!(tables.select(None, 99, Some(2), false, true), Some(3));
        assert_eq!(tables.select(None, 99, Some(0x5F), false, true), Some(0));
        assert_eq!(tables.select(None, 2, Some(2), false, true), Some(21));
    }
}
