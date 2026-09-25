//! All-party damage routes: Fanbite's `$08` SPIRAL BLD and the two `$38`
//! EARTHQUAKE carriers, through [`resolve_damage_skill`] and through
//! `roll_enemy_ability`.
//!
//! The class they prove is [`DamageClass::AllParty`]: one `move.w #$C` per
//! party slot, in slot order, in one frame. What each test pins is the part of
//! that reading the cartridge's own instructions decide —
//!
//! - **slot set**: `loc_B6A2` (`ps4.asm:17492`) fills `Fighters_Hit_Flags`
//!   after the arm cleared `Current_Target_Index`, `loc_B75A` (`ps4.asm:17559`)
//!   writes `$FF` for an out slot, and `Fighter_TakeDamage` (`ps4.asm:3564`)
//!   drops the request on a negative flag;
//! - **order and draws**: `Battle_UpdateFighters` (`ps4.asm:987`) dispatches
//!   the five `$C` routines in `Obj_Fighters` order in one frame, each drawing
//!   its own sixteen [`crate::battle::calculate_damage`] rolls;
//! - **deaths**: hit points come off in `FighterShowDamage_DecreaseHP`
//!   (`ps4.asm:3640`), after every roll has been drawn, so a target that dies
//!   cannot truncate the sequence.

use super::tests::*;
use super::*;
use crate::battle::{
    Battle, Command, ELEMENT_SLOTS, EnemySkill, FormationEnemy, FormationRecord, PartyMember,
    RoundOrders, SliceRolls, fixtures,
};

/// The three carriers of `docs/battle/ENEMY_DAMAGE_ROUTES.md` §2's all-party rows,
/// with the stat line `generated/enemies.json` gives them. `Enemy_DamageCharacter`
/// (`ps4.asm:3775`) reads the caster's stat through record byte 1, so each
/// expected number below is that carrier's own attack word.
#[derive(Clone, Copy)]
struct Carrier {
    enemy_id: u16,
    symbol: &'static str,
    hp: u16,
    strength: u8,
    mental: u8,
    attack: u16,
    abilities: [u8; 8],
}

const FANBITE: Carrier = Carrier {
    enemy_id: 15,
    symbol: "Fanbite",
    hp: 261,
    strength: 32,
    mental: 9,
    attack: 111,
    abilities: [0, 0, 0, 0, 0, 0, SPIRAL_BLD, SPIRAL_BLD],
};
const SAND_WORM: Carrier = Carrier {
    enemy_id: 80,
    symbol: "SandWorm",
    hp: 1440,
    strength: 95,
    mental: 20,
    attack: 279,
    abilities: [0, 0, 0, 0, 0, EARTHQUAKE, EARTHQUAKE, EARTHQUAKE],
};
const KING_RAPPY: Carrier = Carrier {
    enemy_id: 149,
    symbol: "KingRappy",
    hp: 2911,
    strength: 90,
    mental: 33,
    attack: 244,
    abilities: [0, 0, EARTHQUAKE, EARTHQUAKE, 0, 0, 0, 0],
};

/// The one `EnemySkillData` record each all-party pair runs, as
/// `generated/enemy_skills.json` decodes it: `01 05 09 00 06 01 00 00` at
/// `$2833A4` for `$08` and `$283524` for `$38`. Selector 5 is `attack`, which
/// both readers take as a **word**; byte 3 is zero, so the record adds no
/// bonus before the element multiply.
fn all_party_record(ability: u8) -> EnemySkill {
    let name = match ability {
        SPIRAL_BLD => "SPIRAL BLD",
        EARTHQUAKE => "EARTHQUAKE",
        other => panic!("no all-party record for {other:#04X}"),
    };
    EnemySkill {
        id: ability,
        name: name.into(),
        effect: 1,
        power_stat: 0x05,
        target: 9,
        power: 0,
        resistance: 0x06,
        element: 1,
    }
}

