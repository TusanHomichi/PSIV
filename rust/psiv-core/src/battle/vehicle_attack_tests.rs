//! The vehicle's swing, on the capture's own rolls and on its own profiles.
//!
//! What the tests here pin:
//!
//! * the draw count: three `loc_B6A2` passes and one sixteen-draw
//!   `Battle_CalculateDamage` per swing, in that order, then nothing else;
//! * which pass decides — the first two are drawn for their rolls alone
//!   (`loc_B6A2` blanks all nine flags before each pass), so a critical on the
//!   third pass is the one that carries a bonus and a critical on an earlier
//!   one is discarded;
//! * the damage: the `$53` capture's own frames, replayed to the log's 165 and
//!   234, and each vehicle's own attack byte through the same formula;
//! * the element: `loc_280A`'s `$32(a1)` — the **target's** energy property —
//!   with the actor's hands never read;
//! * the negative control: a fighter that is not a vehicle keeps
//!   `Character_Attack`'s weapon check and still spends an unarmed turn with
//!   `TurnSkipped { reason: Unarmed }`.
//!
//! The enemy record here is the pack's `DESRTLEACH`
//! (`generated/enemies.json`, enemy 81) as `replay/pack.rs` carries it, with
//! its three SAND STORM slots cleared so an enemy turn is a plain attack.

use super::*;
use crate::battle::damage::DAMAGE_DRAWS;
use crate::battle::rng::SliceRolls;
use crate::battle::{
    Battle, BattleData, EQUIPMENT_SLOTS, EnemyRecord, FormationEnemy, FormationRecord, RoundOrders,
    Stats, fixtures, resolve_attack, status,
};
use crate::state::VehicleRecord;
use crate::vehicle;

fn id(n: u8) -> FighterId {
    FighterId::new(n).expect("a valid id")
}

/// The `$53` capture's sixteen damage draws for round 1, f25098. They sum to
/// 52 under `andi.w #7`, which is the `S` the log's 165 pins.
const ROUND_1_DRAWS: [u16; DAMAGE_DRAWS] = [
    54585, 9461, 19287, 57121, 10526, 20327, 24840, 60018, 44864, 4799, 17217, 56326, 43137, 36566,
    840, 15362,
];

/// Round 2's, f25248 — `S` = 64, and the swing's third pass came back critical.
const ROUND_2_DRAWS: [u16; DAMAGE_DRAWS] = [
    20416, 25223, 27244, 28407, 29013, 62411, 13188, 54267, 42062, 3476, 49400, 6957, 18528, 24338,
    60335, 12415,
];

/// Enemy 81, the Desrt Leach the `$53` capture fought.
fn desrt_leach() -> EnemyRecord {
    EnemyRecord {
        id: 81,
        name: "DESRTLEACH".into(),
        hp: 1040,
        strength: 68,
        mental: 1,
        agility: 56,
        dexterity: 52,
        attack: 286,
        defence: 28,
        mental_defence: 19,
        attack_element: 1,
        attack_status: 0,
        properties: [2, 2, 2, 2, 4, 2, 2, 0, 1, 2, 0, 0, 0, 2],
        regular_abilities: [0; 8],
        condition_ids: [0; 4],
        conditional_abilities: [0; 4],
        experience: 1500,
        meseta: 1,
    }
}

fn data() -> BattleData {
    fixtures::data().with_enemies([desrt_leach()])
}

/// `Vehicle_Index` `index` at `hp`, seated in the party's first slot, against
/// `enemy` in the first enemy slot.
fn roster_against(index: u16, hp: u16, enemy: &EnemyRecord) -> (Roster, FighterId) {
    let member = vehicle::battle_member(
        index,
        VehicleRecord {
            current_hp: hp,
            max_hp: hp,
            ..VehicleRecord::default()
        },
    )
    .expect("a retail vehicle");
    let mut roster = Roster::new();
    let actor = roster
        .add_party_member(member.character, member.name.clone(), member.stats)
        .expect("a free party slot");
    assert_eq!(actor, id(1), "loc_78EE seats the vehicle in Fighter_Char_1");
    assert!(roster.add_enemy(1, enemy).is_some());
    (roster, actor)
}

