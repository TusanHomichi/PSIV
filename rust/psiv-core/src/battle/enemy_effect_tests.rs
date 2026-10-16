use super::*;
use crate::battle::{
    Battle, Command, EnemyRecord, FormationEnemy, FormationRecord, PartyMember, RoundOrders,
    SliceRolls, fixtures, status,
};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

/// The records the routes name, exactly as `generated/enemy_skills.json` holds
/// them (byte 2's low nibble is the range).
fn skills() -> Vec<EnemySkill> {
    let skill =
        |id, name: &str, effect, power_stat, target, power, resistance, element| EnemySkill {
            id,
            name: name.into(),
            effect,
            power_stat,
            target,
            power,
            resistance,
            element,
        };
    vec![
        // `$0B` STASISBALL: `1C 01 08 40 01 0D` - STR vs STR, efess.
        skill(11, "STASISBALL", 0x1C, 1, 8, 64, 1, 13),
        // `$28` DORAN: `06 82 09 50 02 0B` - MEN vs MEN, psychic.
        skill(40, "DORAN", 0x06, 0x82, 9, 80, 2, 11),
        // `$29` SEALS: `08 82 09 50 02 0B`.
        skill(41, "SEALS", 0x08, 0x82, 9, 80, 2, 11),
        // `$2D` DEBAN: `0A 82 02 00 00 00` - no resistance selector, no roll.
        skill(45, "DEBAN", 0x0A, 0x82, 2, 0, 0, 0),
        // `$2A` RIMIT: `07 82 09 20 02 0B`.
        skill(42, "RIMIT", 0x07, 0x82, 9, 32, 2, 11),
        // `$2F` VOL: `02 82 08 50 02 0A` - MEN vs MEN, biological, kills.
        skill(47, "VOL", 0x02, 0x82, 8, 80, 2, 10),
        // `$4C` EVIL EYE: `07 02 08 40 02 0B` - no `$80` flag on the selector.
        skill(76, "EVIL EYE", 0x07, 2, 8, 64, 2, 11),
        // `$34` VOICE: `07 01 09 80 02 0B`.
        skill(52, "VOICE", 0x07, 1, 9, 128, 2, 11),
        // `$57` GELUN: `03 82 09 40 02 0B`.
        skill(87, "GELUN", 0x03, 0x82, 9, 64, 2, 11),
        // `$24` POISONMIST: `1B 01 08 50 01 0D` - STR vs STR, efess, threshold 80.
        skill(36, "POISONMIST", 0x1B, 1, 8, 80, 1, 13),
        // `$25` SLEEP GAS: `07 01 09 50 01 0B` - STR vs STR, psychic, threshold 80.
        skill(37, "SLEEP GAS", 0x07, 1, 9, 80, 1, 11),
        // `$4B` SHADOWBIND: `06 02 09 40 02 0B` - MEN vs MEN, psychic, threshold 64.
        skill(75, "SHADOWBIND", 0x06, 2, 9, 64, 2, 11),
    ]
}

fn enemy(enemy_id: u16, strength: u8, mental: u8) -> EnemyRecord {
    let mut record = fixtures::zoran_bult();
    record.id = enemy_id;
    record.name = format!("ENEMY{enemy_id}");
    record.strength = strength;
    record.mental = mental;
    record.regular_abilities = [0; 8];
    record
}

fn data() -> BattleData {
    fixtures::data()
        .with_enemies([
            enemy(76, 40, 1),
            enemy(19, 40, 0),
            enemy(115, 40, 31),
            enemy(106, 1, 1),
            enemy(72, 76, 54),
            enemy(88, 73, 63),
            enemy(70, 20, 18),
            enemy(77, 40, 1),
            enemy(57, 20, 1),
            enemy(63, 20, 1),
            enemy(138, 1, 20),
            enemy(111, 1, 20),
        ])
        .with_enemy_skills(skills())
}