fn carrier_record(carrier: &Carrier) -> crate::battle::EnemyRecord {
    let mut record = fixtures::zoran_bult();
    record.id = carrier.enemy_id;
    record.name = carrier.symbol.into();
    record.hp = carrier.hp;
    record.strength = carrier.strength;
    record.mental = carrier.mental;
    record.attack = carrier.attack;
    record.agility = 100;
    record.regular_abilities = carrier.abilities;
    record.condition_ids = [0; 4];
    record
}

/// Carrier records plus the record the pair under test runs.
fn all_party_data(carriers: &[Carrier], abilities: &[u8]) -> BattleData {
    fixtures::data()
        .with_enemies(carriers.iter().map(carrier_record))
        .with_enemy_skills(abilities.iter().map(|ability| all_party_record(*ability)))
}

/// Three party members with three different defense words and three different
/// physical element factors, so a number below can only come from the target's
/// own stats — the `move.b (a1,d2.w), d2` / `move.b (a1,d3.w), d3` reads of
/// `Enemy_DamageCharacter`'s enemy-skill branch (`ps4.asm:3775`), whose `a1` is
/// the *taking* fighter's stats. Slot 4 and 5 stay empty, which is the
/// five-slot loop's other case.
fn all_party_roster(data: &BattleData, carrier: &Carrier) -> Roster {
    let mut r = Roster::new();
    for (record, defence, physical) in [
        (fixtures::alys(), 7u16, 2u8),
        (fixtures::chaz(), 12, 3),
        (fixtures::hahn(), 3, 2),
    ] {
        let mut member = PartyMember::seat(&record, data).unwrap();
        member.stats.curr_hp = 400;
        member.stats.max_hp = 400;
        member.stats.agility.battle = 255;
        member.stats.defence.battle = defence;
        member.stats.mental_defence.battle = 0;
        member.stats.element_props = [2; ELEMENT_SLOTS];
        member.stats.element_props[0] = physical;
        r.add_party_member(member.character, member.name, member.stats);
    }
    r.add_enemy(1, data.enemy(carrier.enemy_id).unwrap());
    r
}

/// The three numbers `record_damage` gives this roster for one carrier: slot
/// 1 defense 7 factor 2, slot 2 defense 12 factor 3, slot 3 defense 3 factor 2.
fn expected_triple(carrier: &Carrier) -> [u16; 3] {
    [
        record_damage(carrier.attack, 7, 2, 0),
        record_damage(carrier.attack, 12, 3, 0),
        record_damage(carrier.attack, 3, 2, 0),
    ]
}

/// What a carrier's own object chain takes off the stream before its damage
/// request, as `enemy_damage::ObjectDraws` records it for the route.
///
/// The two `$38` carriers differ here and nowhere else. SandWorm's arm loads
/// `BattleObj_Earthquake` (`ps4.asm:47884`), whose state 4 shake calls
/// `UpdateRNGSeed2` twice on every fourth frame of its sixty (`loc_2438E`,
/// `ps4.asm:47972`, the calls at lines 47976 and 47983). KingRappy's loads
/// `BattleObj_KingRappyEarthquake` (`ps4.asm:67513`), whose state 4
/// (`loc_344AC`, line 67554) shakes the camera from a fixed byte table
/// (`loc_3451C`, line 67590) and calls nothing; Fanbite's
/// `BattleObj_LocustaSpiralBld` (`ps4.asm:29175`) calls nothing either.
fn object_draws(carrier: &Carrier) -> usize {
    if carrier.enemy_id == SAND_WORM.enemy_id {
        EARTHQUAKE_SHAKE_DRAWS as usize
    } else {
        0
    }
}

