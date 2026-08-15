//! The two selection tables every damaging action goes through.
//!
//! Both verified in ROM and reproduced here as typed data rather than as magic
//! numbers at the call sites.

/// How wide a stat is read.
///
/// The rule is a threshold, not a per-stat flag: offsets below `$26` are read
/// with `move.b`, `$26` and above with `move.w`. That single `cmpi.w #$26`
/// appears at four call sites, which is the whole reason the distinction is
/// visible at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatWidth {
    /// `move.b` — offsets below [`WORD_STAT_THRESHOLD`].
    Byte,
    /// `move.w` — offsets at or above it.
    Word,
}

/// The offset at which a stat read widens from a byte to a word.
pub const WORD_STAT_THRESHOLD: u16 = 0x26;

/// One entry of the stat-offset table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatSlot {
    /// Offset into the fighter's stats struct.
    pub offset: u16,
    /// The disassembly's name for it, for reading the table back.
    pub name: &'static str,
}

impl StatSlot {
    /// How this offset is read, by the `cmpi.w #$26` rule.
    #[must_use]
    pub const fn width(&self) -> StatWidth {
        if self.offset >= WORD_STAT_THRESHOLD {
            StatWidth::Word
        } else {
            StatWidth::Byte
        }
    }

    /// Whether this entry selects no stat at all (index 0).
    #[must_use]
    pub const fn is_none(&self) -> bool {
        self.offset == 0
    }
}

/// `loc_275A` @ `$00275A` — "which stat", indexed by the record's byte 1
/// masked with `$7F`, or by byte 4 for the defence side.
///
/// ROM bytes: `0000 001A 001D 0020 0023 0026 002A 002E`.
pub const STAT_OFFSETS: [StatSlot; 8] = [
    StatSlot {
        offset: 0x0000,
        name: "none",
    },
    StatSlot {
        offset: 0x001A,
        name: "strength",
    },
    StatSlot {
        offset: 0x001D,
        name: "mental",
    },
    StatSlot {
        offset: 0x0020,
        name: "agility",
    },
    StatSlot {
        offset: 0x0023,
        name: "dexterity",
    },
    StatSlot {
        offset: 0x0026,
        name: "attack",
    },
    StatSlot {
        offset: 0x002A,
        name: "defence",
    },
    StatSlot {
        offset: 0x002E,
        name: "mental_defence",
    },
];

/// The mask applied to the record byte before indexing [`STAT_OFFSETS`].
pub const STAT_INDEX_MASK: u8 = 0x7F;

/// Looks up a stat slot, masking and bounds-checking the record byte.
#[must_use]
pub fn stat_slot(record_byte: u8) -> Option<&'static StatSlot> {
    STAT_OFFSETS.get(usize::from(record_byte & STAT_INDEX_MASK))
}

/// `loc_276A` @ `$00276A` — "which element resistance", indexed by the
/// record's byte 5.
///
/// ROM bytes: `00 30 32 34 36 38 3A 3C 3E 40 42 44 46 48 4A 00`. Entries 1..=14
/// are `$2E + 2*index`; 0 and 15 are `$00`, selecting nothing.
pub const ELEMENT_OFFSETS: [u8; 16] = [
    0x00, 0x30, 0x32, 0x34, 0x36, 0x38, 0x3A, 0x3C, 0x3E, 0x40, 0x42, 0x44, 0x46, 0x48, 0x4A, 0x00,
];

/// The disassembly's names for element ids 1..=14, in order.
pub const ELEMENT_NAMES: [&str; 14] = [
    "physical",
    "energy",
    "fire",
    "gravity",
    "water",
    "anti",
    "electric",
    "holy",
    "bros",
    "bio",
    "psycho",
    "mechanical",
    "efess",
    "destroy",
];

/// Element ids at or above this mean "physical skill — use the weapon's
/// element instead" (`cmpi.b #$10, d3`, `ps4.asm:3817`).
pub const WEAPON_ELEMENT_SENTINEL: u8 = 0x10;

