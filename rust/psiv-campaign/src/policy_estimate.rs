//! What an action is worth, computed with the engine's own formulas.
//!
//! A player who has fought a few rounds knows roughly what each command does;
//! this module computes that knowledge exactly instead of letting the policy
//! guess. Every number comes from a `psiv_core::battle` formula fed a local,
//! constructed roll sequence ([`SliceRolls`]). Nothing here reads or advances
//! the game's RNG: the session's stream is untouched by a policy decision.
//!
//! * **Damage** is `Battle_CalculateDamage` (`ps4.asm:17374`): sixteen draws of
//!   0..7 summed to `S`, then the 16-bit shape `((S + 8) * attack >> 6) +
//!   attack + 2 * bonus`, times the element factor, `>> 2`, minus the defence,
//!   clamped to 1..999 (`loc_266C`). The policy evaluates
//!   [`calculate_damage`] at every `S` from 0 to 112 and weights each by how
//!   many of the 8^16 draw sequences sum to it, so an expectation is exact,
//!   truncations and clamps included. Capping at a target's HP is done per
//!   outcome, so overkill is never counted.
//! * **Healing** is `Battle_CalcHealing` (`ps4.asm:17411`) the same way.
//! * **Hit, miss and critical** for a plain attack is `Battle_CalculateChances`
//!   (`ps4.asm:17338`) with the physical constants (`loc_B716`,
//!   `ps4.asm:17536`), evaluated at each of its 64 rolls.
//! * **Whether a status effect lands** is the same kernel with the record's
//!   power, the target's resistance stat and element factor, the record's
//!   threshold and effect id (`Effect_DoTechnique`/`Effect_DoSkill`,
//!   `ps4.asm:9546-9610`), as the engine's `player_effect::lands` calls it.
//!
//! Values are integers in 1/256 HP ([`Value`]); the crate forbids floating
//! point.
//!
//! # Effect classes
//!
//! [`EffectClass::of`] reads the ability record's effect byte through the
//! cartridge's own dispatch table, `AbilityEffectsOffs` (`ps4.asm:9036-9086`),
//! so a technique, a skill and an item that share an effect id share a class.

use psiv_core::battle::{
    DAMAGE_DRAWS, DAMAGE_ROLL_MASK, PHYSICAL, SliceRolls, Verdict, calc_healing, calculate_chances,
    calculate_damage, clamp_damage, critical_bonus,
};
use std::sync::OnceLock;

/// A value in 1/256 HP.
pub type Value = u64;

/// One HP as a [`Value`].
pub const HP: Value = 256;

/// What an effect id does, by `AbilityEffectsOffs` (`ps4.asm:9036-9086`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectClass {
    /// Effect 1, `AbilityEffect_None` (`ps4.asm:9092`): the caller's damage pass
    /// (`loc_281E`/`loc_2836` to `loc_266C`).
    Damage,
    /// Effect 2, `AbilityEffect_Death` (`ps4.asm:9098`): HP to zero when it lands.
    Death,
    /// A lowered enemy stat: 3 `AbilityEffect_AttackDown` (`:9109`), 4
    /// `AbilityEffect_DefenseDown` (`:9124`), 6 `AbilityEffect_AgilityDown`
    /// (`:9139`), `$21` `AbilityEffect_DexterityDown` (`:9457`).
    Debuff(Stat),
    /// Effect 7, `AbilityEffect_SleepParalyze` (`:9154`): the target sleeps.
    Sleep,
    /// Effect 8, `AbilityEffect_SealTech` (`:9167`).
    Seal,
    /// A raised ally stat: 9 `AbilityEffect_AttackUp` (`:9181`), `$A`
    /// `AbilityEffect_DefenseUp` (`:9203`), `$B` `AbilityEffect_MagicDefenseUp`
    /// (`:9220`), `$C` `AbilityEffect_AgilityUp` (`:9237`), `$26`
    /// `AbilityEffect_DexterityUp` (`:9254`).
    Buff(Stat),
    /// Effect `$D`, `AbilityEffect_ElementResistanceUp` (`:9265`).
    Resist,
    /// Effect `$12`, `AbilityEffect_NormalLogic` (`:9282`) on the HP path
    /// (`loc_2EC4`/`loc_2ED8` to `loc_2F02`).
    Heal,
    /// Effect `$F` on the TP path (`loc_2F40`, `ps4.asm:4649`): ATARAXIA.
    TpRestore,
    /// Effects `$E` (`AbilityEffect_WakeUp`, `:9360`), `$13` and `$14`
    /// (`AbilityEffect_RestoreAgiAndDex`, `:9402`, the cure-poison and
    /// cure-paralysis objects): the status bits the cure clears.
    Cure(u8),
    /// Effects `$15` and `$16` (`AbilityEffect_RestoreAgiAndDex`, `:9402`, the
    /// revival objects); `full` for `$16`.
    Revive {
        /// Whether the target rises with full HP rather than a quarter.
        full: bool,
    },
    /// Effect `$17` (MEDIC PW): heal every human, the fallen included.
    HealRevive,
    /// Effect `$27`, `AbilityEffect_RestoreStats` (`:9472`): the Psycho Wand.
    Dispel,
    /// Anything else: no use the policy knows.
    Other,
}

