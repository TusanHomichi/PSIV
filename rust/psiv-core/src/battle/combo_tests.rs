use super::*;
use crate::battle::engine::{Battle, Command, PartyMember, RoundOrders};
use crate::battle::fighters::FighterId;
use crate::battle::fixtures;
use crate::battle::records::{BattleData, FormationEnemy, FormationRecord};
use crate::battle::rng::SliceRolls;
use crate::battle::skill::Skill;
use crate::battle::stats::{Stats, status};
use crate::battle::technique::Technique;

fn id(v: u8) -> FighterId {
    FighterId::new(v).expect("valid fighter id")
}

fn tech(id: u8, name: &str, cost: u8, power: u8, targeting: u8, element: u8) -> Technique {
    Technique {
        id,
        name: name.into(),
        effect: 1,
        cost,
        targeting,
        power,
        resistance: 7,
        element,
    }
}

fn skill(
    id: u8,
    name: &str,
    power_stat: u8,
    requires_weapon: bool,
    power: u8,
    targeting: u8,
    element: u8,
) -> Skill {
    Skill {
        id,
        name: name.into(),
        effect: 1,
        power_stat,
        requires_weapon,
        targeting,
        power,
        resistance: 7,
        element,
    }
}

fn base_data() -> BattleData {
    let mut enemy = fixtures::zoran_bult();
    enemy.hp = 500;
    enemy.agility = 1;
    enemy.mental = 10;
    enemy.mental_defence = 10;
    enemy.defence = 10;
    enemy.properties = [2; 14];

    BattleData::new()
        .with_enemies([enemy])
        .with_items(fixtures::items())
        .with_techniques([
            tech(1, "FOI", 2, 10, 0x11, 3),
            tech(4, "WAT", 3, 12, 0x11, 5),
            tech(7, "TSU", 4, 15, 0x11, 7),
            tech(10, "ZAN", 4, 14, 0x12, 2),
            tech(13, "GRA", 8, 20, 0x12, 4),
            tech(16, "MEGID", 55, 60, 0x12, 14),
            Technique {
                id: 32,
                name: "DEBAN".into(),
                effect: 10,
                cost: 4,
                targeting: 0x19,
                power: 8,
                resistance: 0,
                element: 0,
            },
        ])
        .with_skills([
            skill(2, "RAYBLADE", 2, true, 16, 0x11, 6),
            skill(7, "ASTRAL", 2, false, 18, 0x11, 2),
            skill(11, "TANDLE", 2, false, 22, 0x12, 7),
            skill(13, "LEGEON", 2, false, 50, 0x12, 2),
            skill(15, "POSIBOLT", 1, false, 45, 0x12, 2),
            Skill {
                id: 27,
                name: "NEGATIS".into(),
                effect: 2,
                power_stat: 2,
                requires_weapon: false,
                targeting: 0x12,
                power: 0x40,
                resistance: 2,
                element: 14,
            },
        ])
        .with_combos([
            ComboRecord {
                id: 1,
                name: "SYNTH_ONE".into(),
                effect: 1,
                range: 1,
                power: 0,
                resistance: 7,
                element: 6,
                participants: 2,
                ordered: false,
                setups: vec![vec![0x80 | 2, 0x80 | 7]],
            },
            ComboRecord {
                id: 2,
                name: "SYNTH_TWO".into(),
                effect: 1,
                range: 2,
                power: 0,
                resistance: 7,
                element: 3,
                participants: 2,
                ordered: false,
                setups: vec![vec![1, 10]],
            },
            ComboRecord {
                id: 4,
                name: "SYNTH_ORD".into(),
                effect: 1,
                range: 2,
                power: 0,
                resistance: 7,
                element: 7,
                participants: 2,
                ordered: true,
                setups: vec![vec![4, 0x80 | 11]],
            },
            ComboRecord {
                id: 8,
                name: "SYNTH_TRI".into(),
                effect: 1,
                range: 2,
                power: 0,
                resistance: 7,
                element: 2,
                participants: 3,
                ordered: false,
                setups: vec![vec![1, 4, 7]],
            },
            ComboRecord {
                id: 10,
                name: "SYNTH_KO".into(),
                effect: 2,
                range: 2,
                power: 0x40,
                resistance: 2,
                element: 14,
                participants: 2,
                ordered: false,
                setups: vec![vec![13, 0x80 | 27]],
            },
            ComboRecord {
                id: 14,
                name: "SYNTH_QUAD".into(),
                effect: 1,
                range: 2,
                power: 0,
                resistance: 7,
                element: 6,
                participants: 4,
                ordered: true,
                setups: vec![vec![32, 16, 0x80 | 13, 0x80 | 15]],
            },
        ])
}