/// The same, against the capture's own Desrt Leach.
fn roster(index: u16, hp: u16) -> (Roster, FighterId) {
    roster_against(index, hp, &desrt_leach())
}

/// The `$53` capture's formation: ambush 30, run 25, no drop, one enemy.
fn formation(enemy_id: u16) -> FormationRecord {
    FormationRecord {
        id: 0x53,
        ambush_chance: 30,
        run_chance: 25,
        drop_rate: 0,
        drop_item: None,
        enemies: vec![FormationEnemy {
            slot: 1,
            enemy_id,
            position: 0,
        }],
    }
}

/// The port's resolution for `target`.
fn resolution(events: &[BattleEvent], target: FighterId) -> (Verdict, Option<u16>) {
    events
        .iter()
        .find_map(|event| match event {
            BattleEvent::Resolved {
                target: hit,
                verdict,
                damage,
                ..
            } if *hit == target => Some((*verdict, *damage)),
            _ => None,
        })
        .expect("the swing resolved its target")
}

fn hp(roster: &Roster, target: FighterId) -> u16 {
    roster.get(target).expect("present").stats.curr_hp
}

/// One whole swing of `index`'s vehicle: `hits` (one per pass), then `draws`.
fn swing(index: u16, hits: &[u16], draws: &[u16]) -> (Roster, Vec<BattleEvent>) {
    let (mut roster, actor) = roster(index, 740);
    let stream: Vec<u16> = hits.iter().chain(draws.iter()).copied().collect();
    let mut rolls = SliceRolls::new(&stream);
    let mut events = Vec::new();
    let died = resolve_attack(&mut roster, actor, None, &data(), &mut rolls, &mut events)
        .expect("a vehicle swing resolves without the item table");
    assert!(died.is_empty());
    assert_eq!(
        rolls.drawn(),
        stream.len(),
        "one roll per pass plus the damage run, and nothing else"
    );
    (roster, events)
}

#[test]
fn the_land_rover_replays_the_captures_first_swing() {
    // f25059, f25060, f25072: one roll per pass. The margin is dexterity 70
    // against agility 56, and the three draws read r = 24, 42 and 0 — all
    // normal, which is the log's $00 flag for the swing and the 165 its damage
    // needs: ((52+8)*200)>>6 + 200 = 387, *2 >> 2 = 193, -28 = 165.
    let (roster, events) = swing(1, &[19672, 50730, 32000], &ROUND_1_DRAWS);
    assert_eq!(ROUND_1_DRAWS.iter().map(|r| r & 7).sum::<u16>(), 52);
    assert_eq!(resolution(&events, id(6)), (Verdict::Normal, Some(165)));
    assert_eq!(hp(&roster, id(6)), 875, "1040 - 165, the log's own HP");
}

#[test]
fn the_land_rover_replays_the_captures_second_swing() {
    // f25209, f25210, f25222, the damage at f25248. Here the passes disagree:
    // r = 63 is a critical, r = 2 is normal and r = 60 is a critical again, and
    // the log's flag for the swing is $01 with 234 of damage — so the **third**
    // pass is the one that survived. ((64+8)*200)>>6 + 200 + 2*50 = 525,
    // *2 >> 2 = 262, -28 = 234.
    let (roster, events) = swing(1, &[18879, 62594, 1212], &ROUND_2_DRAWS);
    assert_eq!(ROUND_2_DRAWS.iter().map(|r| r & 7).sum::<u16>(), 64);
    assert_eq!(resolution(&events, id(6)), (Verdict::Critical, Some(234)));
    assert_eq!(hp(&roster, id(6)), 806, "1040 - 234, the log's own HP");
}

#[test]
fn only_the_third_passs_verdicts_survive() {
    // Round 2's own first two rolls, with the third pass swapped for a normal
    // hit (r = 0): the swing lands 184 instead of 234, because `loc_B6A2`
    // blanks all nine `Fighters_Hit_Flags` before every pass
    // (ps4.asm:17493-17496) and only the last write reaches loc_266C.
    let (_, earned) = swing(1, &[18879, 62594, 1212], &ROUND_2_DRAWS);
    let (_, demoted) = swing(1, &[18879, 62594, 0], &ROUND_2_DRAWS);
    assert_eq!(resolution(&earned, id(6)), (Verdict::Critical, Some(234)));
    assert_eq!(
        resolution(&demoted, id(6)),
        (Verdict::Normal, Some(184)),
        "the third pass's own verdict, not the first's"
    );
}

