//! The Motavia single-target pairs: the record numbers each `(enemy, ability)`
//! pair produces, the stat selectors and widths they read, and one
//! engine-driven round of all 21 of them.
//!
//! Each number is pinned twice — once as the literal `generated/enemies.json`
//! and `Battle_CalculateDamage` (`ps4.asm:17374`) give it, and once through
//! [`record_damage`], the formula written out — so a change to either the
//! fixtures or the cartridge reading shows up as a failing pair.

use super::tests::*;
use super::*;
use crate::battle::{
    Battle, Command, FormationEnemy, FormationRecord, PartyMember, RoundOrders, SliceRolls,
    fixtures, technique,
};
use crate::battle::{ELEMENT_SLOTS, STAT_INDEX_MASK};

/// Resolves one listed pair once and pins the request it makes: one damage
/// roll, an `EnemySkillUsed` and a `Resolved` — and nothing else. The party
/// fixture's defense is 7 and its magic defense 0, so a record selecting `$06`
/// subtracts 7 and one selecting `$07` subtracts 0.
fn resolve_pair(carrier: &Carrier, ability: u8) -> u16 {
    let data = motavia_data(&[*carrier], &[ability]);
    let mut r = motavia_roster(&data, carrier);
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(
        resolve_damage_skill(
            &mut r,
            id(6),
            ability,
            Some(id(2)),
            &data,
            &mut rolls,
            &mut events
        ),
        "{} {ability:#04X} is a listed route",
        carrier.symbol
    );
    assert_eq!(rolls.drawn(), 16, "{}: one damage request", carrier.symbol);
    assert_eq!(
        events.len(),
        2,
        "{}: ability and damage only: {events:?}",
        carrier.symbol
    );
    assert!(
        matches!(
            events[0],
            BattleEvent::EnemySkillUsed { actor, skill, .. }
                if actor == id(6) && skill == ability
        ),
        "{}: {events:?}",
        carrier.symbol
    );
    let BattleEvent::Resolved {
        actor,
        target,
        verdict,
        damage: Some(damage),
        remaining_hp,
    } = events[1]
    else {
        panic!("{}: {events:?}", carrier.symbol);
    };
    assert_eq!(actor, id(6), "{}", carrier.symbol);
    assert_eq!(target, id(2), "{}", carrier.symbol);
    assert_eq!(
        verdict,
        crate::battle::Verdict::Normal,
        "{}",
        carrier.symbol
    );
    assert_eq!(remaining_hp, 400 - damage, "{}", carrier.symbol);
    assert_eq!(
        r.get(id(1)).unwrap().stats.curr_hp,
        400,
        "{}",
        carrier.symbol
    );
    assert_eq!(
        r.get(id(3)).unwrap().stats.curr_hp,
        400,
        "{}",
        carrier.symbol
    );
    assert_eq!(
        r.get(id(2)).unwrap().stats.curr_hp,
        400 - damage,
        "{}",
        carrier.symbol
    );
    damage
}

/// `$2E` GIWAT: record byte 1 is `$82`. `Enemy_DamageCharacter` (line 3798)
/// and `Effect_SetupSkillParams` (line 9580) mask it with `$7F` before indexing
/// their stat table, so it is selector 2 (mental) — not selector 0, which
/// would deal `record_damage(0, ..)` — and each carrier's own mental stat
/// decides the number.
#[test]
fn giwat_reads_the_masked_mental_selector_for_every_carrier() {
    for (carrier, mental, expected) in [
        (FROST_SABER, 45u16, 113u16),
        (TECH_PLANT, 27, 103),
        (HEW_GILLA, 79, 132),
        (DARK_WITCH, 76, 130),
        (DELM_LARS, 45, 113),
        (XE_ATHOUL, 118, 154),
    ] {
        assert_eq!(motavia_record(GIWAT).power_stat, 0x82);
        assert_eq!(
            resolve_pair(&carrier, GIWAT),
            expected,
            "{}",
            carrier.symbol
        );
        assert_eq!(
            expected,
            record_damage(mental, 0, 2, 88),
            "{}: the formula's own terms",
            carrier.symbol
        );
        assert_ne!(
            expected,
            record_damage(0, 0, 2, 88),
            "{}: $82 masked is selector 2, not 0",
            carrier.symbol
        );
    }
}

/// `$37` SAND STORM: the record's byte 2 is 9 and its byte 1 is `$05`. Nibble 9
/// is `AbilityRange_MultiChars`, which only multiplies the no-op effect
/// handler of `AbilityEffect_None`; the chain makes one request. Selector 5 is
/// `attack`, which both readers take as a **word** — 286 here, so a byte read
/// would produce a different number.
#[test]
fn sand_storm_reads_the_attack_word_on_a_nibble_9_record() {
    assert_eq!(motavia_record(SAND_STORM).target, 9);
    let damage = resolve_pair(&DESRT_LEACH, SAND_STORM);
    assert_eq!(damage, 249);
    assert_eq!(damage, record_damage(286, 7, 2, 96), "attack is a word");
    assert_ne!(
        damage,
        record_damage(286 & 0xFF, 7, 2, 96),
        "a byte read of attack 286 would give a different number"
    );
}