/// A member at the given STR and MEN, normal (2) to every element, so every
/// scale is 2.
fn member(data: &BattleData, strength: u8, mental: u8) -> PartyMember {
    let mut member = PartyMember::seat(&fixtures::chaz(), data).unwrap();
    member.stats.curr_hp = 500;
    member.stats.max_hp = 500;
    member.stats.strength.battle = strength;
    member.stats.mental.battle = mental;
    member.stats.element_props = [2; crate::battle::ELEMENT_SLOTS];
    member
}

fn roster(data: &BattleData, party: usize, enemy_id: u16, enemies: u8) -> Roster {
    let mut r = Roster::new();
    for _ in 0..party {
        let member = member(data, 20, 20);
        r.add_party_member(member.character, member.name, member.stats);
    }
    for slot in 1..=enemies {
        r.add_enemy(slot, data.enemy(enemy_id).unwrap()).unwrap();
    }
    r
}

fn run(
    r: &mut Roster,
    actor: u8,
    ability: u8,
    target: Option<u8>,
    data: &BattleData,
    draws: &[u16],
) -> (EffectTurn, usize, Vec<BattleEvent>) {
    let mut rolls = SliceRolls::new(draws);
    let mut events = Vec::new();
    let turn = resolve_effect_skill(
        r,
        id(actor),
        ability,
        target.map(id),
        data,
        &mut rolls,
        &mut events,
    );
    (turn, rolls.drawn(), events)
}

fn used(actor: u8, skill: u8, name: &str) -> BattleEvent {
    BattleEvent::EnemySkillUsed {
        actor: id(actor),
        skill,
        name: name.into(),
    }
}

#[test]
fn voice_rolls_once_per_eligible_slot_and_only_sets_the_sleep_bit() {
    // `v = (r + STR - MEN) * 2` against the record's 128: STR 40 vs MEN 20 misses
    // up to r = 44 and lands from 45.
    let data = data();
    let mut r = roster(&data, 3, 76, 1);
    let agility = r.get(id(1)).unwrap().stats.agility.battle;
    let (turn, drawn, events) = run(&mut r, 6, 52, None, &data, &[45, 44, 45]);
    assert_eq!(turn, EffectTurn::Resolved);
    assert_eq!(drawn, 3, "one roll per slot, slot order");
    assert_eq!(
        events,
        vec![
            used(6, 52, "VOICE"),
            BattleEvent::StatusInflicted {
                actor: id(6),
                target: id(1),
                status: status::ASLEEP,
            },
            BattleEvent::StatusInflicted {
                actor: id(6),
                target: id(3),
                status: status::ASLEEP,
            },
        ]
    );
    assert_eq!(r.get(id(1)).unwrap().stats.status, status::ASLEEP);
    assert_eq!(r.get(id(2)).unwrap().stats.status, 0, "r = 44 misses");
    assert_eq!(r.get(id(3)).unwrap().stats.status, status::ASLEEP);
    assert_eq!(
        r.get(id(1)).unwrap().stats.agility.battle,
        agility,
        "VOICE's sleep leaves agility alone (loc_25074 is a bare bset)"
    );
}

#[test]
fn voice_skips_the_asleep_the_paralyzed_and_the_dead_before_the_roll() {
    let data = data();
    let mut r = roster(&data, 4, 76, 1);
    r.get_mut(id(1)).unwrap().stats.status = status::ASLEEP;
    r.get_mut(id(2)).unwrap().stats.status = status::PARALYZED;
    r.get_mut(id(3)).unwrap().mark_defeated();
    // `AbilityEffect_SleepParalyze` tests StatusAsleep (bit 3) only: the second
    // sleep bit does not stop the roll.
    r.get_mut(id(4)).unwrap().stats.status = status::ASLEEP_2;
    let (_, drawn, events) = run(&mut r, 6, 52, None, &data, &[63]);
    assert_eq!(drawn, 1, "only the ASLEEP_2 member rolls");
    assert_eq!(
        events,
        vec![
            used(6, 52, "VOICE"),
            BattleEvent::StatusInflicted {
                actor: id(6),
                target: id(4),
                status: status::ASLEEP,
            },
        ]
    );
    assert_eq!(r.get(id(2)).unwrap().stats.status, status::PARALYZED);
    assert_eq!(r.get(id(3)).unwrap().stats.status, status::DEAD);
}