/// Round 3's sixteen damage draws, f25520 — `S` = 45.
const ROUND_3_DRAWS: [u16; DAMAGE_DRAWS] = [
    13264, 21779, 58424, 44131, 37009, 1009, 15389, 55502, 42815, 36752, 33404, 31861, 31113,
    30764, 30916, 63377,
];

#[test]
fn the_logs_third_round_needs_the_last_passs_verdict() {
    // Round 3's own rolls, the discriminating case: the first pass reads
    // critical (r = 61, (61 + 14) * 2 = 150) and the second and third read
    // normal (r = 1, r = 2), while the log's damage is 154 — the normal hit's
    // arithmetic, ((45+8)*200)>>6 + 200 = 365, 365*2>>2 - 28 = 154. A port
    // reading the *first* pass's flags would land 204, and so would one that
    // demoted a critical: 365 + 2*50 = 465, 465*2>>2 - 28 = 204.
    //
    // The fixture's own `hit` byte for this action is $01 all the same: it is
    // the flag at the action's **hit frame** (`oracle/fixture/observations.py`),
    // which for a swing whose passes arrive in three frames is the first
    // pass's, not the last. `replay::compare::flag_lags_the_swing` is where
    // the replay reads that byte as a reach claim instead, and the damage is
    // what pins the verdict it reports.
    let (roster, events) = swing(1, &[64381, 49153, 43906], &ROUND_3_DRAWS);
    assert_eq!(ROUND_3_DRAWS.iter().map(|r| r & 7).sum::<u16>(), 45);
    assert_eq!(resolution(&events, id(6)), (Verdict::Normal, Some(154)));
    assert_eq!(hp(&roster, id(6)), 1040 - 154);
}

#[test]
fn the_swing_costs_three_passes_then_one_damage_run() {
    let (mut roster, actor) = roster(1, 740);
    let mut rolls = SliceRolls::new(&[0; VEHICLE_HIT_PASSES + DAMAGE_DRAWS]);
    let mut events = Vec::new();
    resolve_attack(&mut roster, actor, None, &data(), &mut rolls, &mut events).expect("resolves");
    let targets = events
        .iter()
        .find_map(|event| match event {
            BattleEvent::Attacked {
                actor: who,
                targets,
            } if *who == actor => Some(targets.clone()),
            _ => None,
        })
        .expect("the vehicle swings");
    assert_eq!(targets, vec![id(6)]);
    // Sixteen zero draws: ((0+8)*200)>>6 + 200 = 225, *2 >> 2 = 112, -28 = 84.
    assert_eq!(resolution(&events, id(6)), (Verdict::Normal, Some(84)));
    assert_eq!(
        rolls.drawn(),
        VEHICLE_HIT_PASSES + DAMAGE_DRAWS,
        "three loc_B6A2 passes and one Battle_CalculateDamage, in that order"
    );
}

#[test]
fn every_vehicle_swings_with_its_own_attack_byte() {
    // Sixteen zero draws give S = 0, so ((8*ATK)>>6 + ATK) * 2 >> 2 - 28:
    // the Land Rover's 200 lands 84, the Ice Digger's 250 lands 112 and the
    // Hydrofoil's 150 lands 56. All three draw the same three passes.
    for (index, attack, damage) in [(1u16, 200u16, 84u16), (2, 250, 112), (3, 150, 56)] {
        let profile = vehicle::profile(index).expect("a retail vehicle");
        assert_eq!(
            profile.attack, attack,
            "{}'s VehicleData attack",
            profile.name
        );
        let (roster, events) = swing(index, &[0, 0, 0], &[0; DAMAGE_DRAWS]);
        let actor = id(1);
        assert_eq!(
            roster.get(actor).expect("present").stats.attack.battle,
            attack
        );
        assert_eq!(
            resolution(&events, id(6)),
            (Verdict::Normal, Some(damage)),
            "{}'s swing",
            profile.name
        );
        assert_eq!(
            hp(&roster, id(6)),
            1040 - damage,
            "{}'s damage",
            profile.name
        );
    }
}