fn formation(count: usize) -> FormationRecord {
    FormationRecord {
        id: 1,
        ambush_chance: 0,
        run_chance: 0,
        drop_rate: 0,
        drop_item: None,
        enemies: (0..count)
            .map(|i| FormationEnemy {
                slot: (i + 1) as u8,
                enemy_id: 10,
                position: 0,
            })
            .collect(),
    }
}

fn make_member(
    character: u8,
    agility: u8,
    mental: u8,
    tp: u16,
    techs: &[u8],
    skills: &[u8],
    armed: bool,
) -> PartyMember {
    let name = format!("P{}", character + 1);
    let mut rec = fixtures::alys();
    rec.id = character;
    rec.name = name.clone();
    rec.agility = agility;
    rec.mental = mental;
    rec.strength = mental;
    rec.hp = 200;
    rec.tp = tp;
    rec.equipment = if armed { [3, 0, 0, 0] } else { [0, 0, 0, 0] };
    rec.techniques = [0; 16];
    for (i, &t) in techs.iter().enumerate() {
        rec.techniques[i] = t;
    }
    rec.skills = [0; 8];
    rec.skill_uses = [0; 8];
    for (i, &s) in skills.iter().enumerate() {
        rec.skills[i] = s;
        rec.skill_uses[i] = 3;
    }
    let items = fixtures::items();
    let stats = Stats::from_character(&rec, |item_id| {
        items.iter().find(|i| i.id == item_id).cloned()
    });
    PartyMember {
        character,
        name,
        stats,
    }
}

#[test]
fn unordered_two_tech_combo_fires_in_either_order_and_charges_both_casters() {
    let data = base_data();
    // Reverse order: ZAN (slot 0, agi 30) then FOI (slot 1, agi 20) still triggers FIRE STORM.
    let party = vec![
        make_member(0, 30, 20, 30, &[10], &[], true),
        make_member(1, 20, 25, 30, &[1], &[], true),
    ];
    let mut rolls = SliceRolls::new(&[0x40; 256]);
    let (mut battle, _) = Battle::start(&formation(2), party, &data, true, &mut rolls).unwrap();
    let orders = RoundOrders::Commands(vec![
        Command::Technique {
            technique: 10,
            target: None,
        },
        Command::Technique {
            technique: 1,
            target: Some(id(6)),
        },
    ]);
    let events = battle.round(&orders, &data, &mut rolls).unwrap();

    // Exactly one ComboUsed, no individual TechniqueUsed events.
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::TechniqueUsed { .. })),
        "individual tech casts must be consumed by the combo: {events:?}"
    );
    let combo = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::ComboUsed {
                actor,
                participants,
                tp_after,
                combo,
                name,
            } => Some((
                *actor,
                participants.clone(),
                tp_after.clone(),
                *combo,
                name.clone(),
            )),
            _ => None,
        })
        .expect("FIRE STORM fired");
    assert_eq!(combo.0, id(1));
    assert_eq!(combo.1, vec![id(1), id(2)]);
    assert_eq!(combo.2, vec![(id(1), 26), (id(2), 28)]);
    assert_eq!(combo.3, 2);
    assert_eq!(combo.4, "SYNTH_TWO");

    // Range 2 hits both enemies.
    let hits: Vec<FighterId> = events
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Resolved {
                actor,
                target,
                damage: Some(d),
                ..
            } if *actor == id(1) && *d > 0 => Some(*target),
            _ => None,
        })
        .collect();
    assert_eq!(hits, vec![id(6), id(7)]);
}

