//! The damage and healing formulas.
//!
//! Transcribed instruction for instruction, because every truncation in them
//! is load-bearing. Two things a port must not "clean up", both called out by
//! `docs/battle/BATTLE_SCOUT.md` §4.5:
//!
//! - The `lsr.w` after `muls.w` throws the product's high word away. The
//!   multiply is 16×16→32, the shift touches only the low word, and nothing
//!   afterwards reads the high half — so the whole pipeline is 16-bit wrapping
//!   arithmetic. Overflow starts at `(S+8)*ATK >= 65536`, i.e. `ATK > 546` at a
//!   maximum roll: unreachable with retail equipment, reproduced anyway.
//! - The shifts are **logical**, not arithmetic. That is invisible until
//!   `t - DEF` goes negative, which happens routinely against a defended
//!   target, and it is why the result is read as a signed word only at the very
//!   end.
//!
//! Element factor 0 ("immune") still deals 1 damage, because the minimum clamp
//! runs after the multiply. Immunity in PSIV is 1 HP per hit, not zero.

use super::rng::Rolls;

/// How many draws `Battle_CalculateDamage` takes: `moveq #$F, d7` then `dbf`.
pub const DAMAGE_DRAWS: usize = 16;

/// The mask on each damage draw: `andi.w #7, d0`.
pub const DAMAGE_ROLL_MASK: u16 = 7;

/// The lowest damage `loc_266C` will store.
pub const MIN_DAMAGE: i16 = 1;

/// The highest damage `loc_266C` will store.
pub const MAX_DAMAGE: i16 = 999;

/// Sums sixteen draws, each masked to 0..7 — the `S` of the scout's write-up.
///
/// Ranges 0..=112 with a mean of 56.
fn sum_draws(rolls: &mut impl Rolls) -> u16 {
    let mut sum = 0u16;
    for _ in 0..DAMAGE_DRAWS {
        sum = sum.wrapping_add(rolls.next_roll() & DAMAGE_ROLL_MASK);
    }
    sum
}

/// `Battle_CalculateDamage` — retail **`$00B5CA`** (`ps4.asm:17374`).
///
/// ```text
///     moveq   #$F, d7
///     moveq   #0, d6
/// -   jsr     (UpdateRNGSeed2).l
///     andi.w  #7, d0
///     add.w   d0, d6              ; 16 draws of 0..7 -> S in [0,112]
///     dbf     d7, -
///     addq.w  #8, d6              ; S + 8
///     muls.w  d1, d6              ; * attack power
///     lsr.w   #6, d6              ; / 64, LOW WORD ONLY
///     add.w   d1, d6              ; + attack power
///     add.w   d4, d4
///     add.w   d4, d6              ; + 2 * bonus
///     muls.w  d3, d6              ; * element factor
///     lsr.w   #2, d6              ; / 4, LOW WORD ONLY
///     sub.w   d2, d6              ; - defence
/// ```
///
/// Returns the raw signed word, **unclamped** — `loc_266C` is a separate step
/// and some callers read the raw value. Pass it through [`clamp_damage`] to get
/// what gets stored in `Battle_Heal_Damage_List`.
///
/// Draws exactly [`DAMAGE_DRAWS`] rolls from `rolls`, always, even when the
/// result is predetermined.
pub fn calculate_damage(
    attack: u16,
    defence: u16,
    element_factor: u16,
    bonus: u16,
    rolls: &mut impl Rolls,
) -> i16 {
    let mut t = sum_draws(rolls).wrapping_add(8);
    t = t.wrapping_mul(attack);
    t >>= 6;
    t = t.wrapping_add(attack);
    // `add.w d4, d4` doubles the bonus in place, so the doubling wraps too.
    t = t.wrapping_add(bonus.wrapping_add(bonus));
    t = t.wrapping_mul(element_factor);
    t >>= 2;
    t = t.wrapping_sub(defence);
    t as i16
}

/// `loc_266C` — retail `$00266C`. Clamps to `1..=999` and stores.
///
/// Separate from [`calculate_damage`] because the raw word has callers of its
/// own, and because the clamp is what makes an "immune" hit deal 1 rather
/// than 0.
#[must_use]
pub const fn clamp_damage(raw: i16) -> u16 {
    if raw < MIN_DAMAGE {
        MIN_DAMAGE as u16
    } else if raw > MAX_DAMAGE {
        MAX_DAMAGE as u16
    } else {
        raw as u16
    }
}

