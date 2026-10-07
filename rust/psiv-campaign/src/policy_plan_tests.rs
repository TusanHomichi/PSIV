//! The party policy's rules on constructed boards.
//!
//! Each test carries its own negative: the same board with the one fact the
//! rule reads changed, where the choice must change with it.

use super::*;
use crate::policy_board::Weapon;
use crate::policy_estimate::{damage, max_damage};

const FIRE: u8 = 3;
const WATER: u8 = 5;

fn chaz() -> Combatant {
    Combatant {
        name: "Chaz".into(),
        tp: 50,
        mental: 60,
        strength: 60,
        dexterity: 50,
        agility: 40,
        attack: (100, 100),
        defence: (40, 40),
        mental_defence: (20, 20),
        ..Combatant::new(1, 400, 400)
    }
}

fn rika() -> Combatant {
    Combatant {
        name: "Rika".into(),
        defence: (20, 20),
        ..Combatant::new(2, 300, 300)
    }
}

fn foe(attack: u16) -> Combatant {
    Combatant {
        name: "Foe".into(),
        agility: 20,
        attack: (attack, attack),
        defence: (30, 30),
        mental_defence: (20, 20),
        ..Combatant::new(6, 2000, 2000)
    }
}

fn technique(id: u8, effect: u8, range: u8, power: u8, element: u8, cost: u8, targets: &[u8]) -> Ability {
    Ability {
        source: Source::Technique(id),
        effect,
        range,
        power_stat: 60,
        power,
        resistance: 7,
        element,
        tp_cost: cost,
        targets: targets.to_vec(),
    }
}

fn kit(id: u8, abilities: Vec<Ability>) -> Kit {
    Kit {
        id,
        abilities,
        weapon: Some(Weapon {
            elements: vec![1],
            all: false,
        }),
    }
}

/// A board whose actor, fighter 1, has `abilities`; nobody else acts.
fn board(party: Vec<Combatant>, enemies: Vec<Combatant>, abilities: Vec<Ability>) -> Board {
    Board {
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
    let mut fire_weak = foe(50);
    fire_weak.elements[usize::from(FIRE - 1)] = 4;
    let chosen = scripted().decide(&board(vec![chaz()], vec![fire_weak], abilities.clone()));
    assert_eq!(tech_id(&chosen), Some(1), "{chosen:?}");

    let mut water_weak = foe(50);
    water_weak.elements[usize::from(WATER - 1)] = 4;
    let chosen = scripted().decide(&board(vec![chaz()], vec![water_weak], abilities.clone()));
    assert_eq!(tech_id(&chosen), Some(4), "{chosen:?}");

    let mut fire_resistant = foe(50);
    fire_resistant.elements[usize::from(FIRE - 1)] = 1;
    fire_resistant.elements[usize::from(WATER - 1)] = 3;
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
    let mut hurt = rika();
    hurt.hp = 180;
    let strong = foe(200);
    assert!(
        max_damage(strong.attack.0, hurt.defence.0, 2, 0) >= hurt.hp,
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

    let chosen = scripted().decide(&board(vec![chaz(), hurt.clone()], vec![foe(50)], vec![res.clone()]));
    assert!(matches!(chosen, Intent::Attack { .. }), "{chosen:?}");

    // What a round showed counts too: Chaz lost 200 of his 800 last round, so
    // Rika's 180 is at risk although no plain attack could do it.
    let mut planner = scripted();
    let big = Combatant {
        hp: 800,
        max_hp: 800,
        ..chaz()
    };
    let full = board(vec![big.clone(), rika()], vec![foe(50)], vec![res.clone()]);
    assert!(matches!(planner.decide(&full), Intent::Attack { .. }));
    planner.end_round();
    let mut hit = big;
    hit.hp = 600;
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
    let mut weak = foe(50);
    weak.elements[usize::from(FIRE - 1)] = 4;
    let mut poor = chaz();
    poor.tp = 19;
    let chosen = scripted().decide(&board(vec![poor.clone()], vec![weak.clone()], vec![nafoi.clone()]));
    assert!(matches!(chosen, Intent::Attack { .. }), "{chosen:?}");

    poor.tp = 20;
    let chosen = scripted().decide(&board(vec![poor.clone()], vec![weak.clone()], vec![nafoi.clone()]));
    assert_eq!(tech_id(&chosen), Some(3));

    // A caster who knows a cure keeps its TP: 22 covers NAFOI but not
    // NAFOI and RES.
    poor.tp = 22;
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
        tp_cost: 0,
        targets: vec![6],
    };
    let enemy = foe(50);
    // A weak swing, so the comparison is between the two commands.
    let me = Combatant {
        attack: (20, 20),
        ..chaz()
    };
    let planner = scripted();
    let b = board(vec![me.clone()], vec![enemy.clone()], vec![crosscut.clone()]);
    let once = damage(60, enemy.defence.0, 2, 20, enemy.hp);
    let pick = planner
        .ability_pick(&b, &b.kits[0], 0)
        .expect("CROSSCUT has a target");
    assert_eq!(pick.value, once);

    // A technique between one and two CROSSCUTs.
    let power = (0..=255_u8)
        .find(|p| {
            let v = damage(60, enemy.mental_defence.0, 2, (*p).into(), enemy.hp);
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
    let mut a = foe(50);
    a.hp = 50;
    let mut b = foe(50);
    b.id = 7;
    b.hp = 50;
    let second = Combatant {
        name: "Gryz".into(),
        ..chaz()
    };
    let second = Combatant { id: 3, ..second };
    let mut round = Board {
        actor: 1,
        party: vec![chaz(), second],
        enemies: vec![a, b],
        kits: vec![kit(1, Vec::new()), kit(3, Vec::new())],
        stock: Vec::new(),
    };
    let mut planner = scripted();
    assert_eq!(planner.decide(&round), Intent::Attack { target: Some(6) }, "a tie takes the first");
    round.actor = 3;
    assert_eq!(planner.decide(&round), Intent::Attack { target: Some(7) }, "6 is already booked");
    planner.end_round();
    round.actor = 1;
    assert_eq!(planner.decide(&round), Intent::Attack { target: Some(6) }, "a new round");
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
    let mut hurt = rika();
    hurt.hp = 180;
    let weak = Combatant {
        id: 3,
        name: "Hahn".into(),
        attack: (10, 10),
        ..chaz()
    };
    let enemies = vec![foe(200)];
    let mut round = Board {
        actor: 1,
        party: vec![chaz(), hurt.clone(), weak.clone()],
        enemies: enemies.clone(),
        kits: vec![kit(1, vec![res(60)]), kit(3, vec![res(60)])],
        stock: Vec::new(),
    };
    let mut planner = scripted();
    assert!(matches!(planner.decide(&round), Intent::Attack { .. }), "Chaz hits");
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
