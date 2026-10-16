//! The party policy's rules on constructed boards.
//!
//! Each test carries its own negative: the same board with the one fact the
//! rule reads changed, where the choice must change with it.

use std::sync::LazyLock;

use psiv_core::battle::{
    BattleData, Bonuses, ELEMENT_SLOTS, EQUIPMENT_SLOTS, Fighter, FighterId, ItemKind, ItemRecord,
    SKILL_SLOTS, StatPair, StatTriple, Stats, TECHNIQUE_SLOTS,
};

use super::*;
use crate::policy_board::Weapon;
use crate::policy_estimate::{HP, damage, max_damage, min_damage};

const FIRE: u8 = 3;
const WATER: u8 = 5;
/// A one-handed sword whose swing is physical (element 1).
const SWORD: u8 = 1;

/// The records every board borrows: the sword.
static DATA: LazyLock<BattleData> = LazyLock::new(|| {
    BattleData::new().with_items([ItemRecord {
        id: SWORD,
        name: "SWORD".into(),
        kind: ItemKind::OneHandedSingleTarget,
        bonuses: Bonuses::default(),
        element: 1,
    }])
});

/// Fighter `id` at full `hp`: every stat zero, every element factor normal
/// (2), nothing equipped, the slot occupied.
fn combatant(id: u8, name: &str, hp: u16) -> Combatant {
    let stats = Stats {
        name_bytes: [0; 6],
        profession: 0,
        level: 0,
        experience: 0,
        curr_hp: hp,
        max_hp: hp,
        curr_tp: 0,
        max_tp: 0,
        status: 0,
        strength: StatTriple::uniform(0),
        mental: StatTriple::uniform(0),
        agility: StatTriple::uniform(0),
        dexterity: StatTriple::uniform(0),
        attack: StatPair::uniform(0),
        defence: StatPair::uniform(0),
        mental_defence: StatPair::uniform(0),
        element_props: [2; ELEMENT_SLOTS],
        element_shadow: [2; ELEMENT_SLOTS],
        weapon_elements: [0; 2],
        equipment: [0; EQUIPMENT_SLOTS],
        techniques: [0; TECHNIQUE_SLOTS],
        skills: [0; SKILL_SLOTS],
        curr_skill_uses: [0; SKILL_SLOTS],
        max_skill_uses: [0; SKILL_SLOTS],
        physical_prop_save: 2,
        enemy_id: 0,
        gain_exp_flag: false,
    };
    Combatant {
        fighter: Fighter {
            id: FighterId::new(id).expect("a fighter id"),
            name: name.into(),
            character: None,
            stats,
            ability: 0,
            reaction_flags: 0,
            active: true,
        },
        plain_attacks: 8,
    }
}

/// `c` with `edit` made to its live stats.
fn with(mut c: Combatant, edit: impl FnOnce(&mut Stats)) -> Combatant {
    edit(&mut c.fighter.stats);
    c
}

/// `c` as fighter `id`.
fn renumbered(mut c: Combatant, id: u8) -> Combatant {
    c.fighter.id = FighterId::new(id).expect("a fighter id");
    c
}

fn chaz() -> Combatant {
    with(combatant(1, "Chaz", 400), |s| {
        s.curr_tp = 50;
        s.mental = StatTriple::uniform(60);
        s.strength = StatTriple::uniform(60);
        s.dexterity = StatTriple::uniform(50);
        s.agility = StatTriple::uniform(40);
        s.attack = StatPair::uniform(100);
        s.defence = StatPair::uniform(40);
        s.mental_defence = StatPair::uniform(20);
        s.equipment[0] = SWORD;
    })
}

fn rika() -> Combatant {
    with(combatant(2, "Rika", 300), |s| {
        s.defence = StatPair::uniform(20)
    })
}