/// Resolves one listed pair once and pins the request it makes: one
/// `EnemySkillUsed`, then one `Resolved` per living party slot in slot order,
/// sixteen draws each, plus the calls the carrier's own object chain takes
/// before them ([`object_draws`]), and nothing else.
fn resolve_all_party(carrier: &Carrier, ability: u8, intended: Option<FighterId>) -> Vec<u16> {
    let data = all_party_data(&[*carrier], &[ability]);
    let mut r = all_party_roster(&data, carrier);
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(
        resolve_damage_skill(
            &mut r,
            id(6),
            ability,
            intended,
            &data,
            &mut rolls,
            &mut events
        ),
        "{} {ability:#04X} is a listed route",
        carrier.symbol
    );
    assert_eq!(
        rolls.drawn(),
        48 + object_draws(carrier),
        "{}: three living slots, one damage request each",
        carrier.symbol
    );
    assert!(
        matches!(
            events.first(),
            Some(BattleEvent::EnemySkillUsed { actor, skill, name })
                if *actor == id(6) && *skill == ability && name == &all_party_record(ability).name
        ),
        "{}: {events:?}",
        carrier.symbol
    );
    let mut damage = Vec::new();
    for (index, event) in events.iter().enumerate().skip(1) {
        let BattleEvent::Resolved {
            actor,
            target,
            verdict,
            damage: Some(d),
            remaining_hp,
        } = event
        else {
            panic!("{}: {events:?}", carrier.symbol);
        };
        assert_eq!(*actor, id(6), "{}", carrier.symbol);
        assert_eq!(*target, id(index as u8), "{}: slot order", carrier.symbol);
        assert_eq!(
            *verdict,
            crate::battle::Verdict::Normal,
            "{}",
            carrier.symbol
        );
        assert_eq!(*remaining_hp, 400 - d, "{}", carrier.symbol);
        damage.push(*d);
    }
    assert_eq!(events.len(), 4, "{}: {events:?}", carrier.symbol);
    assert_eq!(damage.len(), 3, "{}: {events:?}", carrier.symbol);
    damage
}

/// The five-slot loop writes every occupied party slot, but `Fighter_TakeDamage`
/// only computes for a slot whose hit flag is non-negative, and `loc_B75A` wrote
/// that `$FF` for a dead one. Empty slots 4 and 5 have no fighter object at all,
/// so `Battle_UpdateFighters` skips them before the routine could run.
#[test]
fn spiral_bld_hits_every_living_party_slot_with_its_own_roll_and_order() {
    assert_eq!(
        all_party_record(SPIRAL_BLD).target,
        9,
        "the record's nibble"
    );
    let damage = resolve_all_party(&FANBITE, SPIRAL_BLD, None);
    assert_eq!(damage, [55, 81, 59]);
    assert_eq!(damage, expected_triple(&FANBITE));
    assert_eq!(
        damage,
        [
            record_damage(111, 7, 2, 0),
            record_damage(111, 12, 3, 0),
            record_damage(111, 3, 2, 0)
        ]
    );
    assert_ne!(damage[0], damage[1]);
    assert_ne!(damage[1], damage[2]);
}

/// `intended` is the drawn `Current_Target_Index`, and Fanbite's arm cleared it
/// at `ps4.asm:23488` before loading `BattleObj_LocustaSpiralBld`; the request
/// loop runs over `Obj_Fighters` itself. Whether the engine hands the resolver a
/// target, or which one, cannot narrow the five writes.
#[test]
fn the_drawn_target_does_not_narrow_the_all_party_loop() {
    for intended in [None, Some(id(1)), Some(id(2)), Some(id(3))] {
        let damage = resolve_all_party(&FANBITE, SPIRAL_BLD, intended);
        assert_eq!(damage, [55, 81, 59], "intended {intended:?}");
    }
}