/// A stat a buff or debuff changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    /// Attack power.
    Attack,
    /// Defence.
    Defence,
    /// Mental defence.
    MentalDefence,
    /// Agility.
    Agility,
    /// Dexterity.
    Dexterity,
}

impl EffectClass {
    /// The class of effect id `effect`.
    #[must_use]
    pub const fn of(effect: u8) -> EffectClass {
        use psiv_core::battle::status;
        match effect {
            1 => EffectClass::Damage,
            2 => EffectClass::Death,
            3 => EffectClass::Debuff(Stat::Attack),
            4 => EffectClass::Debuff(Stat::Defence),
            6 => EffectClass::Debuff(Stat::Agility),
            0x21 => EffectClass::Debuff(Stat::Dexterity),
            7 => EffectClass::Sleep,
            8 => EffectClass::Seal,
            9 => EffectClass::Buff(Stat::Attack),
            10 => EffectClass::Buff(Stat::Defence),
            11 => EffectClass::Buff(Stat::MentalDefence),
            12 => EffectClass::Buff(Stat::Agility),
            0x26 => EffectClass::Buff(Stat::Dexterity),
            13 => EffectClass::Resist,
            14 => EffectClass::Cure(status::ASLEEP),
            15 => EffectClass::TpRestore,
            18 => EffectClass::Heal,
            19 => EffectClass::Cure(status::POISONED),
            20 => EffectClass::Cure(status::PARALYZED),
            21 => EffectClass::Revive { full: false },
            22 => EffectClass::Revive { full: true },
            23 => EffectClass::HealRevive,
            0x27 => EffectClass::Dispel,
            _ => EffectClass::Other,
        }
    }
}

/// The number of 16-draw sequences of 0..7 whose sum is each `S` in 0..=112;
/// they add up to 8^16 = 2^48.
fn sum_weights() -> &'static [u64; 113] {
    static WEIGHTS: OnceLock<[u64; 113]> = OnceLock::new();
    WEIGHTS.get_or_init(|| {
        let mut weights = [0_u64; 113];
        weights[0] = 1;
        let mut top = 0;
        for _ in 0..DAMAGE_DRAWS {
            let mut next = [0_u64; 113];
            for (sum, ways) in weights.iter().enumerate().take(top + 1) {
                for draw in 0..=usize::from(DAMAGE_ROLL_MASK) {
                    next[sum + draw] += ways;
                }
            }
            weights = next;
            top += usize::from(DAMAGE_ROLL_MASK);
        }
        weights
    })
}

