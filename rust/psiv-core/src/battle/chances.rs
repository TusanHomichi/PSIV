//! Hit, miss and critical — `Battle_CalculateChances`, retail `$00B5A6`.
//!
//! One routine serves five call sites with different constants: the physical
//! hit roll, battle-start priority, escape, and two status-effect paths. The
//! kernel takes the constants as arguments rather than naming the sites,
//! because two of the sites reinterpret the return value entirely.

use super::rng::Rolls;

/// The mask on a chance roll: `andi.w #$3F, d0`, so `r` is 0..=63.
pub const CHANCE_ROLL_MASK: u16 = 0x3F;

/// What a chance roll decided.
///
/// The cartridge returns −1 / 0 / 1, and two call sites read those as something
/// other than hit quality: the status-effect paths put the *effect id* in `d5`,
/// which makes the "critical" arm meaningless there, and the escape roll only
/// tests the sign. Callers that care should match on the variant they mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// `-1`. A miss, a failed effect, or a failed escape.
    Miss,
    /// `0`. An ordinary hit.
    Normal,
    /// `1`. A critical hit, a preemptive strike — or, in a status-effect path,
    /// simply "landed", since `d5` there is an effect id rather than a
    /// threshold.
    Critical,
}

/// `Battle_CalculateChances` — retail `$00B5A6`.
///
/// ```text
///     jsr     (UpdateRNGSeed2).l
///     andi.w  #$3F, d0            ; r in 0..63
///     sub.w   d2, d1              ; actor - target
///     add.w   d1, d0
///     muls.w  d3, d0              ; * scale
///     cmp.w   d4, d0
///     ble     -> -1               ; MISS
///     cmp.w   d5, d0
///     ble     ->  0               ; normal
///     ->  1                       ; CRITICAL
/// ```
///
/// `v = (r + actor − target) * scale`, compared as a **signed 16-bit word**
/// against the two thresholds. The `muls.w` is a 16×16→32 multiply but `cmp.w`
/// reads only the low word, so the comparison truncates exactly as the damage
/// pipeline does.
///
/// Both thresholds are inclusive-below: `v <= miss_le` misses, and
/// `v <= crit_gt` (having survived the first test) is a normal hit.
///
/// The physical constants are scale 2, `miss_le = 8`, `crit_gt = $74`, which
/// makes the reachable band `miss ⟺ r + DEX − AGI ≤ 4` and
/// `crit ⟺ r + DEX − AGI > 58`. Since `r ≤ 63`, **a critical requires
/// `DEX − AGI > −5`**: an attacker five or more dexterity below the target's
/// agility can never crit.
///
/// # The escape roll reads an uninitialised threshold
///
/// `Battle_ProcessRUN` (`$0054AC`) never sets `d5`. It is harmless — the caller
/// only tests the sign, and both [`Verdict::Normal`] and [`Verdict::Critical`]
/// are non-negative — but a port should know it is passing a value the
/// cartridge does not, rather than believe it found the real one. Pass anything
/// for `crit_gt` there and read the result as "escaped or not".
pub fn calculate_chances(
    actor: i16,
    target: i16,
    scale: i16,
    miss_le: i16,
    crit_gt: i16,
    rolls: &mut impl Rolls,
) -> Verdict {
    let roll = (rolls.next_roll() & CHANCE_ROLL_MASK) as i16;
    let margin = actor.wrapping_sub(target);
    let value = roll.wrapping_add(margin).wrapping_mul(scale);

    if value <= miss_le {
        Verdict::Miss
    } else if value <= crit_gt {
        Verdict::Normal
    } else {
        Verdict::Critical
    }
}

/// The physical hit roll's constants (`loc_B716`): scale 2, miss ≤ 8,
/// crit > `$74`.
pub const PHYSICAL: (i16, i16, i16) = (2, 8, 0x74);

/// Battle-start priority (`loc_B62A`): scale 2, ambush ≤ `$C`, preemptive
/// > `$74`.
pub const START_PRIORITY: (i16, i16, i16) = (2, 0x0C, 0x74);