/// The dead slot is written `$C` like the others — the loop does not test
/// anything — but its hit flag was `$FF`, so `Fighter_TakeDamage` sets routine 4
/// and returns before `Figher_DamageCheckActor`. No number, no draw, no event.
#[test]
fn a_dead_party_slot_takes_no_hit_and_draws_nothing() {
    let data = all_party_data(&[FANBITE], &[SPIRAL_BLD]);
    let mut r = all_party_roster(&data, &FANBITE);
    kill(&mut r, 2);
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(resolve_damage_skill(
        &mut r,
        id(6),
        SPIRAL_BLD,
        Some(id(2)),
        &data,
        &mut rolls,
        &mut events
    ));
    assert_eq!(rolls.drawn(), 32, "two living slots, sixteen draws each");
    assert_eq!(events.len(), 3, "{events:?}");
    assert!(matches!(
        events[0],
        BattleEvent::EnemySkillUsed {
            skill: SPIRAL_BLD,
            ..
        }
    ));
    for (event, target) in events[1..].iter().zip([id(1), id(3)]) {
        assert!(
            matches!(event, BattleEvent::Resolved { target: hit, damage: Some(_), .. } if *hit == target),
            "{events:?}"
        );
    }
    assert_eq!(r.get(id(2)).unwrap().stats.curr_hp, 0);
    assert!(r.get(id(2)).unwrap().stats.is_out());
    assert_eq!(r.get(id(1)).unwrap().stats.curr_hp, 400 - 55);
    assert_eq!(r.get(id(3)).unwrap().stats.curr_hp, 400 - 59);
    assert!(
        r.get(id(4)).is_none(),
        "slots 4 and 5 are empty, so there is no fighter to damage"
    );
}

/// Defend is the defender's own physical property (`Stats::begin_defending`),
/// and each target is computed from its own stats, so one member defending
/// changes that member's number alone. Slot 2's factor 3 becomes the defending
/// factor 1: `(124 * 1) >> 2 - 12`.
#[test]
fn defending_one_member_changes_that_members_number_only() {
    let data = all_party_data(&[FANBITE], &[SPIRAL_BLD]);
    let mut r = all_party_roster(&data, &FANBITE);
    r.get_mut(id(2)).unwrap().stats.begin_defending();
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(resolve_damage_skill(
        &mut r,
        id(6),
        SPIRAL_BLD,
        None,
        &data,
        &mut rolls,
        &mut events
    ));
    let damages: Vec<u16> = events
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Resolved {
                damage: Some(d), ..
            } => Some(*d),
            _ => None,
        })
        .collect();
    assert_eq!(damages, [55, 19, 59], "{events:?}");
    assert_eq!(
        damages[1],
        record_damage(111, 12, 1, 0),
        "the defended term"
    );
    assert_eq!(r.get(id(1)).unwrap().stats.curr_hp, 400 - 55);
    assert_eq!(r.get(id(3)).unwrap().stats.curr_hp, 400 - 59);
}

/// Hit points come off in the show-damage phase, three window phases after the
/// roll, and all five targets reach it in the same frame: the death of slot 1
/// cannot truncate slots 2 and 3, which have already been computed.
#[test]
fn a_death_mid_loop_leaves_later_targets_computed() {
    let data = all_party_data(&[FANBITE], &[SPIRAL_BLD]);
    let mut r = all_party_roster(&data, &FANBITE);
    r.get_mut(id(1)).unwrap().stats.curr_hp = 55;
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(resolve_damage_skill(
        &mut r,
        id(6),
        SPIRAL_BLD,
        None,
        &data,
        &mut rolls,
        &mut events
    ));
    assert_eq!(rolls.drawn(), 48);
    assert_eq!(events.len(), 5, "{events:?}");
    assert!(
        matches!(
            events[1],
            BattleEvent::Resolved {
                target,
                damage: Some(55),
                remaining_hp: 0,
                ..
            } if target == id(1)
        ),
        "{events:?}"
    );
    assert!(
        matches!(events[2], BattleEvent::Died { fighter } if fighter == id(1)),
        "{events:?}"
    );
    for (event, target) in events[3..].iter().zip([id(2), id(3)]) {
        assert!(
            matches!(event, BattleEvent::Resolved { target: hit, .. } if *hit == target),
            "{events:?}"
        );
    }
    assert_eq!(r.get(id(1)).unwrap().stats.curr_hp, 0);
    assert!(r.get(id(1)).unwrap().stats.is_out());
    assert_eq!(r.get(id(2)).unwrap().stats.curr_hp, 400 - 81);
    assert_eq!(r.get(id(3)).unwrap().stats.curr_hp, 400 - 59);
}

