//! The gate itself: which `(enemy, ability)` pairs `resolve_damage_skill`
//! accepts, and why the record's byte 2 is not part of the answer.
//!
//! These two tests reach for the Acid Breath and FLAME BOLT fixtures as well as
//! the Motavia carrier lines, so they live in their own module and import from
//! all three.

use super::acid_tests::acid_carrier_data;
use super::flame_tests::{flame_carrier_data, flame_carrier_roster};
use super::tests::*;
use super::*;
use crate::battle::{EnemySkill, SliceRolls};

/// The gate is the pair plus the record's effect byte, not record byte 2's
/// target nibble. `AbilityRangeOffs` (`ps4.asm:8903`) resolves nibble 9 to
/// `AbilityRange_MultiChars` (`ps4.asm:8962`), which only decides how often
/// `Ability_ProcessRange` (`ps4.asm:8975`) calls the effect handler — and
/// these records' handler is `AbilityEffect_None` (`ps4.asm:9092`). The damage
/// request comes from the arm's object chain, so the old target-byte check is
/// gone: both listed pairs below resolve with the byte moved to 9.
#[test]
fn a_listed_route_resolves_with_its_record_on_the_nibble_9_target() {
    // The pairs are in the table — 75 FlattrPlnt's `$33` and 0 Helex's `$02` —
    // so only the record's effect byte can still refuse one.
    for (enemy_id, ability) in [(75u16, 51u8), (0, 2)] {
        assert!(
            DAMAGE_SKILL_ROUTES
                .iter()
                .any(|route| route.enemy_id == enemy_id && route.ability == ability),
            "({enemy_id}, {ability}) must be a listed route for this control"
        );
    }
    for (carrier, skill, target) in [(0u16, 2u8, 1u8), (0, 2, 8), (0, 2, 9), (75, 51, 9)] {
        let data = if skill == 2 {
            flame_carrier_data(carrier, 10)
        } else {
            acid_carrier_data(carrier, 10)
        };
        let mut record = data.enemy_skill(skill).unwrap().clone();
        record.target = target;
        let data = data.with_enemy_skills([record]);
        let mut r = flame_carrier_roster(&data, carrier);
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(
            resolve_damage_skill(
                &mut r,
                id(6),
                skill,
                Some(id(2)),
                &data,
                &mut rolls,
                &mut events
            ),
            "carrier {carrier} ability {skill}: target {target} is not a gate"
        );
        assert_eq!(rolls.drawn(), 16, "carrier {carrier}: one damage request");
        assert_eq!(events.len(), 2, "carrier {carrier}: ability and damage");
        assert!(
            matches!(
                events[1],
                BattleEvent::Resolved {
                    damage: Some(_),
                    ..
                }
            ),
            "carrier {carrier}: {events:?}"
        );
    }
}

/// The refusal that replaces the target-byte check: the resolver models the
/// damage request alone, so a listed route whose record runs an effect handler
/// as well stays on the caller's unsupported path, drawing nothing and
/// emitting nothing. `AbilityEffectsOffs` (`ps4.asm:9036`) index `$02` is
/// `AbilityEffect_Death` (`ps4.asm:9098`), index `$01` is
/// `AbilityEffect_None` (`ps4.asm:9092`).
#[test]
fn a_listed_route_with_an_effect_handler_is_refused() {
    for (carrier, ability) in [
        (FROST_SABER, GIWAT),
        (DESRT_LEACH, SAND_STORM),
        (TECH_USER, WAT),
        (JUZA, FOI),
        (RAPPY, ROUND_EYES),
        (BLUE_RAPPY, LOVEL_EYES),
    ] {
        assert!(
            DAMAGE_SKILL_ROUTES
                .iter()
                .any(|route| route.enemy_id == carrier.enemy_id && route.ability == ability),
            "({}, {ability:#04X}) must be a listed route for this control",
            carrier.enemy_id
        );
        let listed = motavia_data(&[carrier], &[ability]);
        let record = EnemySkill {
            effect: 2,
            ..motavia_record(ability)
        };
        let data = listed.with_enemy_skills([record]);
        let mut r = motavia_roster(&data, &carrier);
        let before = r.clone();
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(
            !resolve_damage_skill(
                &mut r,
                id(6),
                ability,
                Some(id(2)),
                &data,
                &mut rolls,
                &mut events
            ),
            "{} {ability:#04X}: effect $02 needs its own handler first",
            carrier.symbol
        );
        assert_eq!(r, before, "{}", carrier.symbol);
        assert_eq!(rolls.drawn(), 0, "{}", carrier.symbol);
        assert!(events.is_empty(), "{}", carrier.symbol);
    }
}