fn foe(attack: u16) -> Combatant {
    with(combatant(6, "Foe", 2000), |s| {
        s.agility = StatTriple::uniform(20);
        s.attack = StatPair::uniform(attack);
        s.defence = StatPair::uniform(30);
        s.mental_defence = StatPair::uniform(20);
        // An enemy's plain-attack element sits in the TP slot
        // (`Stats::from_enemy`): physical.
        s.curr_tp = 1;
    })
}

fn technique(
    id: u8,
    effect: u8,
    range: u8,
    power: u8,
    element: u8,
    cost: u8,
    targets: &[u8],
) -> Ability {
    Ability {
        source: Source::Technique(id),
        effect,
        range,
        power_stat: 60,
        power,
        resistance: 7,
        element,
        // The fixture's record: ranges 1, 4, 6 and 8 take a target cursor.
        single: matches!(range, 1 | 4 | 6 | 8),
        tp_cost: cost,
        targets: targets.to_vec(),
    }
}

fn kit(id: u8, abilities: Vec<Ability>) -> Kit {
    Kit {
        id,
        abilities,
        weapon: Some(Weapon { all: false }),
    }
}

/// A board whose actor, fighter 1, has `abilities`; nobody else acts.
fn board(
    party: Vec<Combatant>,
    enemies: Vec<Combatant>,
    abilities: Vec<Ability>,
) -> Board<'static> {
    Board {
        data: &DATA,
        actor: 1,
        party,
        enemies,
        kits: vec![kit(1, abilities)],
        stock: Vec::new(),
    }
}

fn scripted() -> Planner {
    let mut planner = Planner::default();
    planner.begin_battle(true);
    planner
}

fn tech_id(intent: &Intent) -> Option<u8> {
    match intent {
        Intent::Technique { id, .. } => Some(*id),
        _ => None,
    }
}

/// FOI against a fire-weak enemy, WAT against a water-weak one: the element
/// factor the engine multiplies by decides, not the record order. Negative:
/// the same two techniques against a fire-resistant enemy.
#[test]
fn the_policy_picks_the_element_the_enemy_is_weak_to() {
    let abilities = vec![
        technique(1, 1, 1, 40, FIRE, 4, &[6]),
        technique(4, 1, 1, 40, WATER, 4, &[6]),
    ];
    let fire_weak = with(foe(50), |s| s.element_props[usize::from(FIRE - 1)] = 4);
    let chosen = scripted().decide(&board(vec![chaz()], vec![fire_weak], abilities.clone()));
    assert_eq!(tech_id(&chosen), Some(1), "{chosen:?}");

    let water_weak = with(foe(50), |s| s.element_props[usize::from(WATER - 1)] = 4);
    let chosen = scripted().decide(&board(vec![chaz()], vec![water_weak], abilities.clone()));
    assert_eq!(tech_id(&chosen), Some(4), "{chosen:?}");

    let fire_resistant = with(foe(50), |s| {
        s.element_props[usize::from(FIRE - 1)] = 1;
        s.element_props[usize::from(WATER - 1)] = 3;
    });
    let chosen = scripted().decide(&board(vec![chaz()], vec![fire_resistant], abilities));
    assert_eq!(tech_id(&chosen), Some(4), "{chosen:?}");
}

