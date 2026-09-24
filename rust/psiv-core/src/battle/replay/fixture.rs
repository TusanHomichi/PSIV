//! The fixture's shape, as `oracle/battle_fixture.py` writes it.
//!
//! The schema is additive: everything the forced captures added - an action's
//! `kind` and `ability`, the `vehicle` section, an outcome's `defeat` and
//! `dead_party_ids` - is optional, so a fixture extracted before them still
//! reads, and `format_version` stays 1. Those older fields keep their meaning:
//! an action with no `kind` is a physical attack (`Kind::Attack`).

use serde::Deserialize;

/// The one fixture version this harness reads.
pub(crate) const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
pub(crate) struct Fixture {
    pub(crate) format_version: u32,
    pub(crate) provenance: Provenance,
    pub(crate) formation: Formation,
    pub(crate) party: Vec<PartyEntry>,
    /// A vehicle battle's party side (`loc_78EE`, `ps4.asm:11408`).
    #[serde(default)]
    pub(crate) vehicle: Option<VehicleSection>,
    pub(crate) rolls: RollTable,
    pub(crate) outside_rolls: RollTable,
    pub(crate) rounds: Vec<Round>,
    pub(crate) outcome: OutcomeEntry,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Provenance {
    pub(crate) trace_sha256: String,
    /// The frame the opening draw landed on, or `null` for a battle that drew
    /// none - a vehicle battle rolls no `loc_B62A`.
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) priority_frame: Option<u32>,
    pub(crate) roll_count: u32,
    pub(crate) battle_roll_count: u32,
    pub(crate) roll_column: RollColumn,
    #[allow(dead_code)]
    pub(crate) undetermined: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RollColumn {
    pub(crate) agrees: u32,
    pub(crate) subtracts_low_word: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Formation {
    pub(crate) ambush_chance: u8,
    pub(crate) run_chance: u8,
    pub(crate) drop_rate: u8,
    pub(crate) priority: u16,
    pub(crate) enemies: Vec<FormationEnemyEntry>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FormationEnemyEntry {
    pub(crate) formation_slot: u8,
    pub(crate) id: u8,
    pub(crate) enemy_id: u16,
    pub(crate) hp: u16,
    pub(crate) status: u8,
    pub(crate) strength: u16,
    pub(crate) mental: u16,
    pub(crate) agility: u16,
    pub(crate) dexterity: u16,
    pub(crate) attack: u16,
    pub(crate) defence: u16,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PartyEntry {
    pub(crate) id: u8,
    pub(crate) name: String,
    pub(crate) level: u16,
    pub(crate) hp: u16,
    pub(crate) max_hp: u16,
    pub(crate) tp: u16,
    pub(crate) max_tp: u16,
    pub(crate) status: u8,
    pub(crate) strength: u16,
    pub(crate) mental: u16,
    pub(crate) agility: u16,
    pub(crate) dexterity: u16,
    pub(crate) attack: u16,
    pub(crate) defence: u16,
}

/// A vehicle battle's party side: the one fighter `loc_78EE` (`ps4.asm:11408`)
/// seats from `Vehicle_Stats`, and the saved record the log shows behind it.
#[derive(Debug, Deserialize)]
pub(crate) struct VehicleSection {
    /// `Vehicle_Index` (`$FFFFF43C`): 1 Land Rover, 2 Ice Digger,
    /// 3 Hydrofoil (`constants:2385`).
    pub(crate) index: u16,
    /// The party-side slot the vehicle fights from.
    pub(crate) fighter_id: u8,
    /// `Vehicle_Stats + curr_hp` (`$FFFF470E`) at the battle's first frame.
    pub(crate) hp: i32,
    /// The battle copy's maximum: `null` unless the saved record's own HP
    /// equalled the fighter's at the battle's first frame, which is what makes
    /// the saved record the copy's source rather than a coincidence.
    #[serde(default)]
    pub(crate) max_hp: Option<u16>,
    pub(crate) saved_record: SavedRecord,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SavedRecord {
    #[serde(default)]
    pub(crate) hp: Option<i32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) max_hp: Option<u16>,
    #[serde(default)]
    pub(crate) skill_mask: Option<u8>,
    #[serde(default)]
    pub(crate) skill1_current: Option<u8>,
    #[serde(default)]
    pub(crate) skill1_max: Option<u8>,
}

/// The fixture's roll table: one row per call, and the column names it was
/// written with so a reader cannot mix them up silently.
#[derive(Debug, Deserialize)]
pub(crate) struct RollTable {
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<RollRow>,
}

/// `[frame, roll, role, target, pass, round, action]`. A tuple rather than a
/// map: the table is 134 rows, and `json.dump` puts every array element on its
/// own line.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RollRow(
    pub(crate) u32,
    pub(crate) u16,
    pub(crate) String,
    pub(crate) Option<u8>,
    pub(crate) u8,
    pub(crate) u16,
    pub(crate) u8,
);

#[derive(Debug, Clone)]
pub(crate) struct Roll {
    pub(crate) frame: u32,
    pub(crate) roll: u16,
    pub(crate) role: String,
    pub(crate) target: Option<u8>,
    pub(crate) pass_number: u8,
    pub(crate) round: u16,
    pub(crate) action: u8,
}

pub(crate) const ROLL_COLUMNS: [&str; 7] =
    ["frame", "roll", "role", "target", "pass", "round", "action"];

impl RollTable {
    pub(crate) fn rolls(&self) -> Vec<Roll> {
        assert_eq!(self.columns, ROLL_COLUMNS, "the roll table's columns");
        self.rows
            .iter()
            .map(|row| Roll {
                frame: row.0,
                roll: row.1,
                role: row.2.clone(),
                target: row.3,
                pass_number: row.4,
                round: row.5,
                action: row.6,
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Round {
    pub(crate) round: u16,
    #[allow(dead_code)]
    pub(crate) order_frame: u32,
    pub(crate) order: Vec<u8>,
    #[allow(dead_code)]
    pub(crate) ordering: Vec<u16>,
    pub(crate) roll_count: u32,
    pub(crate) actions: Vec<Action>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    /// A physical attack - and what every action of the older fixtures is, the
    /// `kind` field being one the forced captures added.
    #[default]
    Attack,
    /// An enemy's ability, resolved on the slots the log shows it moving.
    Ability,
    /// An enemy's ability that spent the turn without an effect.
    Wasted,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Action {
    pub(crate) actor: u8,
    pub(crate) start_frame: u32,
    pub(crate) end_frame: u32,
    #[allow(dead_code)]
    pub(crate) hit_frame: Option<u32>,
    pub(crate) roll_count: u32,
    #[serde(default)]
    pub(crate) kind: Kind,
    /// The ability id the log's `eN_ability` byte held for this action.
    #[serde(default)]
    pub(crate) ability: Option<u8>,
    pub(crate) targets: Vec<Target>,
    /// The party-side fighters alive when the action opened; the comparator
    /// reads the targets it resolves, so this is the fixture's own record of
    /// what the log had to aim at.
    #[allow(dead_code)]
    pub(crate) living_opponents: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Target {
    pub(crate) id: u8,
    pub(crate) hit: String,
    pub(crate) damage: Option<u16>,
    #[allow(dead_code)]
    pub(crate) damage_frame: Option<u32>,
    pub(crate) hp_after: i32,
    pub(crate) died: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OutcomeEntry {
    pub(crate) victory: bool,
    #[serde(default)]
    pub(crate) defeat: bool,
    pub(crate) dead_enemy_ids: Vec<u8>,
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) dead_party_ids: Vec<u8>,
    pub(crate) experience_total: u16,
    pub(crate) meseta: u16,
}

/// A fixture, parsed and checked for the version this harness reads.
pub(crate) fn fixture(json: &str) -> Fixture {
    let parsed: Fixture =
        serde_json::from_str(json).expect("the fixture is the extractor's output");
    assert_eq!(
        parsed.format_version, FORMAT_VERSION,
        "a fixture version this test reads"
    );
    parsed
}
