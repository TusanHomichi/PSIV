//! The one `EnemyAttack_FloatMine` outcome that loads nothing: `$07` Fission2 on
//! 50 FloatMine2 and `$17` Waiting on the FloatMine carriers spend the turn.
//!
//! Split out of `enemy_skill_tests.rs` under the repository's 1,000-line rule;
//! included with `#[path]` from `enemy_skill.rs`, like `enemy_skill_poison_tests.rs`.

use super::*;
use crate::battle::{
    Battle, Command, FormationEnemy, FormationRecord, PartyMember, RoundOrders, SliceRolls,
    fixtures, status,
};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

fn kill(r: &mut Roster, n: u8) {
    let f = r.get_mut(id(n)).unwrap();
    f.stats.curr_hp = 0;
    f.stats.status = status::DEAD;
}

/// The three records these tests need, `generated/enemy_skills.json`: `$07`
/// Fission2 (`1e 00 0a 00 00 00 00 00` at `0x28339C`), `$17` Waiting
/// (`22 00 00 00 00 00 00 00` at `0x28341C`) and `$19` Detonation
/// (`24 05 09 18 06 01 00 00` at `0x28342C`), one of `EnemyAttack_FloatMine`'s
/// arms and the id 45 CommndBall really rolls. The tuple is
/// `(effect, power_stat, target, power, resistance, element)`.
fn record(id: u8, name: &str, bytes: (u8, u8, u8, u8, u8, u8)) -> EnemySkill {
    let (effect, power_stat, target, power, resistance, element) = bytes;
    EnemySkill {
        id,
        name: name.into(),
        effect,
        power_stat,
        target,
        power,
        resistance,
        element,
    }
}

fn fission2_record() -> EnemySkill {
    record(7, "FISSION", (30, 0, 10, 0, 0, 0))
}

fn waiting_record() -> EnemySkill {
    record(23, "WAITING", (34, 0, 0, 0, 0, 0))
}

fn detonation_record() -> EnemySkill {
    record(25, "DETONATION", (36, 5, 9, 24, 6, 1))
}

/// One lone `EnemyAttack_FloatMine` carrier with `slots` in all eight regular
/// entries, agile enough to act first, with the record table the caller names.
fn float_mine_data(enemy_id: u16, slots: [u8; 8], skills: Vec<EnemySkill>) -> BattleData {
    let mut carrier = fixtures::zoran_bult();
    carrier.id = enemy_id;
    carrier.hp = 300;
    carrier.agility = 100;
    carrier.regular_abilities = slots;
    carrier.condition_ids = [0; 4];
    fixtures::data()
        .with_enemies([carrier])
        .with_enemy_skills(skills)
}

/// One round against that lone carrier, on a fully specified draw stream: nine
/// ordering draws, the four enemy-target draws, then the ability index, slot 0.
/// Party agility 1 keeps the enemy first in the queue, so the round's roll count
/// is the assertion; 400 party HP keeps a fallback swing from ending it early.
fn float_mine_round(
    enemy_id: u16,
    slots: [u8; 8],
    skills: Vec<EnemySkill>,
) -> (Battle, Vec<BattleEvent>, usize) {
    let data = float_mine_data(enemy_id, slots, skills);
    let formation = FormationRecord {
        id: 0,
        ambush_chance: 0,
        run_chance: 254,
        drop_rate: 0,
        drop_item: None,
        enemies: vec![FormationEnemy {
            slot: 1,
            enemy_id,
            position: 20,
        }],
    };
    let party = [fixtures::alys(), fixtures::chaz(), fixtures::hahn()].map(|record| {
        let mut member = PartyMember::seat(&record, &data).unwrap();
        member.stats.curr_hp = 400;
        member.stats.max_hp = 400;
        member.stats.defence.battle = 7;
        member.stats.agility.battle = 1;
        member
    });
    let (mut battle, _) = Battle::start(
        &formation,
        party.to_vec(),
        &data,
        false,
        &mut SliceRolls::new(&[0]),
    )
    .unwrap();
    let mut stream = vec![0; 13];
    stream.push(0);
    stream.extend([0; 40]);
    let mut rolls = SliceRolls::new(&stream);
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend; 3]),
            &data,
            &mut rolls,
        )
        .unwrap();
    (battle, events, rolls.drawn())
}