/// The resistance offset for an element id, or `None` when the id selects
/// nothing or defers to the weapon.
///
/// # The weapon fallback is not implemented
///
/// An id at or above [`WEAPON_ELEMENT_SENTINEL`] means the attack takes its
/// element from the attacker's equipped weapon (`Battle_LoadWpnAttackElem`,
/// `$0027DDD4`). Resolving that needs equipment plumbing this tier does not
/// have, so it returns `None` and the caller must notice — deliberately, rather
/// than silently defaulting to physical and producing plausible wrong numbers.
#[must_use]
pub fn element_offset(element_id: u8) -> Option<u8> {
    if element_id >= WEAPON_ELEMENT_SENTINEL {
        // TODO(equipment): resolve through Battle_LoadWpnAttackElem once the
        // fighter's equipped weapon is reachable from the kernel.
        return None;
    }
    match ELEMENT_OFFSETS.get(usize::from(element_id)) {
        Some(0) | None => None,
        Some(offset) => Some(*offset),
    }
}

/// The name of an element id, when it names one.
#[must_use]
pub fn element_name(element_id: u8) -> Option<&'static str> {
    if element_id == 0 || element_id > 14 {
        return None;
    }
    ELEMENT_NAMES.get(usize::from(element_id) - 1).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stat_table_is_the_rom_bytes() {
        let offsets: Vec<u16> = STAT_OFFSETS.iter().map(|s| s.offset).collect();
        assert_eq!(
            offsets,
            vec![
                0x0000, 0x001A, 0x001D, 0x0020, 0x0023, 0x0026, 0x002A, 0x002E
            ]
        );
    }

    #[test]
    fn the_byte_word_split_falls_where_the_compare_puts_it() {
        // Everything below $26 is a byte read; $26 and up are word reads.
        for slot in &STAT_OFFSETS {
            let expected = if slot.offset >= 0x26 {
                StatWidth::Word
            } else {
                StatWidth::Byte
            };
            assert_eq!(slot.width(), expected, "{}", slot.name);
        }
        assert_eq!(STAT_OFFSETS[4].width(), StatWidth::Byte, "dexterity at $23");
        assert_eq!(STAT_OFFSETS[5].width(), StatWidth::Word, "attack at $26");
    }

    #[test]
    fn a_record_byte_is_masked_before_indexing() {
        // Byte 1 carries flags in its high bit; $7F strips them.
        assert_eq!(
            stat_slot(0x82).map(|s| s.name),
            Some("mental"),
            "$82 & $7F = 2"
        );
        assert_eq!(
            stat_slot(0x85).map(|s| s.name),
            Some("attack"),
            "$85 & $7F = 5"
        );
        assert_eq!(stat_slot(0x05).map(|s| s.name), Some("attack"));
        assert_eq!(stat_slot(0x00).map(|s| s.name), Some("none"));
        assert!(STAT_OFFSETS[0].is_none());
        // Past the table: eight entries, so anything else is out of range.
        assert!(stat_slot(0x08).is_none());
        assert!(stat_slot(0x7F).is_none());
    }

    #[test]
    fn the_element_table_is_the_rom_bytes_and_its_arithmetic() {
        assert_eq!(
            ELEMENT_OFFSETS,
            [
                0x00, 0x30, 0x32, 0x34, 0x36, 0x38, 0x3A, 0x3C, 0x3E, 0x40, 0x42, 0x44, 0x46, 0x48,
                0x4A, 0x00
            ]
        );
        // Entries 1..=14 are $2E + 2*id, which is how the scout derives
        // MonsterFly's physical prop: 1*2 + $2E = $30.
        for id in 1..=14u8 {
            assert_eq!(
                ELEMENT_OFFSETS[usize::from(id)],
                0x2E + 2 * id,
                "element {id}"
            );
        }
        assert_eq!(element_offset(1), Some(0x30), "physical");
        assert_eq!(element_name(1), Some("physical"));
        assert_eq!(element_name(14), Some("destroy"));
    }

    #[test]
    fn ids_that_select_nothing_say_so() {
        assert_eq!(element_offset(0), None);
        assert_eq!(element_offset(15), None);
        assert_eq!(element_name(0), None);
        assert_eq!(element_name(15), None);
    }

    #[test]
    fn the_weapon_fallback_refuses_rather_than_guessing() {
        // $10 and up defer to the equipped weapon, which this tier cannot
        // resolve. Returning None makes the caller handle it.
        for id in [0x10u8, 0x11, 0x20, 0xFF] {
            assert_eq!(element_offset(id), None, "id {id:#04X}");
        }
    }
}