/// Every listed all-party pair, against the same roster: each number is its
/// carrier's own attack word through the record's selector 5, the target's own
/// defense word (`resistance` 6) and the target's own physical factor.
#[test]
fn every_all_party_pair_deals_its_own_record_number() {
    for (carrier, ability) in [
        (FANBITE, SPIRAL_BLD),
        (SAND_WORM, EARTHQUAKE),
        (KING_RAPPY, EARTHQUAKE),
    ] {
        let damage = resolve_all_party(&carrier, ability, Some(id(2)));
        assert_eq!(
            damage,
            expected_triple(&carrier),
            "{} {ability:#04X}",
            carrier.symbol
        );
        if carrier.enemy_id != FANBITE.enemy_id {
            assert_ne!(
                damage,
                expected_triple(&FANBITE),
                "{} {ability:#04X} is not Fanbite's number",
                carrier.symbol
            );
        }
    }
    assert_eq!(expected_triple(&FANBITE), [55, 81, 59]);
    assert_eq!(expected_triple(&SAND_WORM), [149, 222, 153]);
    assert_eq!(expected_triple(&KING_RAPPY), [130, 193, 134]);
    // The attack word is the whole word: SandWorm's 279 is a selector-5 read,
    // and the record's byte 3 is zero, so nothing else enters the product.
    assert_eq!(
        expected_triple(&SAND_WORM)[0],
        record_damage(279, 7, 2, 0),
        "279 is SandWorm's attack, not 279 & 0xFF"
    );
    assert_ne!(
        expected_triple(&SAND_WORM)[0],
        record_damage(279 & 0xFF, 7, 2, 0)
    );
}

/// The refusal that carries over from the single-target class: the resolver
/// models the damage request alone, so a listed all-party route whose record
/// runs an effect handler as well stays on the caller's unsupported path,
/// drawing nothing and emitting nothing.
#[test]
fn a_listed_all_party_route_with_an_effect_handler_is_refused() {
    let mut skill = all_party_record(SPIRAL_BLD);
    skill.effect = 0x02;
    let data = fixtures::data()
        .with_enemies([carrier_record(&FANBITE)])
        .with_enemy_skills([skill]);
    let mut r = all_party_roster(&data, &FANBITE);
    let before = r.clone();
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(!resolve_damage_skill(
        &mut r,
        id(6),
        SPIRAL_BLD,
        Some(id(2)),
        &data,
        &mut rolls,
        &mut events
    ));
    assert_eq!(r, before);
    assert_eq!(rolls.drawn(), 0);
    assert!(events.is_empty());
}

/// An unproven `(enemy, ability)` pair is still refused, class or no class:
/// 80 SandWorm never rolls `$08`, and 15 Fanbite never rolls `$38`.
#[test]
fn an_unlisted_pair_for_the_same_two_records_is_refused() {
    for (carrier, ability) in [(SAND_WORM, SPIRAL_BLD), (FANBITE, EARTHQUAKE)] {
        let data = all_party_data(&[carrier], &[ability]);
        let mut r = all_party_roster(&data, &carrier);
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
            "{} {ability:#04X}",
            carrier.symbol
        );
        assert_eq!(r, before, "{} {}", carrier.symbol, ability);
        assert_eq!(rolls.drawn(), 0, "{} {}", carrier.symbol, ability);
        assert!(events.is_empty(), "{} {}", carrier.symbol, ability);
    }
}