/// Sixteen draws whose masked values sum to `sum`.
fn draws_for(sum: usize) -> [u16; DAMAGE_DRAWS] {
    let mut draws = [0_u16; DAMAGE_DRAWS];
    let mut left = sum;
    for draw in &mut draws {
        let take = left.min(usize::from(DAMAGE_ROLL_MASK));
        *draw = u16::try_from(take).unwrap_or(0);
        left -= take;
    }
    draws
}

/// `Σ weight(S) · f(S)` scaled to a [`Value`]: the exact expectation of `f`.
fn expect(f: impl Fn(&mut SliceRolls<'_>) -> u16) -> Value {
    let total: u64 = sum_weights()
        .iter()
        .enumerate()
        .map(|(sum, weight)| {
            let draws = draws_for(sum);
            let mut rolls = SliceRolls::new(&draws);
            weight * u64::from(f(&mut rolls))
        })
        .sum();
    // Σ weights = 2^48, so the mean in 1/256 HP is the total >> 40.
    total >> 40
}

/// The expected damage of one `Battle_CalculateDamage` pass, each outcome
/// capped at `cap` HP (the target's remaining HP: overkill counts nothing).
#[must_use]
pub fn damage(attack: u16, defence: u16, factor: u16, bonus: u16, cap: u16) -> Value {
    memo((0, attack, defence, factor, bonus, cap), || {
        expect(|rolls| {
            clamp_damage(calculate_damage(attack, defence, factor, bonus, rolls)).min(cap)
        })
    })
}

/// A pure function of its key, remembered: the same fighters meet the same
/// numbers every round, and a training patrol fights thousands of rounds.
fn memo(key: (u8, u16, u16, u16, u16, u16), compute: impl FnOnce() -> Value) -> Value {
    use std::cell::RefCell;
    use std::collections::HashMap;
    thread_local! {
        static CACHE: RefCell<HashMap<(u8, u16, u16, u16, u16, u16), Value>> =
            RefCell::new(HashMap::new());
    }
    if let Some(value) = CACHE.with(|cache| cache.borrow().get(&key).copied()) {
        return value;
    }
    let value = compute();
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() > 1 << 16 {
            cache.clear();
        }
        cache.insert(key, value);
    });
    value
}

/// The most one damage pass can do: every draw 7.
#[must_use]
pub fn max_damage(attack: u16, defence: u16, factor: u16, bonus: u16) -> u16 {
    let draws = draws_for(112);
    clamp_damage(calculate_damage(
        attack,
        defence,
        factor,
        bonus,
        &mut SliceRolls::new(&draws),
    ))
}

/// The expected `Battle_CalcHealing` amount, capped at `cap` (what the
/// patient is missing).
#[must_use]
pub fn healing(power_stat: u16, power: u16, cap: u16) -> Value {
    memo((1, power_stat, power, cap, 0, 0), || {
        expect(|rolls| calc_healing(power_stat, power, rolls).min(cap))
    })
}

/// The least `Battle_CalcHealing` gives: every draw 0.
#[must_use]
pub fn min_healing(power_stat: u16, power: u16) -> u16 {
    calc_healing(power_stat, power, &mut SliceRolls::new(&[0]))
}

/// Of a physical hit roll's 64 outcomes: how many miss, land normally and
/// land critically. `multi` demotes a critical to a normal hit (`loc_B754`).
#[must_use]
pub fn hit_outcomes(dexterity: u16, agility: u16, multi: bool) -> (u64, u64, u64) {
    let (scale, miss, crit) = PHYSICAL;
    let mut counts = (0, 0, 0);
    for roll in 0..64_u16 {
        let verdict = calculate_chances(
            i16::try_from(dexterity).unwrap_or(i16::MAX),
            i16::try_from(agility).unwrap_or(i16::MAX),
            scale,
            miss,
            crit,
            &mut SliceRolls::new(&[roll]),
        );
        match verdict {
            Verdict::Miss => counts.0 += 1,
            Verdict::Normal => counts.1 += 1,
            Verdict::Critical if multi => counts.1 += 1,
            Verdict::Critical => counts.2 += 1,
        }
    }
    counts
}

