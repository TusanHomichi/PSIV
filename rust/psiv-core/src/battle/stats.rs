//! A fighter's live stats, and the two routines that build them.
//!
//! The cartridge keeps three copies of each small stat — `base`, `mod` and
//! `battle` — and this struct keeps all three, because which one a routine
//! reads is load-bearing:
//!
//! - **base** (`$18`, `$1B`, `$1E`, `$21`) is what the level table wrote.
//! - **mod** (`$19`, `$1C`, `$1F`, `$22`) is base plus equipment, computed by
//!   `UpdateCharModStats` (`$0005F754`, `ps4.asm:127814`), and is what a
//!   cure or a wake-up restores `battle` *from*.
//! - **battle** (`$1A`, `$1D`, `$20`, `$23`) is the live value every formula
//!   reads, and the only one a buff or a status effect moves.
//!
//! Enemies fill this differently: `Battle_FillEnemyStats` (`ps4.asm:11939`)
//! writes the record's values into the `mod` and `battle` slots and **leaves
//! the base bytes at zero**. The oracle confirmed that in RAM — reading
//! `strength` off an enemy gets 0. [`Stats::from_enemy`] reproduces it rather
//! than tidying it up, so a bug that reads the wrong copy shows up in a test
//! instead of in a damage number.

use super::records::{
    Bonuses, CharacterRecord, ELEMENT_SLOTS, EQUIPMENT_SLOTS, EnemyRecord, ItemRecord,
};

/// Status bits at `$16` of the stats struct.
pub mod status {
    /// Bit 0.
    pub const POISONED: u8 = 1 << 0;
    /// Bit 1.
    pub const PARALYZED: u8 = 1 << 1;
    /// Bit 2.
    pub const DEAD: u8 = 1 << 2;
    /// Bit 3.
    pub const ASLEEP: u8 = 1 << 3;
    /// Bit 4.
    pub const TECH_SEALED: u8 = 1 << 4;
    /// Bit 5. A second sleep bit; the cartridge tests both.
    pub const ASLEEP_2: u8 = 1 << 5;
    /// Bit 6. Androids fall over rather than die.
    pub const ANDROID_DEAD: u8 = 1 << 6;

    /// The mask `Battle_OrderTurns`, `Battle_ProcessCOMD` and
    /// `Battle_ProcessRUN` use to decide whether a fighter gets a turn at all:
    /// `$6E` (`ps4.asm:7769`).
    ///
    /// **Tech-sealed is not in it.** A sealed character loses techniques, not
    /// their turn — they can still attack. The five bits are paralysed, dead,
    /// asleep, asleep-2 and android-dead.
    pub const NO_TURN: u8 = PARALYZED | DEAD | ASLEEP | ASLEEP_2 | ANDROID_DEAD;

    /// The mask that means "out of the fight": `$44`.
    pub const OUT: u8 = DEAD | ANDROID_DEAD;
}

/// `ProfessionID_Android`. Androids diverge from Tier 2 on.
pub const PROFESSION_ANDROID: u16 = 5;

/// The element factor a fighter gets while defending.
///
/// `Character_Defend`'s tail writes `move.b #1, $30(a0)` (`ps4.asm:6824`) —
/// physical resistance, halving incoming physical damage, until the end-of-turn
/// restore puts it back.
pub const DEFENDING_PHYSICAL_PROP: u8 = 1;