#[test]
fn ordered_combo_requires_exact_turn_order() {
    let data = base_data();
    // Correct order: WAT (agi 30) then TANDLE (agi 20) -> CONDCTTHND.
    let party_ok = vec![
        make_member(0, 30, 20, 30, &[4], &[], true),
        make_member(1, 20, 20, 30, &[], &[11], true),
    ];
    let mut rolls = SliceRolls::new(&[0; 256]);
    let (mut ok_battle, _) =
        Battle::start(&formation(1), party_ok, &data, true, &mut rolls).unwrap();
    let orders = RoundOrders::Commands(vec![
        Command::Technique {
            technique: 4,
            target: Some(id(6)),
        },
        Command::Skill {
            skill: 11,
            target: None,
        },
    ]);
    let ok_events = ok_battle.round(&orders, &data, &mut rolls).unwrap();
    assert!(
        ok_events
            .iter()
            .any(|e| matches!(e, BattleEvent::ComboUsed { combo: 4, .. })),
        "WAT -> TANDLE must trigger CONDCTTHND: {ok_events:?}"
    );
    // Skill participant spent 1 use.
    assert_eq!(
        ok_battle.roster().get(id(2)).unwrap().stats.curr_skill_uses[0],
        2
    );

    // Wrong order: TANDLE (agi 30) then WAT (agi 20) -> must NOT trigger CONDCTTHND.
    let party_rev = vec![
        make_member(0, 20, 20, 30, &[4], &[], true),
        make_member(1, 30, 20, 30, &[], &[11], true),
    ];
    let mut rolls_rev = SliceRolls::new(&[0; 256]);
    let (mut rev_battle, _) =
        Battle::start(&formation(1), party_rev, &data, true, &mut rolls_rev).unwrap();
    let rev_events = rev_battle.round(&orders, &data, &mut rolls_rev).unwrap();
    assert!(
        !rev_events
            .iter()
            .any(|e| matches!(e, BattleEvent::ComboUsed { .. })),
        "TANDLE -> WAT must NOT trigger ordered CONDCTTHND: {rev_events:?}"
    );
    assert!(
        rev_events
            .iter()
            .any(|e| matches!(e, BattleEvent::SkillUsed { skill: 11, .. }))
    );
    assert!(
        rev_events
            .iter()
            .any(|e| matches!(e, BattleEvent::TechniqueUsed { technique: 4, .. }))
    );
}