#[test]
fn the_cannon_is_energy_elemental_and_reads_the_target() {
    // loc_280A reads `$32(a1)` — the target's own element-2 property — and
    // nothing else. On the capture's draws the Desrt Leach's 2 gives 165; an
    // immune target lands the 1-damage floor and a very weak one 359. The
    // attacker's hands take no part: the vehicle build writes no equipment.
    for (energy, expected) in [(2u8, 165u16), (0, 1), (4, 359)] {
        let mut target = desrt_leach();
        target.properties[1] = energy;
        let (mut roster, actor) = roster_against(1, 740, &target);
        let stream: Vec<u16> = [19672u16, 50730, 32000]
            .iter()
            .chain(&ROUND_1_DRAWS)
            .copied()
            .collect();
        let mut rolls = SliceRolls::new(&stream);
        let mut events = Vec::new();
        resolve_attack(&mut roster, actor, None, &data(), &mut rolls, &mut events)
            .expect("resolves");
        assert_eq!(
            resolution(&events, id(6)),
            (Verdict::Normal, Some(expected)),
            "the target's own element 2 decides"
        );
        assert!(
            events
                .iter()
                .all(|event| !matches!(event, BattleEvent::Died { .. })),
            "the floor is not a death"
        );
    }
}

#[test]
fn a_swing_reaches_the_cursor_or_the_first_living_enemy() {
    let (mut roster, actor) = roster(1, 740);
    assert!(roster.add_enemy(2, &desrt_leach()).is_some());
    let second = id(7);

    // The command's target word is one slot (loc_1152), so a swing at the
    // cursor's enemy leaves the other one alone.
    let mut rolls = SliceRolls::new(&[0; VEHICLE_HIT_PASSES + DAMAGE_DRAWS]);
    let mut events = Vec::new();
    resolve_attack(
        &mut roster,
        actor,
        Some(second),
        &data(),
        &mut rolls,
        &mut events,
    )
    .expect("resolves");
    assert_eq!(resolution(&events, second), (Verdict::Normal, Some(84)));
    assert_eq!(hp(&roster, id(6)), 1040, "the cursor chose the other slot");
    assert_eq!(
        rolls.drawn(),
        VEHICLE_HIT_PASSES + DAMAGE_DRAWS,
        "one target, one roll per pass"
    );

    // A cursor left on a corpse falls to the first survivor, exactly as the
    // party's own single-target swing does.
    roster.get_mut(id(6)).expect("present").stats.status = status::DEAD;
    let mut rolls = SliceRolls::new(&[0; VEHICLE_HIT_PASSES + DAMAGE_DRAWS]);
    let mut events = Vec::new();
    resolve_attack(
        &mut roster,
        actor,
        Some(id(6)),
        &data(),
        &mut rolls,
        &mut events,
    )
    .expect("resolves");
    assert_eq!(resolution(&events, second), (Verdict::Normal, Some(84)));
}

#[test]
fn a_kill_is_reported_for_the_engine_to_award() {
    let (mut roster, actor) = roster(1, 740);
    roster.get_mut(id(6)).expect("present").stats.curr_hp = 50;
    let mut rolls = SliceRolls::new(&[0; VEHICLE_HIT_PASSES + DAMAGE_DRAWS]);
    let mut events = Vec::new();
    let died = resolve_attack(&mut roster, actor, None, &data(), &mut rolls, &mut events)
        .expect("resolves");
    assert_eq!(died, vec![id(6)]);
    assert!(events.iter().any(|event| matches!(
        event,
        BattleEvent::Died { fighter } if *fighter == id(6)
    )));
    assert_eq!(hp(&roster, id(6)), 0);
}