/// One fighter's stats, laid out as the cartridge's 128-byte struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stats {
    /// `$06`. `ProfessionID_*`.
    pub profession: u16,
    /// `$08`. Not a level at all for enemies.
    pub level: u16,
    /// `$0A`.
    pub experience: u32,
    /// `$0E`.
    pub curr_hp: u16,
    /// `$10`.
    pub max_hp: u16,
    /// `$12`. For enemies this holds the attack element instead.
    pub curr_tp: u16,
    /// `$14`. For enemies this holds the attack status effect instead.
    pub max_tp: u16,
    /// `$16`. See [`status`].
    pub status: u8,
    /// `$18`, `$19`, `$1A`.
    pub strength: StatTriple,
    /// `$1B`, `$1C`, `$1D`.
    pub mental: StatTriple,
    /// `$1E`, `$1F`, `$20`.
    pub agility: StatTriple,
    /// `$21`, `$22`, `$23`.
    pub dexterity: StatTriple,
    /// `$24` and `$26`.
    pub attack: StatPair,
    /// `$28` and `$2A`.
    pub defence: StatPair,
    /// `$2C` and `$2E`.
    pub mental_defence: StatPair,
    /// The high bytes at `$30`, `$32`, .. `$4A` — what the damage pipeline
    /// reads.
    pub element_props: [u8; ELEMENT_SLOTS],
    /// The low bytes at `$31`, `$33`, .. `$4B`.
    ///
    /// `Battle_RestoreStatsAtTurnEnd` copies `$31` back over `$30` every round
    /// (`ps4.asm:9806`), which is how a Defend wears off — and, for characters,
    /// how the high bytes get populated at all, since `Character_Init` writes
    /// only the low ones (`ps4.asm:88774`).
    pub element_shadow: [u8; ELEMENT_SLOTS],
    /// `$4C`..`$4F`: right hand, left hand, head, body.
    pub equipment: [u8; EQUIPMENT_SLOTS],
    /// `$68`. Zero for characters.
    pub enemy_id: u16,
    /// `$7A`. Set the first time a character survives a won battle; gates
    /// out-of-party experience.
    pub gain_exp_flag: bool,
}

/// A stat the cartridge stores three times: base, equipment-modified, live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatTriple {
    /// What the level table wrote.
    pub base: u8,
    /// Base plus equipment, from `UpdateCharModStats`.
    pub modified: u8,
    /// The live value every formula reads.
    pub battle: u8,
}

impl StatTriple {
    /// All three the same, which is where a character starts.
    #[must_use]
    pub const fn uniform(value: u8) -> StatTriple {
        StatTriple {
            base: value,
            modified: value,
            battle: value,
        }
    }

    /// `modified` and `battle` set, `base` left at zero — how enemies load.
    #[must_use]
    pub const fn enemy(value: u8) -> StatTriple {
        StatTriple {
            base: 0,
            modified: value,
            battle: value,
        }
    }
}

/// A stat the cartridge stores twice: derived and live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatPair {
    /// The derived value, from `UpdateCharModStats` or the enemy record.
    pub derived: u16,
    /// The live value, which buffs and debuffs move.
    pub battle: u16,
}

impl StatPair {
    /// Both halves the same.
    #[must_use]
    pub const fn uniform(value: u16) -> StatPair {
        StatPair {
            derived: value,
            battle: value,
        }
    }
}

impl Stats {
    /// `Battle_FillEnemyStats` — `ps4.asm:11939`.
    ///
    /// Base stat bytes stay zero; the record's values go into the `mod` and
    /// `battle` slots. Element properties are written to **both** bytes of
    /// each word (`move.b (a0), $30(a1)` then `move.b (a0)+, $31(a1)`), which
    /// is why an enemy's Defend-equivalent would wear off correctly if enemies
    /// could defend.
    #[must_use]
    pub fn from_enemy(record: &EnemyRecord) -> Stats {
        Stats {
            profession: 0,
            level: 0,
            experience: 0,
            curr_hp: record.hp,
            max_hp: record.hp,
            curr_tp: u16::from(record.attack_element),
            max_tp: u16::from(record.attack_status),
            status: 0,
            strength: StatTriple::enemy(record.strength),
            mental: StatTriple::enemy(record.mental),
            agility: StatTriple::enemy(record.agility),
            dexterity: StatTriple::enemy(record.dexterity),
            attack: StatPair::uniform(record.attack),
            defence: StatPair::uniform(record.defence),
            mental_defence: StatPair::uniform(record.mental_defence),
            element_props: record.properties,
            element_shadow: record.properties,
            equipment: [0; EQUIPMENT_SLOTS],
            enemy_id: record.id,
            gain_exp_flag: false,
        }
    }