/// Rika at 60% is above the old half-HP line, but the enemy's plain attack at
/// its top roll would take more than she has: she is cured now, before the
/// enemy's turn, not after she falls. Negative: a weak enemy, the same HP,
/// and the actor attacks.
#[test]
fn the_policy_heals_before_a_member_falls() {
    let res = technique(24, 18, 4, 20, 0, 3, &[1, 2]);
    let hurt = with(rika(), |s| s.curr_hp = 180);
    let strong = foe(200);
    assert!(
        max_damage(strong.attack(), hurt.defence(), 2, 0) >= hurt.hp(),
        "the board's premise: one top-roll swing would drop her"
    );
    let chosen = scripted().decide(&board(
        vec![chaz(), hurt.clone()],
        vec![strong],
        vec![res.clone()],
    ));
    assert_eq!(
        chosen,
        Intent::Technique {
            id: 24,
            target: Some(2)
        }
    );

    let chosen = scripted().decide(&board(
        vec![chaz(), hurt.clone()],
        vec![foe(50)],
        vec![res.clone()],
    ));
    assert!(matches!(chosen, Intent::Attack { .. }), "{chosen:?}");

    // What a round showed counts too: Chaz lost 200 of his 800 last round, so
    // Rika's 180 is at risk although no plain attack could do it.
    let mut planner = scripted();
    let big = with(chaz(), |s| {
        s.curr_hp = 800;
        s.max_hp = 800;
    });
    let full = board(vec![big.clone(), rika()], vec![foe(50)], vec![res.clone()]);
    assert!(matches!(planner.decide(&full), Intent::Attack { .. }));
    planner.end_round();
    let hit = with(big, |s| s.curr_hp = 600);
    let after = board(vec![hit, hurt], vec![foe(50)], vec![res]);
    assert_eq!(
        planner.decide(&after),
        Intent::Technique {
            id: 24,
            target: Some(2)
        }
    );
}

/// A cast the actor cannot pay for is never ordered, however good it looks.
/// Negative: the same board with the TP for it.
#[test]
fn the_policy_never_orders_a_technique_it_cannot_afford() {
    let nafoi = technique(3, 1, 1, 120, FIRE, 20, &[6]);
    let weak = with(foe(50), |s| s.element_props[usize::from(FIRE - 1)] = 4);
    let mut poor = with(chaz(), |s| s.curr_tp = 19);
    let chosen = scripted().decide(&board(
        vec![poor.clone()],
        vec![weak.clone()],
        vec![nafoi.clone()],
    ));
    assert!(matches!(chosen, Intent::Attack { .. }), "{chosen:?}");

    poor.fighter.stats.curr_tp = 20;
    let chosen = scripted().decide(&board(
        vec![poor.clone()],
        vec![weak.clone()],
        vec![nafoi.clone()],
    ));
    assert_eq!(tech_id(&chosen), Some(3));

    // A caster who knows a cure keeps its TP: 22 covers NAFOI but not
    // NAFOI and RES.
    poor.fighter.stats.curr_tp = 22;
    let res = technique(24, 18, 4, 20, 0, 3, &[1]);
    let chosen = scripted().decide(&board(vec![poor], vec![weak], vec![nafoi, res]));
    assert!(matches!(chosen, Intent::Attack { .. }), "{chosen:?}");
}

/// CROSSCUT resolves one damage pass per target (`loc_9880`, ps4.asm:14997;
/// docs/battle/PLAYER_ABILITIES.md): its value is one pass of the formula,
/// and a technique worth half again as much is preferred. Counted twice, as
/// the old policy did, CROSSCUT would win.
#[test]
fn the_policy_counts_crosscut_once() {
    let crosscut = Ability {
        source: Source::Skill(1),
        effect: 1,
        range: 1,
        power_stat: 60,
        power: 20,
        resistance: 6,
        element: 0x10,
        single: true,
        tp_cost: 0,
        targets: vec![6],
    };
    let enemy = foe(50);
    // A weak swing, so the comparison is between the two commands.
    let me = with(chaz(), |s| s.attack = StatPair::uniform(20));
    let planner = scripted();
    let b = board(
        vec![me.clone()],
        vec![enemy.clone()],
        vec![crosscut.clone()],
    );
    let once = damage(60, enemy.defence(), 2, 20, enemy.hp());
    let pick = planner
        .ability_pick(&b, &b.kits[0], 0)
        .expect("CROSSCUT has a target");
    assert_eq!(pick.value, once);

    // A technique between one and two CROSSCUTs.
    let power = (0..=255_u8)
        .find(|p| {
            let v = damage(60, enemy.stat(7), 2, (*p).into(), enemy.hp());
            v * 2 > once * 3 && v < once * 2
        })
        .expect("a power between one and two passes");
    let gifoi = technique(2, 1, 1, power, FIRE, 4, &[6]);
    let chosen = scripted().decide(&board(vec![me], vec![enemy], vec![crosscut, gifoi]));
    assert_eq!(tech_id(&chosen), Some(2), "{chosen:?}");
}