#[test]
fn three_and_four_member_combos_and_instant_death_combo_resolve() {
    let data = base_data();
    // 3-member unordered TRIBLASTER (TSU, FOI, WAT).
    let party3 = vec![
        make_member(0, 40, 20, 30, &[7], &[], true),
        make_member(1, 30, 20, 30, &[1], &[], true),
        make_member(2, 20, 20, 30, &[4], &[], true),
    ];
    let mut rolls = SliceRolls::new(&[0; 256]);
    let (mut b3, _) = Battle::start(&formation(2), party3, &data, true, &mut rolls).unwrap();
    let e3 = b3
        .round(
            &RoundOrders::Commands(vec![
                Command::Technique {
                    technique: 7,
                    target: Some(id(6)),
                },
                Command::Technique {
                    technique: 1,
                    target: Some(id(6)),
                },
                Command::Technique {
                    technique: 4,
                    target: Some(id(6)),
                },
            ]),
            &data,
            &mut rolls,
        )
        .unwrap();
    assert!(e3.iter().any(|e| matches!(
        e,
        BattleEvent::ComboUsed {
            combo: 8,
            participants,
            ..
        } if participants.len() == 3
    )));

    // 4-member ordered DESTRUCT (DEBAN, MEGID, LEGEON, POSIBOLT).
    let party4 = vec![
        make_member(0, 50, 25, 100, &[32], &[], true),
        make_member(1, 40, 25, 100, &[16], &[], true),
        make_member(2, 30, 25, 100, &[], &[13], true),
        make_member(3, 20, 25, 100, &[], &[15], true),
    ];
    let mut rolls4 = SliceRolls::new(&[0; 256]);
    let (mut b4, _) = Battle::start(&formation(2), party4, &data, true, &mut rolls4).unwrap();
    let e4 = b4
        .round(
            &RoundOrders::Commands(vec![
                Command::Technique {
                    technique: 32,
                    target: None,
                },
                Command::Technique {
                    technique: 16,
                    target: None,
                },
                Command::Skill {
                    skill: 13,
                    target: None,
                },
                Command::Skill {
                    skill: 15,
                    target: None,
                },
            ]),
            &data,
            &mut rolls4,
        )
        .unwrap();
    assert!(e4.iter().any(|e| matches!(
        e,
        BattleEvent::ComboUsed {
            combo: 14,
            participants,
            ..
        } if participants.len() == 4
    )));

    // Instant-death combo: BLACK HOLE (GRA + NEGATIS) with high roll kills enemies immediately.
    let party_bh = vec![
        make_member(0, 40, 50, 50, &[13], &[], true),
        make_member(1, 30, 50, 50, &[], &[27], true),
    ];
    let mut rolls_bh = SliceRolls::new(&[0x7F; 256]);
    let (mut b_bh, _) = Battle::start(&formation(2), party_bh, &data, true, &mut rolls_bh).unwrap();
    let e_bh = b_bh
        .round(
            &RoundOrders::Commands(vec![
                Command::Technique {
                    technique: 13,
                    target: None,
                },
                Command::Skill {
                    skill: 27,
                    target: None,
                },
            ]),
            &data,
            &mut rolls_bh,
        )
        .unwrap();
    assert!(
        e_bh.iter()
            .any(|e| matches!(e, BattleEvent::ComboUsed { combo: 10, .. }))
    );
    assert_eq!(b_bh.outcome(), Some(crate::battle::Outcome::Victory));
}