    /// A character at the state their initial record describes.
    ///
    /// Derived stats come from [`Stats::update_mod_stats`], so the caller must
    /// supply the equipped items.
    ///
    /// The cartridge populates the element-property *high* bytes indirectly:
    /// `Character_Init` writes only the low ones and the first
    /// `Battle_RestoreStatsAtTurnEnd` — which the battle intro runs before the
    /// first round — copies them up. Setting both here reaches the same state
    /// without modelling a frame of stale RAM.
    ///
    /// # Errors
    /// Propagates whatever [`Stats::update_mod_stats`] reports.
    pub fn from_character(
        record: &CharacterRecord,
        item: impl Fn(u8) -> Option<ItemRecord>,
    ) -> Stats {
        let mut stats = Stats {
            profession: record.profession,
            level: record.level,
            experience: record.experience,
            curr_hp: record.hp,
            max_hp: record.hp,
            curr_tp: record.tp,
            max_tp: record.tp,
            status: 0,
            strength: StatTriple::uniform(record.strength),
            mental: StatTriple::uniform(record.mental),
            agility: StatTriple::uniform(record.agility),
            dexterity: StatTriple::uniform(record.dexterity),
            attack: StatPair::default(),
            defence: StatPair::default(),
            mental_defence: StatPair::default(),
            element_props: record.properties,
            element_shadow: record.properties,
            equipment: record.equipment,
            enemy_id: 0,
            gain_exp_flag: false,
        };
        stats.update_mod_stats(&item);
        stats
    }

    /// `UpdateCharModStats` — retail `$0005F754` (`ps4.asm:127814`).
    ///
    /// Seven passes over the four equipment slots:
    ///
    /// | derived stat | base | item offsets summed |
    /// |---|---|---|
    /// | `strength_mod` | strength | `$B` |
    /// | `mental_mod` | mental | `$C` |
    /// | `agility_mod` | agility | `$D` |
    /// | `dexterity_mod` | dexterity | `$E` |
    /// | `atk_pow` | strength | `$B` + `$F` |
    /// | `dfs_pow` | agility | `$D` + `$10` |
    /// | `magic_dfs` | mental | `$C` + `$11` |
    ///
    /// The four small stats accumulate with `add.b` and **wrap in a byte**;
    /// the three derived words accumulate with `ext.w` + `add.w`, so their
    /// bonuses are signed. Both are reproduced.
    ///
    /// This also refreshes the `battle` copies, matching `FillBattleStats`
    /// (`ps4.asm:11272`), which is what a fighter enters a battle holding.
    pub fn update_mod_stats(&mut self, item: &impl Fn(u8) -> Option<ItemRecord>) {
        let equipped: Vec<Bonuses> = self
            .equipment
            .iter()
            .filter(|id| **id != 0)
            .filter_map(|id| item(*id))
            .map(|record| record.bonuses)
            .collect();

        let sum_byte = |base: u8, pick: fn(&Bonuses) -> i8| {
            equipped
                .iter()
                .fold(base, |acc, b| acc.wrapping_add(pick(b) as u8))
        };
        let sum_word = |base: u8, a: fn(&Bonuses) -> i8, b: fn(&Bonuses) -> i8| {
            equipped.iter().fold(i32::from(base), |acc, bonus| {
                acc + i32::from(a(bonus)) + i32::from(b(bonus))
            }) as u16
        };

        self.strength.modified = sum_byte(self.strength.base, |b| b.strength);
        self.mental.modified = sum_byte(self.mental.base, |b| b.mental);
        self.agility.modified = sum_byte(self.agility.base, |b| b.agility);
        self.dexterity.modified = sum_byte(self.dexterity.base, |b| b.dexterity);

        self.attack.derived = sum_word(self.strength.base, |b| b.strength, |b| b.attack);
        self.defence.derived = sum_word(self.agility.base, |b| b.agility, |b| b.defence);
        self.mental_defence.derived =
            sum_word(self.mental.base, |b| b.mental, |b| b.mental_defence);

        self.refresh_battle_stats();
    }

    /// `FillBattleStats` — `ps4.asm:11272`. Copies every `mod` into its
    /// `battle` slot, which is what happens on entering a battle.
    pub fn refresh_battle_stats(&mut self) {
        self.strength.battle = self.strength.modified;
        self.mental.battle = self.mental.modified;
        self.agility.battle = self.agility.modified;
        self.dexterity.battle = self.dexterity.modified;
        self.attack.battle = self.attack.derived;
        self.defence.battle = self.defence.derived;
        self.mental_defence.battle = self.mental_defence.derived;
    }

    /// Whether this fighter is out of the fight (`status & $44`).
    #[must_use]
    pub const fn is_out(&self) -> bool {
        self.status & status::OUT != 0
    }

    /// Whether this fighter gets a turn (`status & $6E` clear).
    #[must_use]
    pub const fn can_act(&self) -> bool {
        self.status & status::NO_TURN == 0
    }