/// A random encounter keeps skill uses for the scripted battles and spends TP
/// only where it pays double; a scripted battle spends both.
#[test]
fn an_encounter_is_fought_cheaply_and_a_boss_with_everything() {
    let rayblade = Ability {
        source: Source::Skill(2),
        effect: 1,
        range: 1,
        power_stat: 60,
        power: 90,
        resistance: 6,
        element: 6,
        single: true,
        tp_cost: 0,
        targets: vec![6],
    };
    let b = board(vec![chaz()], vec![foe(50)], vec![rayblade]);
    let mut encounter = Planner::default();
    encounter.begin_battle(false);
    assert!(matches!(encounter.decide(&b), Intent::Attack { .. }));
    assert_eq!(
        scripted().decide(&b),
        Intent::Skill {
            id: 2,
            target: Some(6)
        }
    );
}

/// The round's book: a second member does not pile damage onto an enemy the
/// first one's order already finishes, and a new round starts a new book.
#[test]
fn the_round_book_spreads_damage_over_the_enemies() {
    let a = with(foe(50), |s| s.curr_hp = 50);
    let b = renumbered(with(foe(50), |s| s.curr_hp = 50), 7);
    let mut second = renumbered(chaz(), 3);
    second.fighter.name = "Gryz".into();
    let mut round = Board {
        data: &DATA,
        actor: 1,
        party: vec![chaz(), second],
        enemies: vec![a, b],
        kits: vec![kit(1, Vec::new()), kit(3, Vec::new())],
        stock: Vec::new(),
    };
    let mut planner = scripted();
    assert_eq!(
        planner.decide(&round),
        Intent::Attack { target: Some(6) },
        "a tie takes the first"
    );
    round.actor = 3;
    assert_eq!(
        planner.decide(&round),
        Intent::Attack { target: Some(7) },
        "6 is already booked"
    );
    planner.end_round();
    round.actor = 1;
    assert_eq!(
        planner.decide(&round),
        Intent::Attack { target: Some(6) },
        "a new round"
    );
}

/// The round is ordered as a whole: the cure goes to the member whose own
/// action is worth least, and the strong hitter keeps hitting. Negative: with
/// only the strong hitter able to cure, the strong hitter cures.
#[test]
fn the_cure_goes_to_the_member_whose_action_is_worth_least() {
    let res = |power_stat| Ability {
        power_stat,
        ..technique(24, 18, 4, 20, 0, 3, &[1, 2, 3])
    };
    // Below half, so the percentage line alone puts her at risk: this test is
    // about who cures, not when (`the_policy_heals_before_a_member_falls`).
    let hurt = with(rika(), |s| s.curr_hp = 120);
    let mut weak = renumbered(with(chaz(), |s| s.attack = StatPair::uniform(10)), 3);
    weak.fighter.name = "Hahn".into();
    let enemies = vec![foe(200)];
    let mut round = Board {
        data: &DATA,
        actor: 1,
        party: vec![chaz(), hurt.clone(), weak.clone()],
        enemies: enemies.clone(),
        kits: vec![kit(1, vec![res(60)]), kit(3, vec![res(60)])],
        stock: Vec::new(),
    };
    let mut planner = scripted();
    assert!(
        matches!(planner.decide(&round), Intent::Attack { .. }),
        "Chaz hits"
    );
    round.actor = 3;
    assert_eq!(
        planner.decide(&round),
        Intent::Technique {
            id: 24,
            target: Some(2)
        },
        "Hahn cures"
    );

    let mut alone = Board {
        data: &DATA,
        actor: 1,
        party: vec![chaz(), hurt, weak],
        enemies,
        kits: vec![kit(1, vec![res(60)]), kit(3, Vec::new())],
        stock: Vec::new(),
    };
    let mut planner = scripted();
    assert_eq!(
        planner.decide(&alone),
        Intent::Technique {
            id: 24,
            target: Some(2)
        }
    );
    alone.actor = 3;
    assert!(matches!(planner.decide(&alone), Intent::Attack { .. }));
}