#[test]
fn stasisball_paralyzes_one_target_and_pins_agility_and_dexterity() {
    // STR 40 vs STR 20, scale 2, miss threshold 64: misses up to r = 12.
    let data = data();
    for (roll, landed) in [(12, false), (13, true)] {
        let mut r = roster(&data, 2, 19, 1);
        r.get_mut(id(1)).unwrap().stats.status = status::ASLEEP;
        let (_, drawn, events) = run(&mut r, 6, 11, Some(1), &data, &[roll]);
        assert_eq!(drawn, 1, "r = {roll}");
        let stats = &r.get(id(1)).unwrap().stats;
        if landed {
            assert_eq!(stats.status, status::PARALYZED, "sleep is cleared");
            assert_eq!(stats.agility.battle, 1);
            assert_eq!(stats.dexterity.battle, 1);
            assert_eq!(events.len(), 2);
        } else {
            assert_eq!(stats.status, status::ASLEEP);
            assert_ne!(stats.agility.battle, 1);
            assert_eq!(events, vec![used(6, 11, "STASISBALL")]);
        }
        assert_eq!(
            r.get(id(2)).unwrap().stats.status,
            0,
            "range 8 is the single target"
        );
    }
}

#[test]
fn stasisball_on_a_paralyzed_target_draws_nothing() {
    let data = data();
    let mut r = roster(&data, 1, 19, 1);
    r.get_mut(id(1)).unwrap().stats.status = status::PARALYZED;
    let (_, drawn, events) = run(&mut r, 6, 11, Some(1), &data, &[63]);
    assert_eq!(drawn, 0);
    assert_eq!(events, vec![used(6, 11, "STASISBALL")]);
}

#[test]
fn seals_visits_every_party_slot_and_skips_the_sealed() {
    // MEN 31 vs MEN 20: `v = (r + 11) * 2` against 80 - misses up to r = 29.
    let data = data();
    let mut r = roster(&data, 3, 115, 1);
    r.get_mut(id(2)).unwrap().stats.status = status::TECH_SEALED | status::POISONED;
    let (_, drawn, events) = run(&mut r, 6, 41, None, &data, &[29, 30]);
    assert_eq!(drawn, 2, "the sealed slot spends no roll");
    assert_eq!(
        events,
        vec![
            used(6, 41, "SEALS"),
            BattleEvent::StatusInflicted {
                actor: id(6),
                target: id(3),
                status: status::TECH_SEALED,
            },
        ]
    );
    assert_eq!(r.get(id(1)).unwrap().stats.status, 0, "r = 29 misses");
    assert_eq!(
        r.get(id(2)).unwrap().stats.status,
        status::TECH_SEALED | status::POISONED
    );
    assert_eq!(r.get(id(3)).unwrap().stats.status, status::TECH_SEALED);
}

#[test]
fn doran_sets_agility_from_the_modified_value_with_a_floor_of_one() {
    let data = data();
    let mut r = roster(&data, 2, 115, 1);
    r.get_mut(id(1)).unwrap().stats.agility.modified = 40;
    r.get_mut(id(1)).unwrap().stats.agility.battle = 40;
    r.get_mut(id(2)).unwrap().stats.agility.modified = 31;
    r.get_mut(id(2)).unwrap().stats.agility.battle = 31;
    let (_, drawn, _) = run(&mut r, 6, 40, None, &data, &[63]);
    assert_eq!(drawn, 2);
    assert_eq!(r.get(id(1)).unwrap().stats.agility.battle, 9, "40 - 31");
    assert_eq!(
        r.get(id(2)).unwrap().stats.agility.battle,
        1,
        "an equal value is not above the actor's MEN (`bhi`): the floor"
    );
    // A second cast starts from the modified value again - it does not stack.
    let (_, _, _) = run(&mut r, 6, 40, None, &data, &[63]);
    assert_eq!(r.get(id(1)).unwrap().stats.agility.battle, 9);
}