/// A plain attack's expected damage on one target: the hit roll's outcomes,
/// each with its damage pass, a critical adding [`critical_bonus`].
#[must_use]
pub fn attack_damage(
    attack: u16,
    dexterity: u16,
    agility: u16,
    defence: u16,
    factor: u16,
    multi: bool,
    cap: u16,
) -> Value {
    let (_, normal, critical) = hit_outcomes(dexterity, agility, multi);
    let plain = damage(attack, defence, factor, 0, cap);
    let crit = if critical > 0 {
        damage(attack, defence, factor, critical_bonus(attack), cap)
    } else {
        0
    };
    (normal * plain + critical * crit) / 64
}

/// Of 64 rolls, how many let a status effect land: the effect pass's chance
/// kernel with the record's power and threshold against the target's
/// resistance stat and element factor. A zero resistance selector skips the
/// roll and always lands.
#[must_use]
pub fn landing_chance(power: u16, resistance: u16, factor: u16, threshold: u8, effect: u8, selector: u8) -> u64 {
    if selector == 0 {
        return 64;
    }
    (0..64_u16)
        .filter(|roll| {
            calculate_chances(
                i16::try_from(power).unwrap_or(i16::MAX),
                i16::try_from(resistance).unwrap_or(i16::MAX),
                i16::try_from(factor).unwrap_or(i16::MAX),
                i16::from(threshold),
                i16::from(effect),
                &mut SliceRolls::new(&[*roll]),
            ) != Verdict::Miss
        })
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sum_weights_are_every_sequence_once() {
        let weights = sum_weights();
        assert_eq!(weights.iter().sum::<u64>(), 1 << 48);
        assert_eq!(weights[0], 1);
        assert_eq!(weights[112], 1);
        assert_eq!(weights[56], weights.iter().copied().max().unwrap());
    }

    #[test]
    fn the_expectation_matches_the_formula_at_its_mean() {
        // ((64 * 60 >> 6) + 60) * 2 >> 2, minus 15 is 45 at S = 56; the
        // formula is linear there, so the exact mean is 45 HP give or take
        // the truncations.
        let mean = damage(60, 15, 2, 0, 999);
        assert!((44 * HP..=46 * HP).contains(&mean), "{mean}");
        // Never below one per pass, as `clamp_damage`: an immune target.
        assert_eq!(damage(1, 200, 0, 0, 999), HP);
        // Overkill counts nothing: a 10 HP target.
        assert_eq!(damage(200, 0, 2, 0, 10), 10 * HP);
    }

    #[test]
    fn the_hit_roll_bands_are_the_cartridges() {
        // DEX 5 against AGI 12: r <= 11 misses, never a critical (chances.rs).
        assert_eq!(hit_outcomes(5, 12, false), (12, 52, 0));
        let (miss, normal, crit) = hit_outcomes(80, 10, false);
        assert_eq!(miss + normal + crit, 64);
        assert!(crit > 0);
        assert_eq!(hit_outcomes(80, 10, true).2, 0, "a group swing never crits");
    }

    #[test]
    fn a_zero_element_factor_never_lets_an_effect_land() {
        assert_eq!(landing_chance(50, 10, 0, 8, 2, 7), 0);
        assert_eq!(landing_chance(50, 10, 0, 8, 2, 0), 64);
        assert!(landing_chance(50, 10, 2, 8, 2, 7) > 32);
    }

    #[test]
    fn the_classes_follow_ability_effects_offs() {
        assert_eq!(EffectClass::of(1), EffectClass::Damage);
        assert_eq!(EffectClass::of(18), EffectClass::Heal);
        assert_eq!(EffectClass::of(22), EffectClass::Revive { full: true });
        assert_eq!(EffectClass::of(8), EffectClass::Seal);
        assert_eq!(EffectClass::of(3), EffectClass::Debuff(Stat::Attack));
        assert_eq!(EffectClass::of(10), EffectClass::Buff(Stat::Defence));
        assert_eq!(EffectClass::of(5), EffectClass::Other);
    }
}
