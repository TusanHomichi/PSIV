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
    Bonuses, CharacterRecord, ELEMENT_SLOTS, EQUIPMENT_SLOTS, EnemyRecord, ItemKind, ItemRecord,
    SKILL_SLOTS, TECHNIQUE_SLOTS,
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

/// What `UpdateCharElems` writes into a property an equipped item names.
///
/// Always `1` — "resistant" — regardless of the item or the character.
pub const GRANTED_RESISTANCE: u8 = 1;

/// The element factor a fighter gets while defending.
///
/// `Character_Defend`'s tail writes `move.b #1, $30(a0)` (`ps4.asm:6824`) —
/// physical resistance, halving incoming physical damage, until the end-of-turn
/// restore puts it back.
pub const DEFENDING_PHYSICAL_PROP: u8 = 1;

/// One fighter's stats, laid out as the cartridge's 128-byte struct.
///
/// # A shared interface type
///
/// This is **the** persistent per-character record, not a battle-local copy.
/// `GameState` owns eleven of them — the cartridge's own `Character_Stats`
/// model at `$FFFFF500` — and a battle reads and writes them in place rather
/// than converting to and from something of its own. That is the whole reason
/// the struct carries fields battle never reads, like [`Stats::experience`] and
/// [`Stats::gain_exp_flag`].
///
/// Its shape is therefore a contract between this crate's battle and field
/// halves, and changing it needs the lead's sign-off (adjudicated 2026-08-15).
/// Adding a *method* is free; adding, removing or repurposing a **field** is
/// not.
///
/// # The round-trip invariant
///
/// A battle must leave `curr_hp`, `curr_tp`, `experience`, `level`, `status`
/// and `gain_exp_flag` in a state the field can carry straight on with.
/// [`Battle::into_party`](crate::battle::Battle::into_party) is the handoff,
/// and `battle::engine::tests::handoff` pins the invariant. Everything a battle mutates
/// temporarily — the `battle` copies, [`Stats::element_props`] under a Defend —
/// is restored or recomputed before the battle ends, so no caller has to know
/// which fields were transient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stats {
    /// `$00..$05`. The cartridge name buffer, terminated by `$FE`.
    ///
    /// The field is present on character records only. It is kept here rather
    /// than reconstructed from pack metadata because a save carries the bytes
    /// in place and the name is part of the 0x80-byte record.
    pub name_bytes: [u8; 6],
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
    /// `$50` and `$51`: the attack element each hand's weapon carries, as
    /// [`Stats::update_char_elems`] caches it.
    ///
    /// **Presentation only.** The damage pipeline reads a weapon's element
    /// straight out of `InventoryData` through `Battle_LoadWpnAttackElem`
    /// (`$0027DDD4`); these two bytes are consumed by the battle object that
    /// draws the swing (`move.w $50(a1), $1E(a4)`, `ps4.asm:33857`). They are
    /// modelled because the pack emits them in its conformance vector and
    /// because the same two offsets mean something entirely different on an
    /// enemy, where `$50`..`$5F` is the AI block.
    pub weapon_elements: [u8; 2],
    /// `$4C`..`$4F`: right hand, left hand, head, body.
    pub equipment: [u8; EQUIPMENT_SLOTS],
    /// `$52..$61`: the sixteen technique ids. Zero means an empty slot.
    pub techniques: [u8; TECHNIQUE_SLOTS],
    /// `$62..$69`: the eight character skill ids.
    ///
    /// Enemy stats use `$68..$69` as the `enemy_id` union instead; save
    /// serialization is for character records and therefore writes these
    /// bytes as skills, never as `enemy_id`.
    pub skills: [u8; SKILL_SLOTS],
    /// Even bytes `$6A..$78`: current uses for the eight skills.
    pub curr_skill_uses: [u8; SKILL_SLOTS],
    /// Odd bytes `$6B..$79`: maximum uses for the eight skills.
    pub max_skill_uses: [u8; SKILL_SLOTS],
    /// `$7B` — the finished physical property, saved so Defend can be undone
    /// without losing what armour granted.
    ///
    /// # A ratified bug fix
    ///
    /// Retail has no such byte. `Battle_RestoreStatsAtTurnEnd` puts `$30` back
    /// from `$31` (`ps4.asm:9806`), and `$31` holds the *character record's*
    /// innate value — not the value `UpdateCharElems` computed from armour. So
    /// on the cartridge, defending once permanently discards whatever
    /// resistance your gear was granting for the rest of the battle. That is
    /// the Defend/armour `physical_prop` clobber in
    /// `docs/RUNTIME_DESIGN.md` "Battle bug policy", listed as a fix; the
    /// disassembly's own `bugfixes=1` branch invents this same byte to solve it
    /// (`physical_prop_save = $7B`, `ps4.constants.asm:61`).
    pub physical_prop_save: u8,
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
    /// Decode the saved name's retail font bytes, including renamed characters.
    #[must_use]
    pub fn display_name(&self) -> String {
        self.name_bytes
            .iter()
            .take(5)
            .take_while(|byte| **byte != 0xFE)
            .map(|byte| match *byte {
                1..=26 => char::from(b'A' + byte - 1),
                57..=82 => char::from(b'a' + byte - 57),
                27..=36 => char::from(b'0' + byte - 27),
                0 => ' ',
                0x31 => '-',
                0x32 => '!',
                0x33 => '?',
                0x34 => ':',
                0x53 => '.',
                0x54 => '\'',
                0x55 => ',',
                _ => '?',
            })
            .collect()
    }

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
            name_bytes: [0; 6],
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
            weapon_elements: [0; 2],
            equipment: [0; EQUIPMENT_SLOTS],
            techniques: [0; TECHNIQUE_SLOTS],
            skills: [0; SKILL_SLOTS],
            curr_skill_uses: [0; SKILL_SLOTS],
            max_skill_uses: [0; SKILL_SLOTS],
            physical_prop_save: record.properties[0],
            enemy_id: record.id,
            gain_exp_flag: false,
        }
    }

    /// A character seated exactly as `InitializeCharStats` (`$0044652`) leaves
    /// them.
    ///
    /// That routine copies the 66-byte record into the 128-byte struct and
    /// finishes with **both** derivation passes — [`Stats::update_mod_stats`]
    /// and [`Stats::update_char_elems`] — so a seated character's derived stats
    /// and finished element properties are a pure function of the record plus
    /// the items it names. `item` supplies those items.
    ///
    /// The pack emits the expected result as each character's `initialized`
    /// vector; all eleven are pinned as tests.
    ///
    /// Note which copy the record's element bytes land in: `Character_Init`
    /// writes only the *low* halves (`ps4.asm:88774`), and
    /// [`Stats::update_char_elems`] is what fills the high halves the damage
    /// pipeline reads.
    pub fn from_character(
        record: &CharacterRecord,
        item: impl Fn(u8) -> Option<ItemRecord>,
    ) -> Stats {
        let mut stats = Stats {
            name_bytes: encode_character_name(&record.name),
            profession: record.profession,
            level: record.level,
            experience: record.experience,
            curr_hp: record.hp,
            max_hp: record.max_hp,
            curr_tp: record.tp,
            max_tp: record.max_tp,
            status: 0,
            strength: StatTriple::uniform(record.strength),
            mental: StatTriple::uniform(record.mental),
            agility: StatTriple::uniform(record.agility),
            dexterity: StatTriple::uniform(record.dexterity),
            attack: StatPair::default(),
            defence: StatPair::default(),
            mental_defence: StatPair::default(),
            element_props: [0; ELEMENT_SLOTS],
            element_shadow: record.properties,
            weapon_elements: [0; 2],
            equipment: record.equipment,
            techniques: record.techniques,
            skills: record.skills,
            curr_skill_uses: record.skill_uses,
            max_skill_uses: record.skill_uses,
            physical_prop_save: 0,
            enemy_id: 0,
            gain_exp_flag: false,
        };
        stats.update_mod_stats(&item);
        stats.update_char_elems(&item);
        stats
    }

    /// `UpdateCharElems` — retail **`$0005FD2A`** (`ps4.asm:128345`).
    ///
    /// Rebuilds all fourteen element properties from scratch, from the four
    /// equipment slots and the character record underneath them:
    ///
    /// ```text
    ///     ; clear the fourteen HIGH bytes, and both weapon-element bytes
    ///     move.b  d0, (a0,d2.w)       ; d2 walks $30, $32, .. $4A
    ///     move.w  d0, $50(a0)
    ///
    ///     ; right hand, then left: a shield grants a resistance, anything
    ///     ; else sets that hand's attack element
    ///     cmpi.b  #5, $A(a2)
    ///     beq.s   loc_5FD58           ; -> grant
    ///     bsr.s   loc_5FDB0           ; move.b $12(a2), $50(a0)
    ///
    ///     ; head and body: always grant
    ///     bsr.s   loc_5FDC0
    ///
    /// loc_5FDC0:                      ; grant
    ///     move.b  $12(a2), d0
    ///     beq.s   +                   ; element 0 grants nothing
    ///     subq.w  #1, d0
    ///     add.w   d0, d0
    ///     move.b  #1, $30(a0,d0.w)    ; UNCONDITIONAL
    ///
    ///     ; anything still zero falls back to the record's own byte
    /// loc_5FD8A:
    ///     tst.b   (a1)
    ///     bne.s   +
    ///     move.b  $1(a1), (a1)
    /// ```
    ///
    /// # The grant is unconditional, and that is not a rounding error
    ///
    /// `move.b #1` overwrites whatever was there. A character innately **immune**
    /// to an element (property 0) who equips armour naming that element comes
    /// out merely *resistant* (property 1) — the gear makes them strictly worse
    /// against it. Reachable in play and reproduced deliberately; the pack's
    /// census counts it as `element_props_weakened_by_equipment`, which is empty
    /// for the eleven starting loadouts and need not stay that way.
    ///
    /// The fallback pass keys on the high byte still being zero, which is why a
    /// granted `1` survives it and an untouched slot does not.
    pub fn update_char_elems(&mut self, item: &impl Fn(u8) -> Option<ItemRecord>) {
        self.element_props = [0; ELEMENT_SLOTS];
        self.weapon_elements = [0; 2];

        let equipment = self.equipment;
        for (slot, id) in equipment.iter().enumerate() {
            if *id == 0 {
                continue;
            }
            let Some(record) = item(*id) else { continue };
            // A hand holding anything but a shield sets that hand's attack
            // element; every other case grants a resistance.
            if slot < 2 && record.kind != ItemKind::Shield {
                self.weapon_elements[slot] = record.element;
            } else if let Some(prop) = usize::from(record.element)
                .checked_sub(1)
                .and_then(|index| self.element_props.get_mut(index))
            {
                *prop = GRANTED_RESISTANCE;
            }
        }

        for (prop, innate) in self.element_props.iter_mut().zip(self.element_shadow) {
            if *prop == 0 {
                *prop = innate;
            }
        }
        // FIX: retail restores `$30` from `$31` and so throws the line above
        // away the first time anyone defends. See [`Stats::physical_prop_save`].
        self.physical_prop_save = self.element_props[0];
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
    /// Only the physical slot. The other thirteen are restored by
    /// `AbilityEffect_RestoreStats`, which is Tier 2.
    ///
    /// # A ratified bug fix
    ///
    /// Retail restores from `$31`, the character record's *innate* byte, which
    /// silently discards any physical resistance `UpdateCharElems` derived from
    /// armour. This restores from [`Stats::physical_prop_save`] instead, so
    /// defending costs nothing. The observable Defend behaviour — resistance 1
    /// while defending, the granted value afterwards — is unchanged for anyone
    /// whose gear grants no physical resistance, which is every retail loadout
    /// the oracle has measured.
    pub const fn restore_physical_prop(&mut self) {
        self.element_props[0] = self.physical_prop_save;
    }
}

/// Encodes the ASCII names used by the retail `WinCharset` table into the
/// six-byte name field. `InitializeCharStats` copies source bytes through the
/// `$FF` separator and writes `$FE` as the in-record terminator.
fn encode_character_name(name: &str) -> [u8; 6] {
    let mut bytes = [0; 6];
    let mut cursor = 0;
    for byte in name.bytes() {
        if cursor == 5 {
            break;
        }
        bytes[cursor] = match byte {
            b'A'..=b'Z' => byte - b'A' + 1,
            b'a'..=b'z' => byte - b'a' + 57,
            b'0'..=b'9' => byte - b'0' + 27,
            b' ' => 0,
            b'-' => 0x31,
            b'!' => 0x32,
            b'?' => 0x33,
            b':' => 0x34,
            b'.' => 0x53,
            b'\'' => 0x54,
            b',' => 0x55,
            _ => 0x33,
        };
        cursor += 1;
    }
    bytes[cursor] = 0xFE;
    bytes
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