#[test]
fn gelun_sets_attack_from_the_derived_value_and_floors_at_zero() {
    let data = data();
    let mut r = roster(&data, 2, 115, 1);
    r.get_mut(id(1)).unwrap().stats.attack.derived = 100;
    r.get_mut(id(1)).unwrap().stats.attack.battle = 100;
    r.get_mut(id(2)).unwrap().stats.attack.derived = 20;
    r.get_mut(id(2)).unwrap().stats.attack.battle = 20;
    // Threshold 64: misses up to r = 21.
    let (_, drawn, events) = run(&mut r, 6, 87, None, &data, &[21, 22]);
    assert_eq!(drawn, 2);
    assert_eq!(r.get(id(1)).unwrap().stats.attack.battle, 100, "r = 21");
    assert_eq!(
        r.get(id(2)).unwrap().stats.attack.battle,
        0,
        "20 - 31 clears the word"
    );
    assert_eq!(
        events,
        vec![
            used(6, 87, "GELUN"),
            BattleEvent::StatChanged {
                actor: id(6),
                target: id(2),
                stat: TechniqueStat::Attack,
                value: 0,
            },
        ]
    );
}

#[test]
fn deban_raises_every_living_enemy_from_the_derived_defence_without_a_roll() {
    let data = data();
    let mut r = roster(&data, 1, 70, 3);
    r.get_mut(id(8)).unwrap().mark_defeated();
    let derived = r.get(id(6)).unwrap().stats.defence.derived;
    let (turn, drawn, events) = run(&mut r, 6, 45, None, &data, &[63]);
    assert_eq!(turn, EffectTurn::Resolved);
    assert_eq!(drawn, 0, "the record names no resistance stat");
    assert_eq!(r.get(id(6)).unwrap().stats.defence.battle, derived + 18);
    assert_eq!(r.get(id(7)).unwrap().stats.defence.battle, derived + 18);
    assert_eq!(
        r.get(id(8)).unwrap().stats.defence.battle,
        derived,
        "a dead slot is not visited"
    );
    assert_eq!(
        events.len(),
        3,
        "the skill, then one change per living slot"
    );
}

#[test]
fn shadowsabr_swings_instead_when_its_defence_is_already_raised() {
    let data = data();
    let mut r = roster(&data, 1, 70, 1);
    let (_, _, _) = run(&mut r, 6, 45, None, &data, &[63]);
    r.get_mut(id(6)).unwrap().ability = 45;
    let raised = r.get(id(6)).unwrap().stats.defence.battle;
    let (turn, drawn, events) = run(&mut r, 6, 45, None, &data, &[63]);
    assert_eq!(turn, EffectTurn::Swing);
    assert_eq!(drawn, 0);
    assert!(
        events.is_empty(),
        "loc_F826 loads the ordinary attack objects"
    );
    assert_eq!(r.get(id(6)).unwrap().ability, 0, "`clr.w $24(a4)`");
    assert_eq!(r.get(id(6)).unwrap().stats.defence.battle, raised);
}

