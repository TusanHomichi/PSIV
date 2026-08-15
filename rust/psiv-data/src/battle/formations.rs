//! `battle/formations.json`: formation records and the encounter tables.

use serde::{Deserialize, Serialize};

/// `battle/formations.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormationsFile {
    /// The pack format that wrote this.
    pub format_version: u32,
    /// How many normal formations the file declares.
    pub formation_count: u32,
    /// How many boss formations it declares.
    pub boss_formation_count: u32,
    /// The normal formations, in id order.
    pub formations: Vec<Formation>,
    /// The boss formations, indexed by `Event_Battle_Index`.
    pub boss_formations: Vec<Formation>,
    /// The 32-entry groups an encounter roll indexes.
    #[serde(default)]
    pub encounter_groups: Option<EncounterGroups>,
    /// Which encounter source each of the 417 maps uses.
    #[serde(default)]
    pub map_bindings: Vec<MapBinding>,
    /// The two overworld position grids, 64px cells, indexed `[y][x]`.
    #[serde(default)]
    pub position_grids: Vec<PositionGrid>,
}

/// One map's row in `Battle_EnemyFormationIndexes`, plus how to read it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapBinding {
    /// The map.
    pub map_id: u16,
    /// Its `MapID_*` symbol.
    #[serde(default)]
    pub map_symbol: Option<String>,
    /// Whether the 416-byte table actually covers this id. `false` only for
    /// MapID `$1A0`, whose read runs past the end (`mode` is `outside_table`).
    pub in_table: bool,
    /// The raw table byte — `None` exactly when `in_table` is false.
    #[serde(default)]
    pub value: Option<u8>,
    /// How encounters are sourced here. `none` | `group` | `position_grid` |
    /// `outside_table` — the fourth exists so the 416-vs-417 overrun stays a
    /// visible anomaly rather than an ordinary no-encounters map.
    pub mode: String,
    /// The encounter group, when `mode` is `group`.
    #[serde(default)]
    pub group: Option<u32>,
    /// The grid name, when `mode` is `position_grid`.
    #[serde(default)]
    pub position_grid: Option<String>,
    /// Groups the grid can yield (overworlds only).
    #[serde(default)]
    pub groups_available: Option<Vec<u32>>,
    /// Vehicle-selected groups (overworlds only). Tier 3.
    #[serde(default)]
    pub vehicle_groups: Option<Vec<u32>>,
}

/// One overworld's formation-group grid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionGrid {
    /// The binding key (`motavia` / `dezolis`).
    pub name: String,
    /// The disassembly label.
    #[serde(default)]
    pub label: Option<String>,
    /// The map this grid serves.
    pub map: NamedMapRef,
    /// Where the compressed grid sits in the ROM.
    #[serde(default)]
    pub rom_offset: Option<String>,
    /// Grid width in cells.
    pub columns: u32,
    /// Grid height in cells.
    pub rows: u32,
    /// World pixels per grid cell (64: the cartridge shifts coordinates
    /// right by 6).
    pub cell_size_pixels: u32,
    /// The cartridge's own indexing note.
    #[serde(default)]
    pub indexing: Option<String>,
    /// Groups the cells actually name.
    #[serde(default)]
    pub groups_used: Vec<u32>,
    /// The group a negative cell byte falls back to.
    pub fallback_group: u32,
    /// How many cells hold the fallback byte.
    #[serde(default)]
    pub fallback_cells: Option<u32>,
    /// Rows of columns — `cells[y][x]`, exactly the cartridge's
    /// `(y >> 6) * 64 + (x >> 6)`. A value `>= 0x80` means the fallback.
    pub cells: Vec<Vec<u8>>,
}

/// A map reference as the battle files spell it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedMapRef {
    /// Map id.
    pub id: u16,
    /// The `MapID_*` symbol.
    #[serde(default)]
    pub symbol: Option<String>,
}

/// One `$FF`-terminated formation record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Formation {
    /// Its index across the four normal data blocks.
    ///
    /// Absent on a boss formation, which is reached through
    /// [`Formation::event_battle_index`] instead — the two live in different
    /// index spaces because `Battle_SetupEnemyData` reaches them by different
    /// routes (`ps4.asm:11813`).
    #[serde(default)]
    pub id: Option<u16>,
    /// `Event_Battle_Index`, on a boss formation.
    #[serde(default)]
    pub event_battle_index: Option<u16>,
    /// Byte 0, compared against the party's highest agility.
    pub ambush_chance: u8,
    /// Byte 1, the same comparison for escape.
    pub run_chance: u8,
    /// Whether byte 1 is below `$F0`.
    pub can_run: bool,
    /// Byte 2, out of 128.
    pub drop_rate: u8,
    /// Byte 3, or `None` when the formation drops nothing.
    #[serde(default)]
    pub drop_item: Option<u8>,
    /// Byte 4. Formation `$177` declares four and lists three — the
    /// cartridge's own inconsistency, which is why
    /// [`Formation::count_matches_entries`] exists.
    pub count: u8,
    /// Whether byte 4 agrees with the number of entries.
    pub count_matches_entries: bool,
    /// The enemies, in slot order.
    pub enemies: Vec<FormationEnemy>,
}

impl Formation {
    /// How this formation is identified, for a message.
    #[must_use]
    pub fn key(&self) -> String {
        match (self.id, self.event_battle_index) {
            (Some(id), _) => format!("formation {id}"),
            (None, Some(index)) => format!("boss formation {index}"),
            (None, None) => "an unidentified formation".into(),
        }
    }
}

/// One enemy slot of a formation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormationEnemy {
    /// Which of the four enemy slots, one-based.
    pub slot: u8,
    /// Index into `EnemyData`.
    pub enemy_id: u16,
    /// The on-screen position byte. Presentation only.
    pub position: u8,
}

/// `Battle_FormationIndexes`, the tables a random encounter rolls into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncounterGroups {
    /// How many groups there are.
    pub group_count: u32,
    /// How many formation ids each group holds — 32 in retail, which is why
    /// the encounter roll masks with `$1F`.
    pub entries_per_group: u32,
    /// The groups, in index order.
    pub groups: Vec<EncounterGroup>,
}

/// One encounter group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncounterGroup {
    /// Its index, as `Battle_EnemyFormationIndexes` stores it per map.
    pub group: u32,
    /// The formation ids it can roll.
    pub formation_ids: Vec<u16>,
}