#[test]
fn combo_aborts_if_participant_lacks_tp_is_unarmed_or_separated_by_enemy() {
    let data = base_data();

    // Insufficient TP on P2 prevents FIRE STORM; P1 casts FOI normally.
    let low_tp = vec![
        make_member(0, 30, 20, 30, &[1], &[], true),
        make_member(1, 20, 20, 1, &[10], &[], true), // ZAN costs 4
    ];
    let mut rolls = SliceRolls::new(&[0; 256]);
    let (mut b, _) = Battle::start(&formation(1), low_tp, &data, true, &mut rolls).unwrap();
    let ev = b
        .round(
            &RoundOrders::Commands(vec![
                Command::Technique {
                    technique: 1,
                    target: Some(id(6)),
                },
                Command::Technique {
                    technique: 10,
                    target: None,
                },
            ]),
            &data,
            &mut rolls,
        )
        .unwrap();
    assert!(
        !ev.iter()
            .any(|e| matches!(e, BattleEvent::ComboUsed { .. }))
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, BattleEvent::TechniqueUsed { technique: 1, .. }))
    );

    // Unarmed RAYBLADE (requires_weapon = true) prevents PARADINBLW.
    let unarmed = vec![
        make_member(0, 30, 20, 30, &[], &[2], false),
        make_member(1, 20, 20, 30, &[], &[7], true),
    ];
    let mut rolls2 = SliceRolls::new(&[0; 256]);
    let (mut b2, _) = Battle::start(&formation(1), unarmed, &data, true, &mut rolls2).unwrap();
    let ev2 = b2
        .round(
            &RoundOrders::Commands(vec![
                Command::Skill {
                    skill: 2,
                    target: Some(id(6)),
                },
                Command::Skill {
                    skill: 7,
                    target: Some(id(6)),
                },
            ]),
            &data,
            &mut rolls2,
        )
        .unwrap();
    assert!(
        !ev2.iter()
            .any(|e| matches!(e, BattleEvent::ComboUsed { .. }))
    );

    // Sealed caster prevents FIRE STORM.
    let mut sealed = vec![
        make_member(0, 30, 20, 30, &[1], &[], true),
        make_member(1, 20, 20, 30, &[10], &[], true),
    ];
    sealed[1].stats.status |= status::TECH_SEALED;
    let mut rolls3 = SliceRolls::new(&[0; 256]);
    let (mut b3, _) = Battle::start(&formation(1), sealed, &data, true, &mut rolls3).unwrap();
    let ev3 = b3
        .round(
            &RoundOrders::Commands(vec![
                Command::Technique {
                    technique: 1,
                    target: Some(id(6)),
                },
                Command::Technique {
                    technique: 10,
                    target: None,
                },
            ]),
            &data,
            &mut rolls3,
        )
        .unwrap();
    assert!(
        !ev3.iter()
            .any(|e| matches!(e, BattleEvent::ComboUsed { .. }))
    );

    // Single-target PARADINBLW hits only one enemy when armed.
    let armed = vec![
        make_member(0, 30, 20, 30, &[], &[2], true),
        make_member(1, 20, 20, 30, &[], &[7], true),
    ];
    let mut rolls4 = SliceRolls::new(&[0; 256]);
    let (mut b4, _) = Battle::start(&formation(2), armed, &data, true, &mut rolls4).unwrap();
    let ev4 = b4
        .round(
            &RoundOrders::Commands(vec![
                Command::Skill {
                    skill: 2,
                    target: Some(id(7)),
                },
                Command::Skill {
                    skill: 7,
                    target: Some(id(6)),
                },
            ]),
            &data,
            &mut rolls4,
        )
        .unwrap();
    assert!(
        ev4.iter()
            .any(|e| matches!(e, BattleEvent::ComboUsed { combo: 1, .. }))
    );
    let hits: Vec<FighterId> = ev4
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Resolved {
                actor,
                target,
                damage: Some(_),
                ..
            } if *actor == id(1) => Some(*target),
            _ => None,
        })
        .collect();
    assert_eq!(hits, vec![id(7)]);
    // Skill-only combo (PARADINBLW) sets MAGIC without TECHNIQUE (`loc_2872`
    // only executes `bset #2, $2A(a4)` at `loc_28B4` for `cmd == 2` tech
    // participants, `ps4.asm:4057-4077`).
    let flags = b4.roster().get(id(7)).unwrap().reaction_flags;
    assert_ne!(flags & crate::battle::fighters::reaction::MAGIC, 0);
    assert_eq!(flags & crate::battle::fighters::reaction::TECHNIQUE, 0);
}

struct CountingRolls {
    value: u16,
    count: usize,
}

impl crate::battle::rng::Rolls for CountingRolls {
    fn next_roll(&mut self) -> u16 {
        self.count += 1;
        self.value
    }
}