#[test]
fn only_the_traced_pairs_and_records_run() {
    let data = data();
    // Enemy 77 shares FlyScreamr's routine but is not a carrier of VOICE.
    let mut r = roster(&data, 1, 77, 1);
    let before = r.clone();
    let (turn, drawn, events) = run(&mut r, 6, 52, None, &data, &[63]);
    assert_eq!((turn, drawn, events.len()), (EffectTurn::NotMine, 0, 0));
    assert_eq!(r, before);

    // A record whose effect byte is not the route's handler is not guessed at.
    let mut wrong = data.enemy_skill(52).unwrap().clone();
    wrong.effect = 0x1C;
    let swapped = data.clone().with_enemy_skills([wrong]);
    let mut r = roster(&swapped, 1, 76, 1);
    let (turn, drawn, _) = run(&mut r, 6, 52, None, &swapped, &[63]);
    assert_eq!((turn, drawn), (EffectTurn::NotMine, 0));

    // Neither is a dead actor.
    let mut r = roster(&data, 1, 76, 1);
    r.get_mut(id(6)).unwrap().mark_defeated();
    let (turn, _, _) = run(&mut r, 6, 52, None, &data, &[63]);
    assert_eq!(turn, EffectTurn::NotMine);
}

fn lone_formation(enemy_id: u16) -> FormationRecord {
    FormationRecord {
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
    }
}

#[test]
fn the_engine_dispatches_voice_without_a_swing_and_wakes_the_sleeper_on_an_odd_draw() {
    let mut voice = enemy(76, 40, 1);
    voice.regular_abilities = [52; 8];
    let data = data().with_enemies([voice]);
    // Every draw is 63: the ability index is 7, VOICE lands (63 >= 45), and the
    // round's end-of-round wake roll (`63 & 1`) is odd.
    let mut rolls = SliceRolls::new(&[63]);
    let (mut battle, _) = Battle::start(
        &lone_formation(76),
        vec![member(&data, 20, 20)],
        &data,
        true,
        &mut rolls,
    )
    .unwrap();
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend]),
            &data,
            &mut rolls,
        )
        .unwrap();
    assert!(events.contains(&used(6, 52, "VOICE")));
    assert!(events.contains(&BattleEvent::StatusInflicted {
        actor: id(6),
        target: id(1),
        status: status::ASLEEP,
    }));
    assert!(events.contains(&BattleEvent::WokeUp { fighter: id(1) }));
    assert!(
        !events.iter().any(
            |e| matches!(e, BattleEvent::Attacked { actor, .. } if *actor == id(6))
                || matches!(e, BattleEvent::UnsupportedAbility { .. })
        ),
        "{events:?}"
    );
    assert_eq!(battle.roster().get(id(1)).unwrap().stats.status, 0);
}

#[test]
fn a_whole_party_of_paralyzed_members_is_a_defeat_but_a_sleeping_one_is_not() {
    // `ps4.asm:9739-9772`: the party loses when every occupied slot carries a
    // bit of `$46` (paralyzed, dead, android-dead). Sleep is not in it.
    let mut stasis = enemy(19, 40, 0);
    stasis.regular_abilities = [11; 8];
    let data = data().with_enemies([stasis]);
    let mut rolls = SliceRolls::new(&[63]);
    let (mut battle, _) = Battle::start(
        &lone_formation(19),
        vec![member(&data, 20, 20)],
        &data,
        true,
        &mut rolls,
    )
    .unwrap();
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend]),
            &data,
            &mut rolls,
        )
        .unwrap();
    assert!(events.contains(&BattleEvent::StatusInflicted {
        actor: id(6),
        target: id(1),
        status: status::PARALYZED,
    }));
    assert_eq!(
        battle.outcome(),
        Some(crate::battle::Outcome::Defeat),
        "the only member is paralyzed with 500 HP left"
    );

    // One paralyzed member out of two keeps the fight going.
    let mut rolls = SliceRolls::new(&[63]);
    let (mut battle, _) = Battle::start(
        &lone_formation(19),
        vec![member(&data, 20, 20), member(&data, 20, 20)],
        &data,
        true,
        &mut rolls,
    )
    .unwrap();
    battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend, Command::Defend]),
            &data,
            &mut rolls,
        )
        .unwrap();
    let paralyzed = battle
        .roster()
        .side(Side::Party)
        .filter(|f| f.stats.status & status::PARALYZED != 0)
        .count();
    assert_eq!(paralyzed, 1);
    assert_eq!(battle.outcome(), None);
}