#[test]
fn maelstrom_reads_the_attack_word_on_a_nibble_9_record() {
    assert_eq!(motavia_record(MAELSTROM).target, 9);
    assert_eq!(resolve_pair(&LEVIATHAN, MAELSTROM), 166);
    assert_eq!(166, record_damage(252, 7, 2, 32));
}

#[test]
fn flodbreath_reads_its_carriers_attack_word() {
    for (carrier, attack, expected) in [
        (DEPCEN, 101u16, 69u16),
        (HEW_GILLA, 154, 99),
        (ELMELEW, 184, 116),
    ] {
        assert_eq!(
            resolve_pair(&carrier, FLODBREATH),
            expected,
            "{}",
            carrier.symbol
        );
        assert_eq!(
            expected,
            record_damage(attack, 7, 2, 20),
            "{}: the formula's own terms",
            carrier.symbol
        );
    }
}

#[test]
fn wat_reads_the_masked_mental_selector_for_every_carrier() {
    for (carrier, mental, expected) in [
        (HEW_GILLA, 79u16, 68u16),
        (ELMELEW, 59, 57),
        (TECH_USER, 25, 38),
        (TECH_MASTER, 38, 45),
        (JUZA, 30, 40),
    ] {
        assert_eq!(resolve_pair(&carrier, WAT), expected, "{}", carrier.symbol);
        assert_eq!(
            expected,
            record_damage(mental, 0, 2, 24),
            "{}: the formula's own terms",
            carrier.symbol
        );
    }
}

#[test]
fn foi_reads_the_masked_mental_selector_for_every_carrier() {
    for (carrier, mental, expected) in [
        (TECH_USER, 25u16, 34u16),
        (TECH_MASTER, 38, 41),
        (JUZA, 30, 36),
    ] {
        assert_eq!(resolve_pair(&carrier, FOI), expected, "{}", carrier.symbol);
        assert_eq!(
            expected,
            record_damage(mental, 0, 2, 20),
            "{}: the formula's own terms",
            carrier.symbol
        );
    }
}

/// `$6D` ROUND EYES: selector 1 (strength, a byte) and power byte 0, so the
/// number is the carrier's strength alone against the target's defense.
#[test]
fn round_eyes_reads_strength_with_a_zero_power_byte() {
    assert_eq!(motavia_record(ROUND_EYES).power, 0);
    assert_eq!(resolve_pair(&RAPPY, ROUND_EYES), 12);
    assert_eq!(12, record_damage(34, 7, 2, 0));
}

#[test]
fn lovel_eyes_reads_the_attack_word() {
    assert_eq!(resolve_pair(&BLUE_RAPPY, LOVEL_EYES), 128);
    assert_eq!(128, record_damage(184, 7, 2, 32));
}

/// The stat widths `Enemy_DamageCharacter` (`ps4.asm:3775`) and
/// `Effect_SetupSkillParams` (`ps4.asm:9576`) read: their tables
/// (`loc_275A` `ps4.asm:3892`, `AbilityStatsOffs` `ps4.asm:9619`) hold
/// `atk_pow_battle`, `dfs_pow_battle` and `magic_dfs_battle` as words —
/// `$26`, `$2A`, `$2E` (`ps4.constants.asm:30`) — and compare the offset
/// against `$26` to pick `move.w`; selectors `$01`..`$04` stay bytes.
#[test]
fn the_stat_selectors_read_a_word_for_attack_defense_and_magic_defense() {
    let data = motavia_data(&[DESRT_LEACH], &[SAND_STORM]);
    let mut r = motavia_roster(&data, &DESRT_LEACH);
    let stats = &mut r.get_mut(id(1)).unwrap().stats;
    stats.attack.battle = 320;
    stats.defence.battle = 0x123;
    stats.mental_defence.battle = 0x1F4;
    stats.strength.battle = 200;
    stats.mental.battle = 201;
    stats.agility.battle = 202;
    stats.dexterity.battle = 203;
    assert_eq!(technique::stat(stats, 5), 320, "attack is the whole word");
    assert_eq!(technique::stat(stats, 6), 0x123, "defense is a word");
    assert_eq!(technique::stat(stats, 7), 0x1F4, "magic defense is a word");
    assert_eq!(technique::stat(stats, 1), 200);
    assert_eq!(technique::stat(stats, 2), 201);
    assert_eq!(technique::stat(stats, 3), 202);
    assert_eq!(technique::stat(stats, 4), 203);
    // Byte 1 arrives raw from the record: `$82` is selector 2 only after the
    // mask, and the mask is the caller's.
    assert_eq!(technique::stat(stats, 0x82), 0, "no such selector");
    assert_eq!(technique::stat(stats, 0x82 & STAT_INDEX_MASK), 201);
    assert_eq!(STAT_INDEX_MASK, 0x7F);
}