/// Options worth the same are ranked by what they spend, never by the order
/// a kit lists them in: the same two techniques in either order give the same
/// choice, the lower id at equal cost and the cheaper at unequal cost.
/// Negative: in a kit holding a tie, reversing the list must not change the
/// choice.
#[test]
fn equal_options_are_ranked_by_spend_not_list_order() {
    let enemy = foe(50);
    let low_id = technique(4, 1, 1, 120, FIRE, 4, &[6]);
    let high_id = technique(10, 1, 1, 120, FIRE, 4, &[6]);
    for kit in [
        vec![low_id.clone(), high_id.clone()],
        vec![high_id.clone(), low_id.clone()],
    ] {
        let chosen = scripted().decide(&board(vec![chaz()], vec![enemy.clone()], kit));
        assert_eq!(tech_id(&chosen), Some(4), "{chosen:?}");
    }
    let cheap = technique(10, 1, 1, 120, FIRE, 2, &[6]);
    for kit in [
        vec![low_id.clone(), cheap.clone()],
        vec![cheap.clone(), low_id.clone()],
    ] {
        let chosen = scripted().decide(&board(vec![chaz()], vec![enemy.clone()], kit));
        assert_eq!(tech_id(&chosen), Some(10), "{chosen:?}");
    }
}

/// "Finish it" skips the round's cures only when the kill is certain: two
/// casts whose expected damage empties the enemy but whose least rolls do not
/// leave Rika, below half, to the enemy's turn. The round still cures her.
/// Negative: the same board with the enemy at the casts' least damage, where
/// the kill is certain and nobody cures.
#[test]
fn a_round_that_only_probably_kills_still_cures() {
    let foi = technique(1, 1, 1, 120, FIRE, 4, &[6]);
    let res = technique(24, 18, 4, 20, 0, 3, &[1, 2, 3]);
    let weak = |hp| with(foe(50), |s| s.curr_hp = hp);
    let least = min_damage(60, weak(1).stat(7), 2, 120);
    let hp = 2 * least + 1;
    let first = damage(60, weak(hp).stat(7), 2, 120, hp) / HP;
    let rest = hp - u16::try_from(first).expect("a pass is under 65,536 HP");
    let second = damage(60, weak(hp).stat(7), 2, 120, rest) / HP;
    assert!(
        first + second >= u64::from(hp),
        "the board's premise: the expected casts empty the enemy ({first} + {second} of {hp})"
    );
    let hurt = with(rika(), |s| s.curr_hp = 100);
    let mut gryz = renumbered(chaz(), 3);
    gryz.fighter.name = "Gryz".into();
    let round = |enemy_hp| Board {
        data: &DATA,
        actor: 1,
        party: vec![chaz(), hurt.clone(), gryz.clone()],
        enemies: vec![weak(enemy_hp)],
        kits: vec![
            kit(1, vec![foi.clone(), res.clone()]),
            kit(3, vec![foi.clone()]),
        ],
        stock: Vec::new(),
    };
    let chosen = scripted().decide(&round(hp));
    assert_eq!(
        chosen,
        Intent::Technique {
            id: 24,
            target: Some(2)
        },
        "{chosen:?}"
    );
    let chosen = scripted().decide(&round(2 * least));
    assert_eq!(tech_id(&chosen), Some(1), "a certain kill: {chosen:?}");
}