#[test]
fn a_seal_and_a_sleep_end_with_the_battle_but_poison_and_paralysis_go_out_with_the_party() {
    // `Battle_LastMessage` (`ps4.asm:6350`) clears bits 3 and 4 of every party
    // slot (`andi.b #$E7`, line 6366) before it loads the field.
    let data = data();
    let mut tired = member(&data, 20, 20);
    tired.stats.status =
        status::ASLEEP | status::TECH_SEALED | status::POISONED | status::PARALYZED;
    let mut rolls = SliceRolls::new(&[63]);
    let (battle, _) =
        Battle::start(&lone_formation(19), vec![tired], &data, true, &mut rolls).unwrap();
    let party = battle.into_party();
    assert_eq!(
        party[0].stats.status,
        status::POISONED | status::PARALYZED,
        "asleep and tech-sealed are cleared, nothing else"
    );
}

#[test]
fn a_seal_laid_in_battle_does_not_outlive_it() {
    let mut seals = enemy(115, 40, 31);
    seals.regular_abilities = [41; 8];
    let data = data().with_enemies([seals]);
    let mut rolls = SliceRolls::new(&[63]);
    let (mut battle, _) = Battle::start(
        &lone_formation(115),
        vec![member(&data, 20, 20)],
        &data,
        true,
        &mut rolls,
    )
    .unwrap();
    battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend]),
            &data,
            &mut rolls,
        )
        .unwrap();
    assert_eq!(
        battle.roster().get(id(1)).unwrap().stats.status & status::TECH_SEALED,
        status::TECH_SEALED,
        "sealed during the battle"
    );
    assert_eq!(battle.into_party()[0].stats.status & status::TECH_SEALED, 0);
}

#[test]
fn rimit_sleeps_every_slot_like_voice_but_at_its_own_threshold() {
    // MEN 31 vs MEN 20, `v = (r + 11) * 2` against 32: misses up to r = 5.
    let data = data();
    let mut r = roster(&data, 3, 115, 1);
    let (turn, drawn, events) = run(&mut r, 6, 42, None, &data, &[5, 6, 63]);
    assert_eq!((turn, drawn), (EffectTurn::Resolved, 3));
    assert_eq!(r.get(id(1)).unwrap().stats.status, 0, "r = 5 misses");
    assert_eq!(r.get(id(2)).unwrap().stats.status, status::ASLEEP);
    assert_eq!(r.get(id(3)).unwrap().stats.status, status::ASLEEP);
    assert_eq!(events.len(), 3, "the skill and two sleeps");
}

#[test]
fn evil_eye_sleeps_the_one_target_with_an_unmasked_mental_selector() {
    // Haunt's MEN 1 against MEN 20: `v = (r - 19) * 2` against 64 - misses up to
    // r = 51. Record byte 1 is `$02`, not `$82`; the mask changes nothing.
    let data = data();
    for (roll, asleep) in [(51, false), (52, true)] {
        let mut r = roster(&data, 2, 106, 1);
        let (_, drawn, events) = run(&mut r, 6, 76, Some(2), &data, &[roll]);
        assert_eq!(drawn, 1);
        assert_eq!(r.get(id(1)).unwrap().stats.status, 0, "range 8: one slot");
        assert_eq!(
            r.get(id(2)).unwrap().stats.status,
            if asleep { status::ASLEEP } else { 0 },
            "r = {roll}"
        );
        assert_eq!(events.len(), if asleep { 2 } else { 1 });
    }
}