/// `EnemyAttack_FloatMine` (`ps4.asm:22675`) has an arm for `$14` Warning, `$18`
/// Explosion, `$19` Detonation and `$1A` CyanicBomb only, so FloatMine2's whole
/// regular list (`$07`/`$17`) and FloatMine's and VopalSphre's (`$17`) reach the
/// fall-through `loc_10406` (`ps4.asm:22781`): it loads no object, clears
/// `Current_Target_Index` and `$24(a4)` and sets `Battle_Routine` `$16`. Every
/// `Fighters_Hit_Flags` entry stays `$FF`, so `Battle_DoAttackEffect`
/// (`ps4.asm:8553`) never reaches `Ability_GetEffectAndRange`: no object, no
/// sound, no damage, no status. The actor has acted and nothing happened.
#[test]
fn float_mine_carriers_spend_the_roll_without_an_effect_or_a_swing() {
    for (carrier, slots, ability, name) in [
        (50u16, [7u8; 8], 7u8, "FISSION"),
        (44, [23; 8], 23, "WAITING"),
        (46, [23; 8], 23, "WAITING"),
    ] {
        let (battle, events, drawn) =
            float_mine_round(carrier, slots, vec![fission2_record(), waiting_record()]);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, BattleEvent::EnemyAbilityWasted { .. }))
                .count(),
            1,
            "carrier {carrier}: {events:?}"
        );
        assert!(
            events.contains(&BattleEvent::EnemyAbilityWasted {
                actor: id(6),
                ability,
                name: name.into(),
            }),
            "carrier {carrier}: {events:?}"
        );
        assert!(
            !events.iter().any(|e| matches!(
                e,
                BattleEvent::UnsupportedAbility { .. }
                    | BattleEvent::Attacked { .. }
                    | BattleEvent::Resolved { .. }
                    | BattleEvent::TurnSkipped { .. }
            )),
            "carrier {carrier}: the turn is spent, neither swung nor skipped: {events:?}"
        );
        assert_eq!(
            drawn, 14,
            "carrier {carrier}: nine ordering draws, four enemy-target draws and \
             the ability index; the cartridge takes no hit roll and no damage draw"
        );
        assert!(
            battle.party_stats().all(|(_, stats)| stats.curr_hp == 400),
            "carrier {carrier}: nobody was touched"
        );
        assert_eq!(
            battle.roster().get(id(6)).unwrap().ability,
            0,
            "carrier {carrier}"
        );
    }
}

/// The negative control: the routine's own `$19` arm on its fourth carrier, and
/// the same `$07` record on an enemy outside every `EnemyAttack_FloatMine`
/// entry, both keep the ordinary fallback — an `UnsupportedAbility` notice, a
/// physical swing, its accuracy roll and the 16 damage draws.
#[test]
fn other_abilities_and_carriers_keep_the_physical_fallback() {
    for (carrier, slots, ability, skills) in [
        (45u16, [25u8; 8], 25u8, vec![detonation_record()]),
        (10, [7; 8], 7, vec![fission2_record()]),
    ] {
        let (_, events, drawn) = float_mine_round(carrier, slots, skills);
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::UnsupportedAbility { actor, ability: rolled }
                    if *actor == id(6) && *rolled == ability
            )),
            "carrier {carrier}: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BattleEvent::Attacked { actor, .. } if *actor == id(6))),
            "carrier {carrier}: the ordinary attack path is the fallback: {events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, BattleEvent::EnemyAbilityWasted { .. })),
            "carrier {carrier}: {events:?}"
        );
        assert_eq!(
            drawn, 31,
            "carrier {carrier}: the fallback adds its accuracy roll and the 16 damage draws"
        );
    }
}

/// The witness itself: the traced record and a carrier of the routine both have
/// to hold, and on a miss it leaves the roster and the event list untouched.
#[test]
fn the_no_effect_witness_needs_the_traced_record_and_carrier() {
    let data = float_mine_data(50, [7; 8], vec![fission2_record(), waiting_record()]);
    let mut r = Roster::new();
    r.add_enemy(1, data.enemy(50).unwrap());
    let mut events = Vec::new();
    assert!(resolve_no_effect_turn(&mut r, id(6), 7, &data, &mut events));
    assert_eq!(
        events,
        vec![BattleEvent::EnemyAbilityWasted {
            actor: id(6),
            ability: 7,
            name: "FISSION".into(),
        }]
    );
    assert_eq!(r.get(id(6)).unwrap().ability, 0);

    // A record whose bytes moved cannot pass for the traced one.
    for skill in [
        EnemySkill {
            target: 9,
            ..fission2_record()
        },
        EnemySkill {
            effect: 30,
            ..waiting_record()
        },
    ] {
        let data = float_mine_data(50, [skill.id; 8], vec![skill.clone()]);
        let mut r = Roster::new();
        r.add_enemy(1, data.enemy(50).unwrap());
        let before = r.clone();
        let mut events = Vec::new();
        assert!(
            !resolve_no_effect_turn(&mut r, id(6), skill.id, &data, &mut events),
            "{skill:?}"
        );
        assert_eq!(r, before, "{skill:?}");
        assert!(events.is_empty(), "{skill:?}");
    }

    // The carrier gate is the enemy's record id, not its slot or its list.
    for (enemy_id, dead) in [(51u16, false), (50, true)] {
        r.get_mut(id(6)).unwrap().stats.enemy_id = enemy_id;
        if dead {
            kill(&mut r, 6);
        }
        assert!(!resolve_no_effect_turn(
            &mut r,
            id(6),
            7,
            &data,
            &mut events
        ));
    }
    assert_eq!(events.len(), 1, "a missed witness adds no event");
}