/// `Battle_CalcHealing` — retail **`$00B5FE`** (`ps4.asm:17411`).
///
/// The damage shape without a defence subtraction or an element multiply, and
/// with a final halving:
///
/// ```text
/// heal = ((((S + 8) * MEN) >> 6) + MEN + 2*POWER) >> 1
/// ```
///
/// `power_stat` is `d2` — `mental_battle` for techniques, the record's byte-1
/// stat for skills, item byte 1 for items. `power_byte` is `d3`, the record's
/// byte 3. The caller clamps the result to `max_hp`, or to `max_tp` for the one
/// hardcoded exception, Ataraxia (`ps4.asm:4639`); neither clamp belongs here.
///
/// Draws exactly [`DAMAGE_DRAWS`] rolls, like the damage path.
pub fn calc_healing(power_stat: u16, power_byte: u16, rolls: &mut impl Rolls) -> u16 {
    let mut t = sum_draws(rolls).wrapping_add(8);
    t = t.wrapping_mul(power_stat);
    t >>= 6;
    t = t.wrapping_add(power_stat);
    t = t.wrapping_add(power_byte.wrapping_add(power_byte));
    t >> 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::rng::SliceRolls;

    /// Sixteen draws whose masked values sum to `S`.
    fn draws_for(sum: u16) -> Vec<u16> {
        assert!(sum <= 112, "S maxes out at 112");
        let mut out = vec![0u16; DAMAGE_DRAWS];
        let mut left = sum;
        for slot in &mut out {
            let take = left.min(7);
            *slot = take;
            left -= take;
        }
        assert_eq!(left, 0);
        out
    }

    fn damage_at(sum: u16, attack: u16, defence: u16, element: u16, bonus: u16) -> i16 {
        let draws = draws_for(sum);
        let mut rolls = SliceRolls::new(&draws);
        let value = calculate_damage(attack, defence, element, bonus, &mut rolls);
        assert_eq!(rolls.drawn(), DAMAGE_DRAWS, "the loop is always 16 draws");
        value
    }

    #[test]
    fn only_the_low_three_bits_of_a_draw_count() {
        // The mask is part of the formula: a source handing back big words must
        // give the same S as one handing back 0..7.
        let big = vec![0xFFF8 | 3; DAMAGE_DRAWS];
        let small = vec![3u16; DAMAGE_DRAWS];
        let mut a = SliceRolls::new(&big);
        let mut b = SliceRolls::new(&small);
        assert_eq!(
            calculate_damage(18, 0, 2, 0, &mut a),
            calculate_damage(18, 0, 2, 0, &mut b)
        );
    }

    #[test]
    fn chaz_against_a_monsterfly_matches_the_worked_example() {
        // Scout §12: Chaz atk 18, MonsterFly def 0, physical prop 2, no bonus.
        assert_eq!(damage_at(56, 18, 0, 2, 0), 18, "mean roll");
        assert_eq!(damage_at(0, 18, 0, 2, 0), 10, "minimum roll");
        assert_eq!(damage_at(112, 18, 0, 2, 0), 25, "maximum roll");
    }

    #[test]
    fn a_monsterfly_against_chaz_matches_the_worked_example() {
        // Scout §12: attack 14, Chaz defence 10, physical prop 2.
        assert_eq!(damage_at(56, 14, 10, 2, 0), 4, "mean roll");
        assert_eq!(
            damage_at(0, 14, 10, 2, 0),
            -3,
            "minimum roll goes negative before the clamp"
        );
        assert_eq!(clamp_damage(damage_at(0, 14, 10, 2, 0)), 1);
        assert_eq!(damage_at(112, 14, 10, 2, 0), 10, "maximum roll");
    }

    #[test]
    fn a_critical_bonus_enters_before_the_element_multiply() {
        // A crit adds `attack >> 2` as the bonus: 14 >> 2 = 3.
        //   (56+8)*14 = 896 ; >>6 = 14 ; +14 = 28 ; +2*3 = 34 ;
        //   *2 = 68 ; >>2 = 17 ; -10 = 7
        //
        // Scout §12's crit line reads "+2*3 = 6 before the halving -> 17 ;
        // >>2 ... = 7 ; -10 = -3 -> 1", which is internally inconsistent: if
        // 17 is the post-shift value then 17-10 is 7, not -3. Following the
        // instruction sequence gives 7, and the doubled bonus is worth a
        // meaningful 3 damage rather than nothing. The non-critical line
        // beside it (4) is right and is pinned above as a control.
        assert_eq!(damage_at(56, 14, 10, 2, 3), 7);
        assert_eq!(clamp_damage(damage_at(56, 14, 10, 2, 3)), 7);

        // The bonus is doubled, so it is worth twice its face value before the
        // element multiply halves it back at factor 2.
        assert_eq!(damage_at(56, 14, 10, 2, 0), 4, "the same roll without it");
    }

    #[test]
    fn an_immune_target_still_takes_one() {
        // Element factor 0 zeroes the running total, but the clamp runs after.
        let raw = damage_at(112, 235, 0, 0, 0);
        assert!(raw <= 0, "the multiply wipes it out");
        assert_eq!(clamp_damage(raw), 1, "immunity is 1 HP per hit, not zero");
    }

    #[test]
    fn the_clamp_holds_at_both_ends() {
        assert_eq!(clamp_damage(0), 1);
        assert_eq!(clamp_damage(-30000), 1);
        assert_eq!(clamp_damage(1), 1);
        assert_eq!(clamp_damage(998), 998);
        assert_eq!(clamp_damage(999), 999);
        assert_eq!(clamp_damage(1000), 999);
        assert_eq!(clamp_damage(i16::MAX), 999);
    }

    #[test]
    fn the_cap_catches_a_big_ability_hit() {
        // Retail's realistic attack ceiling is ~235 (scout §4.5), and at
        // element 4 that is only 675 — so the 999 cap is not reachable from a
        // basic attack. A high ability power byte gets there, since the bonus
        // is doubled before the multiply.
        assert_eq!(damage_at(112, 235, 0, 4, 0), 675, "no bonus, under the cap");

        let raw = damage_at(112, 235, 0, 4, 200);
        assert!(raw > MAX_DAMAGE, "raw {raw} should exceed the cap");
        assert_eq!(clamp_damage(raw), 999);
    }

    #[test]
    fn the_sixteen_bit_product_really_truncates() {
        // (S+8) * ATK overflows a word once ATK > 546 at a maximum roll, and
        // the `lsr.w` discards the high half. Clean 32-bit math would give a
        // very different answer; the cartridge's does not.
        let attack = 547u16;
        let raw = damage_at(112, attack, 0, 2, 0);

        let wrapped = (120u16).wrapping_mul(attack);
        assert!(
            (120u32) * u32::from(attack) > 0xFFFF,
            "the premise: this product does overflow"
        );
        assert_eq!(wrapped, 104, "and wraps to a small number");

        // Follow the wrapped value through by hand: 104 >> 6 = 1; + 547 = 548;
        // * 2 = 1096; >> 2 = 274; - 0 = 274.
        assert_eq!(raw, 274);

        // Clean 32-bit arithmetic would have produced something far larger.
        let clean = ((120u32 * u32::from(attack)) >> 6) + u32::from(attack);
        assert!(clean > 1000, "clean math diverges hugely: {clean}");
    }

    #[test]
    fn healing_follows_the_same_shape_with_a_final_halving() {
        let draws = draws_for(56);
        let mut rolls = SliceRolls::new(&draws);
        // MEN 20, power byte 10: (64*20)>>6 = 20; +20 = 40; +20 = 60; >>1 = 30.
        assert_eq!(calc_healing(20, 10, &mut rolls), 30);
        assert_eq!(rolls.drawn(), DAMAGE_DRAWS);

        let draws = draws_for(0);
        let mut rolls = SliceRolls::new(&draws);
        // (8*20)>>6 = 2; +20 = 22; +20 = 42; >>1 = 21.
        assert_eq!(calc_healing(20, 10, &mut rolls), 21);
    }

    #[test]
    fn healing_takes_no_element_and_no_defence() {
        // The shape differs from damage in exactly those two ways, so a heal
        // with the same numbers is not a damage roll.
        let draws = draws_for(56);
        let mut a = SliceRolls::new(&draws);
        let mut b = SliceRolls::new(&draws);
        let heal = calc_healing(18, 0, &mut a);
        let damage = calculate_damage(18, 0, 2, 0, &mut b);
        assert_eq!(heal, 18, "((64*18)>>6 + 18) >> 1");
        assert_eq!(damage, 18);
        // Same number here by coincidence of the constants; the shapes differ.
        let mut c = SliceRolls::new(&draws);
        assert_eq!(calc_healing(18, 5, &mut c), 23, "the power byte doubles");
    }
}