#[test]
fn vol_kills_on_a_landing_roll_and_keeps_only_the_seal() {
    // Character_Dead keeps the seal and drops the other ailments.
    let data = data();
    let mut r = roster(&data, 2, 72, 1);
    r.get_mut(id(1)).unwrap().stats.status = status::TECH_SEALED | status::POISONED;
    // BloodSaber's MEN 54 against 20: `v = (r + 34) * 2`, a miss up to r = 6.
    let (_, drawn, events) = run(&mut r, 6, 47, Some(1), &data, &[6]);
    assert_eq!((drawn, events.len()), (1, 1), "r = 6 misses");
    assert_eq!(r.get(id(1)).unwrap().stats.curr_hp, 500);
    let (_, drawn, events) = run(&mut r, 6, 47, Some(1), &data, &[7]);
    assert_eq!(drawn, 1);
    let dead = r.get(id(1)).unwrap();
    assert_eq!(dead.stats.curr_hp, 0);
    assert_eq!(dead.stats.status, status::DEAD | status::TECH_SEALED);
    assert_eq!(
        events,
        vec![used(6, 47, "VOL"), BattleEvent::Died { fighter: id(1) }]
    );
    assert_eq!(
        r.get(id(2)).unwrap().stats.curr_hp,
        500,
        "range 8: one slot"
    );
}

#[test]
fn vol_misses_on_biological_immunity_but_still_spends_its_roll_and_skips_the_dead() {
    let data = data();
    let mut r = roster(&data, 2, 72, 1);
    r.get_mut(id(1)).unwrap().stats.element_props[9] = 0;
    let (_, drawn, events) = run(&mut r, 6, 47, Some(1), &data, &[63]);
    assert_eq!(drawn, 1, "the scale is zero but the roll is taken");
    assert_eq!(events.len(), 1);
    assert_eq!(r.get(id(1)).unwrap().stats.curr_hp, 500);

    let mut r = roster(&data, 2, 72, 1);
    r.get_mut(id(1)).unwrap().mark_defeated();
    let (_, drawn, events) = run(&mut r, 6, 47, Some(1), &data, &[63]);
    assert_eq!((drawn, events.len()), (0, 1), "a dead target takes no roll");
}

#[test]
fn an_android_dies_into_the_android_bit() {
    let data = data();
    let mut r = roster(&data, 1, 88, 1);
    r.get_mut(id(1)).unwrap().stats.profession = crate::battle::PROFESSION_ANDROID;
    run(&mut r, 6, 47, Some(1), &data, &[0]);
    assert_eq!(r.get(id(1)).unwrap().stats.status, status::ANDROID_DEAD);
}

#[test]
fn poisonmist_poisons_only_the_drawn_target_at_its_own_threshold() {
    // STR 20 vs STR 20, efess factor 2, threshold 80: `v = 2r` misses up to
    // r = 40 and lands from 41. The record is range 8: one slot, the drawn one.
    let data = data();
    for (roll, landed) in [(40, false), (41, true)] {
        let mut r = roster(&data, 3, 57, 1);
        let (turn, drawn, events) = run(&mut r, 6, 36, Some(2), &data, &[roll]);
        assert_eq!((turn, drawn), (EffectTurn::Resolved, 1), "r = {roll}");
        let mut want = vec![used(6, 36, "POISONMIST")];
        if landed {
            want.push(BattleEvent::StatusInflicted {
                actor: id(6),
                target: id(2),
                status: status::POISONED,
            });
        }
        assert_eq!(events, want, "r = {roll}");
        assert_eq!(
            r.get(id(2)).unwrap().stats.status,
            if landed { status::POISONED } else { 0 }
        );
        assert_eq!(r.get(id(1)).unwrap().stats.status, 0, "not the others");
        assert_eq!(r.get(id(3)).unwrap().stats.status, 0, "not the others");
        assert_eq!(r.get(id(2)).unwrap().stats.curr_hp, 500, "no damage");
    }
}

#[test]
fn poisonmist_on_a_poisoned_target_draws_nothing() {
    // `AbilityEffect_Poison` returns on `btst #StatusPoisoned` before its call.
    let data = data();
    let mut r = roster(&data, 1, 57, 1);
    r.get_mut(id(1)).unwrap().stats.status = status::POISONED;
    let (_, drawn, events) = run(&mut r, 6, 36, Some(1), &data, &[63]);
    assert_eq!(drawn, 0);
    assert_eq!(events, vec![used(6, 36, "POISONMIST")]);
}