/// One round against one carrier, driven through `roll_enemy_ability` on a
/// fully specified draw stream: nine ordering draws, the four enemy-target
/// draws, the ability index (0, and every slot holds the ability) and the 16
/// damage draws — 30 in all when the ability resolves, with no swing. Party
/// agility 1 keeps the enemy first in the queue, so Defend has raised nobody's
/// resistance yet.
fn motavia_round(carrier: &Carrier, ability: u8) -> (Vec<BattleEvent>, usize) {
    // Every regular slot holds the ability under test, so the index roll lands
    // on it. The real list above is what the gate is proven against; a zero
    // slot here would dispatch the plain attack instead.
    let carrier = &Carrier {
        abilities: [ability; 8],
        ..*carrier
    };
    let data = motavia_data(&[*carrier], &[ability]);
    let formation = FormationRecord {
        id: 0,
        ambush_chance: 0,
        run_chance: 254,
        drop_rate: 0,
        drop_item: None,
        enemies: vec![FormationEnemy {
            slot: 1,
            enemy_id: carrier.enemy_id,
            position: 20,
        }],
    };
    let party = [fixtures::alys(), fixtures::chaz(), fixtures::hahn()].map(|record| {
        let mut member = PartyMember::seat(&record, &data).unwrap();
        member.stats.curr_hp = 400;
        member.stats.max_hp = 400;
        member.stats.agility.battle = 1;
        member.stats.element_props = [2; ELEMENT_SLOTS];
        member
    });
    let (mut battle, _) = Battle::start(
        &formation,
        party.to_vec(),
        &data,
        false,
        7,
        // `$FFFFEEA8` loads at zero (`GameMode_LoadBattle`'s page wipe,
        // ps4.asm:9992-9994), so a battle whose first ability draw is zero
        // re-rolls it (ps4.asm:19146). This rig wants index zero, so the word
        // has to start elsewhere for that draw to stand.
        &mut SliceRolls::new(&[0]),
    )
    .unwrap();
    let mut stream = vec![0; 13];
    stream.push(0);
    stream.extend([0; 16]);
    let mut rolls = SliceRolls::new(&stream);
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend; 3]),
            &data,
            &mut rolls,
        )
        .unwrap();
    (events, rolls.drawn())
}

/// Every added route is reachable through the engine's own ability roll, not
/// just through a direct call: the ability resolves, the caster never swings,
/// and no other ability of the same carrier's list gets in the way.
#[test]
fn every_motavia_route_resolves_in_an_ordinary_round() {
    for (carrier, ability) in [
        (FROST_SABER, GIWAT),
        (TECH_PLANT, GIWAT),
        (HEW_GILLA, GIWAT),
        (DARK_WITCH, GIWAT),
        (DELM_LARS, GIWAT),
        (XE_ATHOUL, GIWAT),
        (DESRT_LEACH, SAND_STORM),
        (LEVIATHAN, MAELSTROM),
        (DEPCEN, FLODBREATH),
        (HEW_GILLA, FLODBREATH),
        (ELMELEW, FLODBREATH),
        (HEW_GILLA, WAT),
        (ELMELEW, WAT),
        (TECH_USER, WAT),
        (TECH_MASTER, WAT),
        (JUZA, WAT),
        (TECH_USER, FOI),
        (TECH_MASTER, FOI),
        (JUZA, FOI),
        (RAPPY, ROUND_EYES),
        (BLUE_RAPPY, LOVEL_EYES),
    ] {
        let (events, drawn) = motavia_round(&carrier, ability);
        assert_eq!(
            drawn, 30,
            "{} {ability:#04X}: one damage request, no swing",
            carrier.symbol
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::EnemySkillUsed { actor, skill, .. }
                    if *actor == id(6) && *skill == ability
            )),
            "{} {ability:#04X}: {events:?}",
            carrier.symbol
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::Resolved { actor, damage: Some(_), .. } if *actor == id(6)
            )),
            "{} {ability:#04X}: {events:?}",
            carrier.symbol
        );
        assert!(
            !events.iter().any(|e| matches!(
                e,
                BattleEvent::Attacked { .. } | BattleEvent::UnsupportedAbility { .. }
            )),
            "{} {ability:#04X}: dispatched, so nothing falls back: {events:?}",
            carrier.symbol
        );
    }
}