#[test]
fn a_vehicle_round_swings_instead_of_skipping() {
    // The engine's own path: the runtime's `Command::Attack` for a mounted
    // battle, on a stream of zero rolls.
    let data = data();
    let member = vehicle::battle_member(
        1,
        VehicleRecord {
            current_hp: 740,
            max_hp: 740,
            ..VehicleRecord::default()
        },
    )
    .expect("a retail vehicle");
    let mut rolls = SliceRolls::new(&[0; 256]);
    let (mut battle, _) = Battle::start_vehicle(&formation(81), vec![member], &data, &mut rolls)
        .expect("the formation resolves");
    assert!(battle.is_vehicle());
    let timeline = battle
        .round(&RoundOrders::attack_all(), &data, &mut rolls)
        .expect("the round resolves");
    assert!(
        timeline.iter().all(|event| !matches!(
            event,
            BattleEvent::TurnSkipped {
                reason: Skipped::Unarmed,
                ..
            }
        )),
        "the vehicle's turn is command 6, not Character_Attack"
    );
    assert!(
        timeline.iter().any(|event| matches!(
            event,
            BattleEvent::Attacked { actor, .. } if *actor == id(1)
        )),
        "the vehicle swings"
    );
    assert!(
        timeline.iter().any(|event| matches!(
            event,
            BattleEvent::Resolved {
                target,
                damage: Some(84),
                ..
            } if *target == id(6)
        )),
        "and the swing lands the Land Rover's own damage"
    );
}

#[test]
fn an_unarmed_character_still_skips_its_turn() {
    // The negative control. Hahn is not a vehicle, so his Attack is command 1
    // and keeps `Character_Attack`'s weapon check (ps4.asm:13002-13019): both
    // hands empty is `move.w #$FFFF, $32(a4)` and no `loc_B6A2` at all.
    let data = data();
    let mut record = fixtures::hahn();
    record.equipment = [0; EQUIPMENT_SLOTS];
    let mut roster = Roster::new();
    let actor = roster
        .add_party_member(
            record.id,
            record.name.clone(),
            Stats::from_character(&record, |_| None),
        )
        .expect("a free party slot");
    assert!(roster.add_enemy(1, &desrt_leach()).is_some());
    assert!(
        !is_vehicle_fighter(roster.get(actor).expect("present")),
        "Hahn is a character, not a vehicle fighter"
    );

    let mut rolls = SliceRolls::new(&[0; 64]);
    let mut events = Vec::new();
    let died =
        resolve_attack(&mut roster, actor, None, &data, &mut rolls, &mut events).expect("resolves");
    assert!(died.is_empty());
    assert_eq!(
        events,
        vec![BattleEvent::TurnSkipped {
            actor,
            reason: Skipped::Unarmed
        }]
    );
    assert_eq!(rolls.drawn(), 0, "an unarmed turn costs no rolls");
}

#[test]
fn the_vehicle_identity_is_loc_78ee_s_fighter_ids() {
    // `Vehicle_Index + $B` for the three retail vehicles, and nothing else:
    // the eleven characters' own ids never select the vehicle route.
    let (roster, actor) = roster(1, 740);
    let fighter = roster.get(actor).expect("present");
    assert_eq!(fighter.character, Some(0x0C));
    assert!(is_vehicle_fighter(fighter));
    assert_eq!(VEHICLE_FIGHTER_IDS, [0x0C, 0x0D, 0x0E]);
    for index in 1..=3u16 {
        let member = vehicle::battle_member(index, VehicleRecord::default()).expect("a vehicle");
        assert_eq!(
            member.character,
            u8::try_from(0x0B + index).expect("a byte"),
            "Vehicle_Index {index}"
        );
        assert!(VEHICLE_FIGHTER_IDS.contains(&member.character));
    }

    let mut ride = fighter.clone();
    ride.character = Some(1);
    assert!(!is_vehicle_fighter(&ride), "Alys is not a vehicle");
    ride.character = None;
    assert!(!is_vehicle_fighter(&ride), "an enemy is not a vehicle");
}

#[test]
fn the_element_is_the_targets_second_property_word() {
    // loc_280A's `move.b $32(a1), d3`: element_props' second word, which is
    // element id 2 for `element_factor`.
    assert_eq!(VEHICLE_ATTACK_ELEMENT, 2);
    let stats = Stats::from_enemy(&desrt_leach());
    assert_eq!(stats.element_factor(VEHICLE_ATTACK_ELEMENT), Some(2));
    assert_eq!(stats.element_factor(1), Some(2), "its physical property");
    assert_eq!(stats.element_factor(5), Some(4), "water is what it fears");
}