/// One round against one carrier, driven through `roll_enemy_ability` on a
/// fully specified draw stream: nine ordering draws, the four enemy-target
/// draws, the ability index (1, and every slot holds the ability) and three
/// times the 16 damage draws — 62 in all, plus whatever the carrier's own
/// object chain takes before the runs ([`object_draws`]) — with no swing.
/// Party agility 1 keeps the enemy first in the queue, so Defend has raised
/// nobody's resistance yet.
fn all_party_round(carrier: &Carrier, ability: u8) -> (Vec<BattleEvent>, usize) {
    // Every regular slot holds the ability under test, so the index roll lands
    // on it. The real list above is what the gate is proven against; a zero
    // slot here would dispatch the plain attack instead.
    let carrier = &Carrier {
        abilities: [ability; 8],
        ..*carrier
    };
    let data = all_party_data(&[*carrier], &[ability]);
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
        &mut SliceRolls::new(&[0]),
    )
    .unwrap();
    let mut stream = vec![0; 13];
    // The ability index: 1, not 0 - a battle starts the re-roll word at zero
    // (`ps4.asm:9992-9994`), so a zero draw would spend a re-roll, and every
    // slot holds the ability under test.
    stream.push(1);
    stream.extend([0; 48]);
    // The carrier's own object calls sit between the ability roll and the
    // first damage run, so the stream needs them where the resolver takes
    // them rather than only where the runs do.
    stream.extend(vec![0; object_draws(carrier)]);
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
/// just through a direct call: the ability resolves against the whole party,
/// the caster never swings, and no other ability of the same carrier's list gets
/// in the way.
#[test]
fn every_all_party_route_resolves_in_an_ordinary_round() {
    for (carrier, ability) in [
        (FANBITE, SPIRAL_BLD),
        (SAND_WORM, EARTHQUAKE),
        (KING_RAPPY, EARTHQUAKE),
    ] {
        let (events, drawn) = all_party_round(&carrier, ability);
        assert_eq!(
            drawn,
            62 + object_draws(&carrier),
            "{} {ability:#04X}: three requests, no swing",
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
        let hits: Vec<FighterId> = events
            .iter()
            .filter_map(|e| match e {
                BattleEvent::Resolved { target, .. } => Some(*target),
                _ => None,
            })
            .collect();
        assert_eq!(
            hits,
            [id(1), id(2), id(3)],
            "{} {ability:#04X}: the whole party, in slot order",
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

/// One carrier's `$38` (or `$08`) on a caller-supplied stream: the damage per
/// living party slot, in slot order, and the draws the run took.
fn resolve_on(carrier: &Carrier, ability: u8, stream: &[u16]) -> (Vec<u16>, usize) {
    let data = all_party_data(&[*carrier], &[ability]);
    let mut r = all_party_roster(&data, carrier);
    let mut rolls = SliceRolls::new(stream);
    let mut events = Vec::new();
    assert!(resolve_damage_skill(
        &mut r,
        id(6),
        ability,
        None,
        &data,
        &mut rolls,
        &mut events
    ));
    let damage = events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::Resolved {
                damage: Some(d), ..
            } => Some(*d),
            _ => None,
        })
        .collect();
    (damage, rolls.drawn())
}

/// The arithmetic behind [`EARTHQUAKE_SHAKE_DRAWS`], and the two `$38`
/// carriers side by side: same ability, same class, same three numbers, and 28
/// calls between the ability roll and the first damage run for one of them and
/// none for the other.
///
/// The count is the cartridge's, not the port's invention. `loc_24308`
/// (`ps4.asm:47943-47945`) enters the shake with `$11(a4) = $3C` frames and
/// `$10(a4) = 0`; `loc_2436A` (`ps4.asm:47963`) counts `$11` down and leaves
/// the state on the frame that empties it; `loc_2438E` (`ps4.asm:47972`) bumps
/// `$10`, masks it to two bits and reaches the two `jsr (UpdateRNGSeed2).l`
/// calls (lines 47976, 47983) only on the frame that masks to zero. Frames
/// 4, 8 … 56 draw, twice each — 14 × 2. The capture that settled it measured
/// exactly that: `build/sweep-3B/capture/forced_3B_attack_rolls.csv` holds two
/// calls a frame at f26541, f26545 … f26593 of SandWorm's round-3 `$38`.
#[test]
fn earthquakes_shake_draws_twice_every_fourth_frame() {
    assert_eq!(EARTHQUAKE_SHAKE_FRAMES, 0x3C, "sixty frames of shake");
    assert_eq!(EARTHQUAKE_SHAKE_PERIOD, 4, "the $10(a4) mask");
    assert_eq!(
        EARTHQUAKE_SHAKE_DRAWS, 28,
        "the frame that empties $11(a4) leaves the state instead of drawing"
    );

    // SandWorm: the shake, then one run per living slot.
    let (damage, drawn) = resolve_on(&SAND_WORM, EARTHQUAKE, &[0]);
    assert_eq!(drawn, 28 + 48, "the shake, then three runs of sixteen");
    assert_eq!(damage, expected_triple(&SAND_WORM));

    // KingRappy: the same shape with no object calls at all. 28 of the draws
    // are SandWorm's object, which is why the route carries them and the
    // ability cannot.
    let (king, drawn) = resolve_on(&KING_RAPPY, EARTHQUAKE, &[0]);
    assert_eq!(drawn, 48, "no object calls: three runs of sixteen");
    assert_eq!(king, expected_triple(&KING_RAPPY));

    // Fanbite's SPIRAL BLD is the third all-party route and draws none either.
    let (_, fanbite) = resolve_on(&FANBITE, SPIRAL_BLD, &[0]);
    assert_eq!(fanbite, 48);
}

/// The shake's calls are behind the request, so they come *first*: a stream
/// whose opening 28 words have nothing to add to `S` and whose next sixteen
/// mask to seven has to give the maximum-roll number for the first slot, not
/// the mean-roll one. Reading the 28 as the first slot's damage — the shape the
/// missing calls produced (issue #33) — gives the mean number instead.
#[test]
fn the_shake_draws_sit_before_the_first_damage_run() {
    let mut stream = vec![0u16; EARTHQUAKE_SHAKE_DRAWS as usize];
    stream.extend([7u16; 16]);
    let (damage, drawn) = resolve_on(&SAND_WORM, EARTHQUAKE, &stream);
    assert_eq!(drawn, 28 + 48);

    // Slot 1, defense 7, factor 2, at S = 112 rather than S = 0.
    let mut sixteen_max = SliceRolls::new(&[7u16; 16]);
    let max_roll = crate::battle::clamp_damage(crate::battle::calculate_damage(
        279,
        7,
        2,
        0,
        &mut sixteen_max,
    ));
    assert_eq!(damage[0], max_roll, "the sixteen words after the shake");
    assert_ne!(
        damage[0],
        record_damage(279, 7, 2, 0),
        "which is the number the shake's own words would have produced"
    );
}

/// The object's calls depend on nothing the resolution finds: the arm loads it
/// on the ability roll, so the shake runs its frames and takes its calls before
/// the five-slot write and the target walk behind it. A party with one slot
/// left takes the 16 of that slot; a party with none still takes the 28.
#[test]
fn the_shake_is_drawn_whatever_the_target_walk_finds() {
    for (dead, expected) in [(&[1u8, 3][..], 28 + 16), (&[1, 2, 3][..], 28)] {
        let data = all_party_data(&[SAND_WORM], &[EARTHQUAKE]);
        let mut r = all_party_roster(&data, &SAND_WORM);
        for slot in dead {
            kill(&mut r, *slot);
        }
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(resolve_damage_skill(
            &mut r,
            id(6),
            EARTHQUAKE,
            None,
            &data,
            &mut rolls,
            &mut events
        ));
        assert_eq!(rolls.drawn(), expected, "dead slots {dead:?}");
        let resolved = events
            .iter()
            .filter(|event| matches!(event, BattleEvent::Resolved { .. }))
            .count();
        assert_eq!(
            resolved,
            3 - dead.len(),
            "one per living slot, the skill event always: {events:?}"
        );
        assert!(matches!(
            events.first(),
            Some(BattleEvent::EnemySkillUsed { .. })
        ));
    }
}