#[test]
fn sleep_gas_visits_every_slot_and_sleeps_the_ones_that_land() {
    // STR 20 vs STR 20, psychic factor 2, threshold 80: lands from r = 41, one
    // roll per living slot that is neither asleep nor paralyzed.
    let data = data();
    let mut r = roster(&data, 4, 63, 1);
    r.get_mut(id(4)).unwrap().stats.status = status::PARALYZED;
    let (turn, drawn, events) = run(&mut r, 6, 37, None, &data, &[41, 40, 41]);
    assert_eq!(
        (turn, drawn),
        (EffectTurn::Resolved, 3),
        "the paralyzed rolls none"
    );
    assert_eq!(
        events,
        vec![
            used(6, 37, "SLEEP GAS"),
            BattleEvent::StatusInflicted {
                actor: id(6),
                target: id(1),
                status: status::ASLEEP,
            },
            BattleEvent::StatusInflicted {
                actor: id(6),
                target: id(3),
                status: status::ASLEEP,
            },
        ]
    );
    assert_eq!(r.get(id(2)).unwrap().stats.status, 0, "r = 40 misses");
    assert_eq!(r.get(id(4)).unwrap().stats.status, status::PARALYZED);
}

#[test]
fn shadowbind_sets_agility_from_the_modified_value_on_every_slot_that_lands() {
    // MEN 20 vs MEN 20, psychic factor 2, threshold 64: lands from r = 33. The
    // new battle agility is `modified - MEN` with a floor of one, set from the
    // modified value so a second cast does not stack.
    let data = data();
    for enemy_id in [138, 111] {
        let mut r = roster(&data, 3, enemy_id, 1);
        for (slot, modified) in [(1, 50), (2, 12), (3, 50)] {
            let stats = &mut r.get_mut(id(slot)).unwrap().stats;
            stats.agility.modified = modified;
            stats.agility.battle = modified;
        }
        let (turn, drawn, events) = run(&mut r, 6, 75, None, &data, &[33, 33, 32]);
        assert_eq!((turn, drawn), (EffectTurn::Resolved, 3), "enemy {enemy_id}");
        assert_eq!(
            events,
            vec![
                used(6, 75, "SHADOWBIND"),
                BattleEvent::StatChanged {
                    actor: id(6),
                    target: id(1),
                    stat: TechniqueStat::Agility,
                    value: 30,
                },
                BattleEvent::StatChanged {
                    actor: id(6),
                    target: id(2),
                    stat: TechniqueStat::Agility,
                    value: 1,
                },
            ],
            "enemy {enemy_id}"
        );
        assert_eq!(
            r.get(id(3)).unwrap().stats.agility.battle,
            50,
            "r = 32 misses"
        );
        let (_, _, _) = run(&mut r, 6, 75, None, &data, &[63, 63, 63]);
        assert_eq!(
            r.get(id(1)).unwrap().stats.agility.battle,
            30,
            "a second cast does not stack"
        );
    }
}

#[test]
fn the_zelan_pairs_are_routes_only_for_their_own_carriers() {
    let data = data();
    // 57 Mistralgec's own list does not make SLEEP GAS a route. (58 FlameNewt
    // shares POISONMIST's arm and object; lane A6 captured it and routed it.)
    for (enemy_id, ability) in [(57u16, 37u8), (63, 36), (138, 36), (77, 75)] {
        let mut r = roster(&data, 1, 77, 1);
        r.get_mut(id(6)).unwrap().stats.enemy_id = enemy_id;
        let before = r.clone();
        let (turn, drawn, events) = run(&mut r, 6, ability, Some(1), &data, &[63]);
        assert_eq!(
            (turn, drawn, events.len()),
            (EffectTurn::NotMine, 0, 0),
            "{enemy_id} / {ability}"
        );
        assert_eq!(r, before);
    }
}