    /// Whether this is an android.
    #[must_use]
    pub const fn is_android(&self) -> bool {
        self.profession == PROFESSION_ANDROID
    }

    /// The element factor this fighter presents against an attack of
    /// `element_id`, or `None` when the id selects nothing.
    ///
    /// Element ids are one-based over [`Stats::element_props`]; the cartridge
    /// reaches them as `$2E + 2 * id`.
    #[must_use]
    pub fn element_factor(&self, element_id: u8) -> Option<u8> {
        let index = usize::from(element_id).checked_sub(1)?;
        self.element_props.get(index).copied()
    }

    /// `Character_Defend`'s tail: physical resistance until the round ends.
    pub const fn begin_defending(&mut self) {
        self.element_props[0] = DEFENDING_PHYSICAL_PROP;
    }

    /// `Battle_RestoreStatsAtTurnEnd`'s first act (`ps4.asm:9806`):
    /// `move.b $31(a3), physical_prop(a3)`.
    ///
    /// Only the physical slot, only from the shadow byte. The other thirteen
    /// are restored by `AbilityEffect_RestoreStats`, which is Tier 2.
    pub const fn restore_physical_prop(&mut self) {
        self.element_props[0] = self.element_shadow[0];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::fixtures;
    use crate::battle::records::{BattleData, ItemKind};

    /// The fixture item table, in the shape `update_mod_stats` wants.
    fn lookup(id: u8) -> Option<ItemRecord> {
        fixtures::items().into_iter().find(|item| item.id == id)
    }

    #[test]
    fn the_opening_party_derives_the_stats_the_oracle_saw_in_ram() {
        // Tape 07/09 logged the party's live stats. Reproducing them from the
        // initial records is the end-to-end check on UpdateCharModStats.
        let chaz = Stats::from_character(&fixtures::chaz(), lookup);
        assert_eq!(chaz.attack.battle, 18, "8 strength + two Hunt-Knives at 5");
        assert_eq!(chaz.defence.battle, 10, "7 agility + helm 1 + cloth 2");
        assert_eq!(chaz.dexterity.battle, 5);
        assert_eq!(chaz.agility.battle, 7);
        assert_eq!(chaz.max_hp, 25);

        let alys = Stats::from_character(&fixtures::alys(), lookup);
        assert_eq!(alys.attack.battle, 13, "12 strength + Boomerang 1");
        assert_eq!(alys.defence.battle, 18, "15 agility + crown 1 + cloth 2");
        assert_eq!(alys.dexterity.battle, 13);
        assert_eq!(alys.agility.battle, 15);

        let hahn = Stats::from_character(&fixtures::hahn(), lookup);
        assert_eq!(hahn.attack.battle, 8, "6 strength + Dagger 2");
        assert_eq!(
            hahn.defence.battle, 9,
            "4 agility + shield 2 + band 1 + cloth 2"
        );
    }

    #[test]
    fn a_character_fills_base_mod_and_battle_but_an_enemy_skips_base() {
        // The oracle's wrinkle, and the reason all three copies exist here.
        let chaz = Stats::from_character(&fixtures::chaz(), lookup);
        assert_eq!(chaz.strength.base, 8);
        assert_eq!(chaz.strength.modified, 8, "no item gives strength");
        assert_eq!(chaz.strength.battle, 8);

        let enemy = Stats::from_enemy(&fixtures::zoran_bult());
        assert_eq!(
            enemy.strength.base, 0,
            "reading `strength` off an enemy gets 0"
        );
        assert_eq!(enemy.strength.modified, 18);
        assert_eq!(enemy.strength.battle, 18);
        assert_eq!(enemy.agility.base, 0);
        assert_eq!(enemy.agility.battle, 6);
        assert_eq!(enemy.level, 0, "not a level at all");
        assert_eq!(enemy.attack.battle, 16);
        assert_eq!(enemy.defence.battle, 2);
        assert_eq!(enemy.curr_hp, 25);
        assert_eq!(enemy.max_hp, 25);
        // The attack element and status live in the TP slots.
        assert_eq!(enemy.curr_tp, 1, "physical");
        assert_eq!(enemy.max_tp, 0, "no status rider");
        assert_eq!(
            Stats::from_enemy(&fixtures::monster_fly()).max_tp,
            0x1B,
            "MonsterFly's plain attack carries poison"
        );
    }

    #[test]
    fn an_empty_equipment_slot_contributes_nothing() {
        // Alys's left hand is 0. A zero id must be skipped, not looked up.
        let mut bare = fixtures::chaz();
        bare.equipment = [0; EQUIPMENT_SLOTS];
        let stats = Stats::from_character(&bare, |_| {
            panic!("a zero equipment id must never reach the lookup")
        });
        assert_eq!(stats.attack.derived, 8, "strength alone");
        assert_eq!(stats.defence.derived, 7, "agility alone");
        assert_eq!(stats.mental_defence.derived, 6, "mental alone");
    }

    #[test]
    fn defending_halves_the_physical_factor_and_wears_off() {
        let mut stats = Stats::from_enemy(&fixtures::zoran_bult());
        assert_eq!(stats.element_factor(1), Some(2), "normal");
        stats.begin_defending();
        assert_eq!(
            stats.element_factor(1),
            Some(1),
            "resistant while defending"
        );
        // Other elements are untouched — only $30 moves.
        assert_eq!(stats.element_factor(2), Some(2));
        stats.restore_physical_prop();
        assert_eq!(stats.element_factor(1), Some(2), "restored at turn end");
    }

    #[test]
    fn element_ids_are_one_based_and_bounded() {
        let stats = Stats::from_character(&fixtures::chaz(), lookup);
        assert_eq!(stats.element_factor(0), None, "id 0 selects nothing");
        assert_eq!(stats.element_factor(1), Some(2), "physical is the first");
        assert_eq!(
            stats.element_factor(8),
            Some(0),
            "Chaz is immune to holyword"
        );
        assert_eq!(stats.element_factor(9), Some(1), "and resists brose");
        assert_eq!(stats.element_factor(14), Some(2), "destroy is the last");
        assert_eq!(stats.element_factor(15), None, "past the table");
    }

    #[test]
    fn the_status_masks_are_the_cartridge_constants() {
        assert_eq!(status::NO_TURN, 0x6E);
        assert_eq!(status::OUT, 0x44);

        let mut stats = Stats::from_enemy(&fixtures::zoran_bult());
        assert!(stats.can_act());
        assert!(!stats.is_out());

        stats.status = status::POISONED;
        assert!(stats.can_act(), "poison does not cost a turn");
        assert!(!stats.is_out());

        stats.status = status::ASLEEP;
        assert!(!stats.can_act());
        assert!(!stats.is_out(), "asleep is not out of the fight");

        stats.status = status::TECH_SEALED;
        assert!(stats.can_act(), "a sealed character can still swing");
        assert!(!stats.is_out());

        stats.status = status::DEAD;
        assert!(!stats.can_act());
        assert!(stats.is_out());

        stats.status = status::ANDROID_DEAD;
        assert!(stats.is_out());
    }

    #[test]
    fn a_negative_bonus_is_sign_extended_for_the_word_stats() {
        // AddItemBonusToCharStats2 does `ext.w` before adding; the byte path
        // does not. Retail has no negative bonuses, but the two paths differ
        // and the difference is the cartridge's, so it is pinned.
        let cursed = |id: u8| {
            (id == 2).then(|| ItemRecord {
                id: 2,
                name: "CURSED".into(),
                kind: ItemKind::OneHandedSingleTarget,
                bonuses: Bonuses {
                    attack: -4,
                    defence: -3,
                    ..Bonuses::default()
                },
                element: 1,
            })
        };
        let mut record = fixtures::chaz();
        record.equipment = [2, 0, 0, 0];
        let stats = Stats::from_character(&record, cursed);
        assert_eq!(stats.attack.derived, 4, "8 - 4");
        assert_eq!(stats.defence.derived, 4, "7 - 3");
    }

    #[test]
    fn the_fixture_data_set_resolves_everything_it_names() {
        // A fixture that quietly lost a record would make later tests lie.
        let data: BattleData = fixtures::data();
        for id in [1u16, 9, 10] {
            assert!(data.enemy(id).is_ok(), "enemy {id}");
        }
        for id in [1u8, 2, 3, 4, 5, 6, 7, 10] {
            assert!(data.item(id).is_ok(), "item {id}");
        }
        for id in [0u8, 1, 2] {
            assert!(data.level_table(id).is_ok(), "level table {id}");
        }
    }
}