#[test]
fn all_enemy_combo_consumes_lead_retarget_tie_draws() {
    let mut enemy = fixtures::zoran_bult();
    enemy.hp = 150;
    enemy.agility = 1;
    enemy.mental = 10;
    enemy.mental_defence = 10;
    enemy.defence = 10;
    enemy.properties = [2; 14];
    let data = base_data().with_enemies([enemy]);
    // Round 1: kill enemy slot 6 with single-target FOI while leaving slots 7
    // and 8 untouched (tied at HP deficit 0). Round 2: run all-enemy SYNTH_TWO
    // (FOI + ZAN) twice from identical states — once with FOI aimed at dead
    // slot 6 (triggers `loc_5A98` tie draw between slots 7 and 8 before
    // `loc_27DE7C` / `loc_AF74`), once aimed at living slot 7 (no retarget).
    let make_battle = || {
        let killer = make_member(0, 40, 200, 30, &[1], &[], true);
        let caster = make_member(1, 30, 20, 30, &[10], &[], true);
        let mut rolls = CountingRolls {
            value: 0x3F,
            count: 0,
        };
        let (mut b, _) =
            Battle::start(&formation(3), vec![killer, caster], &data, true, &mut rolls).unwrap();
        // Kill slot 6 on round 1 with single-target FOI while caster defends.
        b.round(
            &RoundOrders::Commands(vec![
                Command::Technique {
                    technique: 1,
                    target: Some(id(6)),
                },
                Command::Defend,
            ]),
            &data,
            &mut rolls,
        )
        .unwrap();
        assert!(!b.roster().get(id(6)).unwrap().is_alive());
        assert!(b.roster().get(id(7)).unwrap().is_alive());
        assert!(b.roster().get(id(8)).unwrap().is_alive());
        b
    };

    let mut b_dead = make_battle();
    let mut rolls_dead = CountingRolls { value: 0, count: 0 };
    let ev_dead = b_dead
        .round(
            &RoundOrders::Commands(vec![
                Command::Technique {
                    technique: 1,
                    target: Some(id(6)),
                },
                Command::Technique {
                    technique: 10,
                    target: None,
                },
            ]),
            &data,
            &mut rolls_dead,
        )
        .unwrap();

    let mut b_live = make_battle();
    let mut rolls_live = CountingRolls { value: 0, count: 0 };
    let ev_live = b_live
        .round(
            &RoundOrders::Commands(vec![
                Command::Technique {
                    technique: 1,
                    target: Some(id(7)),
                },
                Command::Technique {
                    technique: 10,
                    target: None,
                },
            ]),
            &data,
            &mut rolls_live,
        )
        .unwrap();

    assert!(
        ev_dead
            .iter()
            .any(|e| matches!(e, BattleEvent::ComboUsed { combo: 2, .. }))
    );
    assert!(
        ev_live
            .iter()
            .any(|e| matches!(e, BattleEvent::ComboUsed { combo: 2, .. }))
    );
    assert_eq!(rolls_dead.count, rolls_live.count + 1);
}

#[test]
fn instant_death_combo_miss_leaves_enemy_reaction_flags_clear() {
    let mut data = base_data();
    let mut resistant = fixtures::zoran_bult();
    resistant.hp = 500;
    resistant.agility = 1;
    resistant.mental = 250;
    resistant.properties = [0; 14]; // 0 element factor -> guaranteed miss
    data = data.with_enemies([resistant]);

    let party = vec![
        make_member(0, 30, 20, 30, &[13], &[], true),
        make_member(1, 20, 20, 30, &[], &[27], true),
    ];
    let mut rolls = SliceRolls::new(&[0; 256]);
    let (mut b, _) = Battle::start(&formation(1), party, &data, true, &mut rolls).unwrap();
    let ev = b
        .round(
            &RoundOrders::Commands(vec![
                Command::Technique {
                    technique: 13,
                    target: None,
                },
                Command::Skill {
                    skill: 27,
                    target: None,
                },
            ]),
            &data,
            &mut rolls,
        )
        .unwrap();
    assert!(
        ev.iter()
            .any(|e| matches!(e, BattleEvent::ComboUsed { combo: 10, .. }))
    );
    assert!(ev.iter().any(|e| matches!(
        e,
        BattleEvent::Resolved {
            verdict: Verdict::Miss,
            ..
        }
    )));
    // Retail `loc_5FB4` (`ps4.asm:8817`) runs `bset #2, $2A(a4)` where `a4` is
    // the acting lead character from `Character_DoCombo`; the enemy loop
    // (`loc_6020`) never writes enemy `$2A`.
    assert_eq!(b.roster().get(id(6)).unwrap().reaction_flags, 0);
}