/// The escape roll (`Battle_ProcessRUN`): scale 2, fail ≤ `$28`. The third
/// value is **not** the cartridge's — see [`calculate_chances`].
pub const ESCAPE: (i16, i16) = (2, 0x28);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::rng::SliceRolls;

    fn verdict_at(roll: u16, actor: i16, target: i16) -> Verdict {
        let draws = [roll];
        let mut rolls = SliceRolls::new(&draws);
        let (scale, miss, crit) = PHYSICAL;
        calculate_chances(actor, target, scale, miss, crit, &mut rolls)
    }

    #[test]
    fn only_the_low_six_bits_of_the_roll_count() {
        assert_eq!(verdict_at(0xFFC0 | 20, 5, 12), verdict_at(20, 5, 12));
    }

    #[test]
    fn chaz_against_a_monsterfly_misses_on_r_at_most_eleven() {
        // Scout §12: DEX 5 vs AGI 12, so v = (r - 7) * 2; miss iff v <= 8.
        for roll in 0..=11 {
            assert_eq!(verdict_at(roll, 5, 12), Verdict::Miss, "r = {roll}");
        }
        for roll in 12..=63 {
            assert_eq!(verdict_at(roll, 5, 12), Verdict::Normal, "r = {roll}");
        }
    }

    #[test]
    fn chaz_can_never_crit_a_monsterfly() {
        // DEX 5 vs AGI 12 is a margin of -7, past the -5 cliff.
        for roll in 0..=63 {
            assert_ne!(verdict_at(roll, 5, 12), Verdict::Critical, "r = {roll}");
        }
    }

    #[test]
    fn the_monsterfly_hits_chaz_on_the_documented_bands() {
        // Scout §12: DEX 8 vs AGI 7, so v = (r + 1) * 2.
        for roll in 0..=3 {
            assert_eq!(verdict_at(roll, 8, 7), Verdict::Miss, "r = {roll}");
        }
        for roll in 4..=57 {
            assert_eq!(verdict_at(roll, 8, 7), Verdict::Normal, "r = {roll}");
        }
        for roll in 58..=63 {
            assert_eq!(verdict_at(roll, 8, 7), Verdict::Critical, "r = {roll}");
        }
    }

    #[test]
    fn the_critical_cliff_sits_exactly_at_a_margin_of_minus_five() {
        // crit needs r + DEX - AGI > 58 with r <= 63, so DEX - AGI > -5.
        let can_crit =
            |margin: i16| (0..=63).any(|roll| verdict_at(roll, margin, 0) == Verdict::Critical);
        assert!(!can_crit(-5), "a margin of -5 can never crit");
        assert!(can_crit(-4), "a margin of -4 can, on r = 63");
        assert_eq!(verdict_at(63, -4, 0), Verdict::Critical);
        assert_eq!(verdict_at(62, -4, 0), Verdict::Normal);
    }

    #[test]
    fn both_thresholds_are_inclusive_below() {
        // v == miss_le misses; v == crit_gt is still normal.
        let (scale, miss, crit) = PHYSICAL;
        let exactly_miss = [(miss / scale) as u16];
        let mut rolls = SliceRolls::new(&exactly_miss);
        assert_eq!(
            calculate_chances(0, 0, scale, miss, crit, &mut rolls),
            Verdict::Miss
        );

        let exactly_crit = [(crit / scale) as u16];
        let mut rolls = SliceRolls::new(&exactly_crit);
        assert_eq!(
            calculate_chances(0, 0, scale, miss, crit, &mut rolls),
            Verdict::Normal,
            "the crit threshold is strictly greater-than"
        );
    }

    #[test]
    fn a_battle_start_can_be_an_ambush_but_never_preemptive_at_level_one() {
        // Scout §12: party agility 7 vs formation byte $10, so v = (r - 9) * 2.
        let (scale, ambush, preempt) = START_PRIORITY;
        let mut ambushes = 0;
        for roll in 0..=63u16 {
            let draws = [roll];
            let mut rolls = SliceRolls::new(&draws);
            match calculate_chances(7, 0x10, scale, ambush, preempt, &mut rolls) {
                Verdict::Miss => ambushes += 1,
                Verdict::Normal => {}
                Verdict::Critical => panic!("a preemptive strike should be unreachable"),
            }
        }
        assert_eq!(ambushes, 16, "16 of 64 rolls, the documented 25%");
    }

    #[test]
    fn escape_succeeds_on_the_documented_band() {
        // Scout §12: v = (r + 7) * 2 > $28 -> r > 13, so 50 of 64 rolls.
        let (scale, fail) = ESCAPE;
        let escaped = (0..=63u16)
            .filter(|roll| {
                let draws = [*roll];
                let mut rolls = SliceRolls::new(&draws);
                // `crit_gt` is uninitialised in retail; only the sign is read.
                calculate_chances(7, 0, scale, fail, 0, &mut rolls) != Verdict::Miss
            })
            .count();
        assert_eq!(escaped, 50);
    }

    #[test]
    fn the_scale_multiply_truncates_like_the_damage_one() {
        // A large scale overflows the word, and `cmp.w` sees only the low half.
        let draws = [63u16];
        let mut rolls = SliceRolls::new(&draws);
        let verdict = calculate_chances(0, 0, 4096, 0, 0x74, &mut rolls);
        // 63 * 4096 = 258_048, which wraps to 63*4096 mod 65536 = 61_440,
        // read signed as -4096: a miss, where clean math would have crit.
        assert_eq!(verdict, Verdict::Miss);
    }
}
