//! Authored, synthetic battle-decision fixture for the Redshirt consumer.
//!
//! No record here describes a retail encounter or a campaign save. The
//! ability *semantics* use the tested RES/RIMIT/CROSSCUT records: see
//! `battle/technique_tests.rs`, `battle/skill_tests.rs`, and `docs/RIMIT.md`
//! (retail RIMIT record at 0x2A9C98). `Battle::start`/`Battle::round` own all
//! combat rules. One JSON-lines step is one full battle round, not a native
//! menu button or evidence of an ordinary-input campaign route.

use std::io::{self, BufRead, Read, Write};
use std::path::Path;

use psiv_core::battle::{
    Battle, BattleData, BattleEvent, Bonuses, CharacterRecord, Command, ELEMENT_SLOTS, EnemyRecord,
    FighterId, FormationEnemy, FormationRecord, ItemKind, ItemRecord, Lcg41, Outcome, PartyMember,
    Reach, Rng2, RoundOrders, Side, Skill, Skipped, Technique, candidate_targets, skill_targets,
    status, technique_targets, weapon_reach,
};
use serde::Deserialize;
use serde_json::{Value, json};

const MAX_ROUNDS: u16 = 12;
const FRAME_START: u16 = 100;
const FRAME_STEP: u16 = 23;
const FIXTURE_SCHEMA: &str = "psiv-synthetic-battle-fixture-v1";
const MAX_FIXTURE_BYTES: usize = 4096;

#[derive(Clone, Copy)]
struct Foe {
    hp: u16,
    attack: u16,
    defence: u16,
    agility: u8,
    dexterity: u8,
}

#[derive(Clone, Copy)]
struct Case {
    id: &'static str,
    split: &'static str,
    seed: u32,
    party_hp: u16,
    party_tp: u16,
    crosscut_uses: u8,
    foes: &'static [Foe],
}

#[derive(Clone)]
struct Scenario {
    id: String,
    split: String,
    seed: u32,
    party_hp: u16,
    party_tp: u16,
    crosscut_uses: u8,
    foes: Vec<Foe>,
    fixture: bool,
}

impl From<Case> for Scenario {
    fn from(case: Case) -> Self {
        Self {
            id: case.id.into(),
            split: case.split.into(),
            seed: case.seed,
            party_hp: case.party_hp,
            party_tp: case.party_tp,
            crosscut_uses: case.crosscut_uses,
            foes: case.foes.to_vec(),
            fixture: false,
        }
    }
}

// Fixed before provider collection. All numbers are authored test values.
const CASES: [Case; 8] = [
    Case {
        id: "calm_single",
        split: "calibration",
        seed: 0x1357_2468,
        party_hp: 92,
        party_tp: 18,
        crosscut_uses: 2,
        foes: &[Foe {
            hp: 32,
            attack: 30,
            defence: 3,
            agility: 8,
            dexterity: 8,
        }],
    },
    Case {
        id: "wounded_single",
        split: "calibration",
        seed: 0x2468_1357,
        party_hp: 49,
        party_tp: 18,
        crosscut_uses: 2,
        foes: &[Foe {
            hp: 38,
            attack: 31,
            defence: 4,
            agility: 9,
            dexterity: 9,
        }],
    },
    Case {
        id: "split_pair",
        split: "calibration",
        seed: 0x3141_5926,
        party_hp: 81,
        party_tp: 18,
        crosscut_uses: 2,
        foes: &[
            Foe {
                hp: 29,
                attack: 27,
                defence: 3,
                agility: 8,
                dexterity: 8,
            },
            Foe {
                hp: 43,
                attack: 26,
                defence: 5,
                agility: 10,
                dexterity: 9,
            },
        ],
    },
    Case {
        id: "sturdy_pair",
        split: "calibration",
        seed: 0x2718_2818,
        party_hp: 67,
        party_tp: 21,
        crosscut_uses: 2,
        foes: &[
            Foe {
                hp: 49,
                attack: 28,
                defence: 6,
                agility: 9,
                dexterity: 8,
            },
            Foe {
                hp: 37,
                attack: 27,
                defence: 4,
                agility: 11,
                dexterity: 10,
            },
        ],
    },
    Case {
        id: "lean_single",
        split: "held_out",
        seed: 0x4242_1717,
        party_hp: 73,
        party_tp: 15,
        crosscut_uses: 1,
        foes: &[Foe {
            hp: 54,
            attack: 32,
            defence: 5,
            agility: 11,
            dexterity: 9,
        }],
    },
    Case {
        id: "wounded_pair",
        split: "held_out",
        seed: 0x5eed_1234,
        party_hp: 55,
        party_tp: 21,
        crosscut_uses: 2,
        foes: &[
            Foe {
                hp: 34,
                attack: 27,
                defence: 4,
                agility: 9,
                dexterity: 8,
            },
            Foe {
                hp: 47,
                attack: 28,
                defence: 6,
                agility: 10,
                dexterity: 10,
            },
        ],
    },
    Case {
        id: "uneven_pair",
        split: "held_out",
        seed: 0x6a09_e667,
        party_hp: 88,
        party_tp: 18,
        crosscut_uses: 2,
        foes: &[
            Foe {
                hp: 25,
                attack: 28,
                defence: 3,
                agility: 12,
                dexterity: 8,
            },
            Foe {
                hp: 61,
                attack: 27,
                defence: 7,
                agility: 7,
                dexterity: 9,
            },
        ],
    },
    Case {
        id: "thin_reserve",
        split: "held_out",
        seed: 0x7f4a_7c15,
        party_hp: 58,
        party_tp: 12,
        crosscut_uses: 1,
        foes: &[
            Foe {
                hp: 44,
                attack: 28,
                defence: 5,
                agility: 11,
                dexterity: 9,
            },
            Foe {
                hp: 32,
                attack: 27,
                defence: 3,
                agility: 9,
                dexterity: 8,
            },
        ],
    },
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureInput {
    schema: String,
    id: String,
    seed: u32,
    party: FixtureParty,
    foes: Vec<FixtureFoe>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureParty {
    hp: u16,
    tp: u16,
    crosscut_uses: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureFoe {
    hp: u16,
    attack: u16,
    defence: u16,
    agility: u8,
    dexterity: u8,
}

fn fixture(path: &Path) -> Result<Scenario, &'static str> {
    let file = std::fs::File::open(path).map_err(|_| "fixture_unreadable")?;
    let mut bytes = Vec::new();
    file.take((MAX_FIXTURE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "fixture_unreadable")?;
    if bytes.len() > MAX_FIXTURE_BYTES {
        return Err("fixture_too_large");
    }
    // Struct deserialization rejects unknown, missing, duplicate and wrong-type
    // fields before any battle object is created.
    let value: FixtureInput = serde_json::from_slice(&bytes).map_err(|_| "fixture_invalid_json")?;
    if value.schema != FIXTURE_SCHEMA {
        return Err("fixture_invalid_schema");
    }
    let id_bytes = value.id.as_bytes();
    if id_bytes.is_empty()
        || id_bytes.len() > 40
        || !id_bytes[0].is_ascii_lowercase()
        || !id_bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
    {
        return Err("fixture_invalid_id");
    }
    if !(1..=100).contains(&value.party.hp) || value.party.tp > 24 || value.party.crosscut_uses > 8
    {
        return Err("fixture_invalid_party");
    }
    if !(1..=2).contains(&value.foes.len()) {
        return Err("fixture_invalid_foes");
    }
    let mut foes = Vec::with_capacity(value.foes.len());
    for row in value.foes {
        if !(1..=200).contains(&row.hp)
            || !(1..=255).contains(&row.attack)
            || row.defence > 255
            || row.agility == 0
            || row.dexterity == 0
        {
            return Err("fixture_invalid_foes");
        }
        foes.push(Foe {
            hp: row.hp,
            attack: row.attack,
            defence: row.defence,
            agility: row.agility,
            dexterity: row.dexterity,
        });
    }
    Ok(Scenario {
        id: value.id,
        split: "authored_fixture".into(),
        seed: value.seed,
        party_hp: value.party.hp,
        party_tp: value.party.tp,
        crosscut_uses: value.party.crosscut_uses,
        foes,
        fixture: true,
    })
}

fn case_config(case: &Scenario) -> Value {
    let mut config = json!({
        "schema": "psiv-synthetic-battle-v1",
        "id": case.id, "split": case.split, "seed": case.seed,
        "rng": {"kind": "Rng2/HV_SURROGATE", "frame_start": FRAME_START, "frame_step": FRAME_STEP},
        "round_limit": MAX_ROUNDS,
        "party": {"name": "Pilot", "max_hp": 100, "hp": case.party_hp, "max_tp": 24,
                  "tp": case.party_tp, "strength": 16, "mental": 14, "agility": 18,
                  "dexterity": 18, "crosscut_uses": case.crosscut_uses,
                  "equipment": "authored single-target blade plus shield",
                  "techniques": ["RES", "RIMIT"], "skill": "CROSSCUT"},
        "foes": case.foes.iter().enumerate().map(|(index, foe)| json!({
            "slot": index + 1, "name": format!("Foe {}", index + 1), "hp": foe.hp,
            "attack": foe.attack, "defence": foe.defence,
            "agility": foe.agility, "dexterity": foe.dexterity,
            "regular_abilities": "plain attack only"
        })).collect::<Vec<_>>()
    });
    if case.fixture {
        config["source"] = json!(FIXTURE_SCHEMA);
    }
    config
}

fn data(case: &Scenario) -> BattleData {
    let enemies = case
        .foes
        .iter()
        .enumerate()
        .map(|(index, foe)| EnemyRecord {
            id: 100 + index as u16,
            name: format!("Foe {}", index + 1),
            hp: foe.hp,
            strength: 10,
            mental: 8,
            agility: foe.agility,
            dexterity: foe.dexterity,
            attack: foe.attack,
            defence: foe.defence,
            mental_defence: 4,
            attack_element: 1,
            attack_status: 0,
            properties: [2; ELEMENT_SLOTS],
            regular_abilities: [0; 8],
            condition_ids: [0; 4],
            conditional_abilities: [0; 4],
            experience: 0,
            meseta: 0,
        });
    // Exact supported record fields from the core's existing ability tests;
    // only the fighter/equipment/foe values above are authored.
    BattleData::new()
        .with_enemies(enemies)
        .with_items([
            ItemRecord {
                id: 1,
                name: "Blade".into(),
                kind: ItemKind::OneHandedSingleTarget,
                bonuses: Bonuses {
                    attack: 8,
                    ..Bonuses::default()
                },
                element: 1,
            },
            ItemRecord {
                id: 2,
                name: "Guard".into(),
                kind: ItemKind::Shield,
                bonuses: Bonuses {
                    defence: 5,
                    ..Bonuses::default()
                },
                element: 0,
            },
        ])
        .with_techniques([
            Technique {
                id: 24,
                name: "RES".into(),
                effect: 18,
                cost: 3,
                targeting: 0x34,
                power: 16,
                resistance: 0,
                element: 0,
            },
            Technique {
                id: 23,
                name: "RIMIT".into(),
                effect: 7,
                cost: 10,
                targeting: 0x12,
                power: 48,
                resistance: 2,
                element: 11,
            },
        ])
        .with_skills([Skill {
            id: 1,
            name: "CROSSCUT".into(),
            effect: 1,
            power_stat: 5,
            requires_weapon: true,
            targeting: 0x11,
            power: 80,
            resistance: 6,
            element: 16,
        }])
}

fn member(case: &Scenario, data: &BattleData) -> PartyMember {
    let mut techniques = [0; 16];
    techniques[..2].copy_from_slice(&[24, 23]);
    let record = CharacterRecord {
        id: 0,
        name: "Pilot".into(),
        profession: 0,
        level: 1,
        experience: 0,
        hp: 100,
        max_hp: 100,
        tp: case.party_tp,
        max_tp: 24,
        strength: 16,
        mental: 14,
        agility: 18,
        dexterity: 18,
        properties: [2; ELEMENT_SLOTS],
        equipment: [1, 2, 0, 0],
        techniques,
        skills: [1, 0, 0, 0, 0, 0, 0, 0],
        skill_uses: [case.crosscut_uses, 0, 0, 0, 0, 0, 0, 0],
    };
    let mut seated = PartyMember::seat(&record, data).expect("authored equipment exists");
    seated.stats.curr_hp = case.party_hp;
    seated
}

#[derive(Clone)]
struct Choice {
    id: String,
    description: String,
    command: Command,
}

struct Probe {
    case: Scenario,
    data: BattleData,
    battle: Battle,
    rng: Lcg41,
    revision: u16,
    last: Option<Value>,
}

impl Probe {
    fn new(case: Scenario) -> Self {
        let data = data(&case);
        let formation = FormationRecord {
            id: 900,
            ambush_chance: 0,
            run_chance: 0xf0,
            drop_rate: 0,
            drop_item: None,
            enemies: case
                .foes
                .iter()
                .enumerate()
                .map(|(index, _)| FormationEnemy {
                    slot: (index + 1) as u8,
                    enemy_id: 100 + index as u16,
                    position: 0,
                })
                .collect(),
        };
        let party = vec![member(&case, &data)];
        let mut rng = Lcg41::new(case.seed);
        let (battle, _) = Battle::start(
            &formation,
            party,
            &data,
            false,
            0,
            &mut Rng2::with_surrogate(&mut rng, FRAME_START),
        )
        .expect("authored formation resolves");
        Self {
            case,
            data,
            battle,
            rng,
            revision: 0,
            last: None,
        }
    }

    fn legal(&self) -> Vec<Choice> {
        if self.battle.outcome().is_some() || self.battle.round_number() >= MAX_ROUNDS {
            return Vec::new();
        }
        let actor = FighterId::new(1).expect("party slot one");
        let roster = self.battle.roster();
        let Some(pilot) = roster.get(actor) else {
            return Vec::new();
        };
        if !pilot.is_alive() || !pilot.stats.can_act() {
            return Vec::new();
        }
        let mut choices = Vec::new();
        if let Ok(Some(reach)) = weapon_reach(&pilot.stats, &self.data) {
            match reach {
                Reach::Single => {
                    for foe in roster.living(Side::Enemy) {
                        let id = foe.id.get();
                        if candidate_targets(roster, actor, Some(foe.id), reach) == vec![foe.id] {
                            choices.push(Choice {
                                id: format!("attack_{id}"),
                                description: format!(
                                    "Attack {} (fighter {id}, {}/{} HP); free physical swing.",
                                    foe.name, foe.stats.curr_hp, foe.stats.max_hp
                                ),
                                command: Command::AttackTarget(foe.id),
                            });
                        }
                    }
                }
                Reach::All => choices.push(Choice {
                    id: "attack_all".into(),
                    description: "Attack all living enemies; free physical swing.".into(),
                    command: Command::Attack,
                }),
            }
        }
        choices.push(Choice {
            id: "defend".into(),
            description: "Defend this round; reduce physical damage after acting; free.".into(),
            command: Command::Defend,
        });
        if pilot.stats.status & status::TECH_SEALED == 0 {
            if let Some(res) = self.data.technique(24)
                && res.supported()
                && pilot.stats.curr_tp >= u16::from(res.cost)
            {
                for target in technique_targets(roster, actor, res) {
                    let id = target.get();
                    let recipient = roster.get(target).expect("eligible target");
                    choices.push(Choice {
                        id: format!("res_{id}"),
                        description: format!(
                            "RES on {} (fighter {id}, {}/{} HP); costs 3 TP; restores HP.",
                            recipient.name, recipient.stats.curr_hp, recipient.stats.max_hp
                        ),
                        command: Command::Technique {
                            technique: 24,
                            target: Some(target),
                        },
                    });
                }
            }
            if let Some(rimit) = self.data.technique(23)
                && rimit.supported()
                && pilot.stats.curr_tp >= u16::from(rimit.cost)
                && !technique_targets(roster, actor, rimit).is_empty()
            {
                choices.push(Choice {
                    id: "rimit_all".into(),
                    description: "RIMIT on all living enemies; costs 10 TP; may cause sleep."
                        .into(),
                    command: Command::Technique {
                        technique: 23,
                        target: None,
                    },
                });
            }
        }
        if let Some(crosscut) = self.data.skill(1)
            && crosscut.supported()
            && pilot.stats.skills.contains(&1)
            && pilot.stats.curr_skill_uses[0] > 0
            && weapon_reach(&pilot.stats, &self.data)
                .ok()
                .flatten()
                .is_some()
        {
            for target in skill_targets(roster, actor, crosscut) {
                let id = target.get();
                let foe = roster.get(target).expect("eligible target");
                choices.push(Choice { id: format!("crosscut_{id}"),
                    description: format!("CROSSCUT {} (fighter {id}, {}/{} HP); costs one skill use; two hit attempts.",
                                         foe.name, foe.stats.curr_hp, foe.stats.max_hp),
                    command: Command::Skill { skill: 1, target: Some(target) } });
            }
        }
        choices
    }

    fn state(&self) -> Value {
        let fighters = |side| {
            self.battle
                .roster()
                .side(side)
                .map(|fighter| {
                    let base = json!({"id": fighter.id.get(), "name": fighter.name,
                              "hp": fighter.stats.curr_hp, "max_hp": fighter.stats.max_hp,
                              "status": fighter.stats.status,
                              "attack": fighter.stats.attack.battle,
                              "defence": fighter.stats.defence.battle,
                              "agility": fighter.stats.agility.battle,
                              "dexterity": fighter.stats.dexterity.battle,
                              "mental": fighter.stats.mental.battle});
                    if side == Side::Party {
                        let mut value = base;
                        value["tp"] = json!(fighter.stats.curr_tp);
                        value["max_tp"] = json!(fighter.stats.max_tp);
                        value["crosscut_uses"] = json!(fighter.stats.curr_skill_uses[0]);
                        value
                    } else {
                        base
                    }
                })
                .collect::<Vec<_>>()
        };
        let outcome = match self.battle.outcome() {
            Some(Outcome::Victory) => "victory",
            Some(Outcome::Defeat) => "defeat",
            Some(Outcome::Escaped) => "escaped",
            Some(Outcome::ScriptedExit) => "scripted_exit",
            None => "ongoing",
        };
        json!({"schema": "psiv-synthetic-battle-state-v1", "case": self.case.id,
               "revision": self.revision, "round": self.battle.round_number(),
               "round_limit": MAX_ROUNDS, "outcome": outcome,
               "party": fighters(Side::Party), "enemies": fighters(Side::Enemy),
               "legal": self.legal().iter().map(|choice| json!({
                   "id": choice.id, "description": choice.description
               })).collect::<Vec<_>>(),
               "last": self.last})
    }

    fn step(&mut self, revision: u16, action: &str) -> Value {
        if revision != self.revision {
            return json!({"ok": false, "error": "stale_revision"});
        }
        let Some(chosen) = self.legal().into_iter().find(|choice| choice.id == action) else {
            return json!({"ok": false, "error": "unavailable_action"});
        };
        let before_party_hp = self
            .battle
            .roster()
            .get(FighterId::new(1).expect("party slot one"))
            .expect("authored pilot")
            .stats
            .curr_hp;
        let before_enemy_hp: u32 = self
            .battle
            .roster()
            .side(Side::Enemy)
            .map(|fighter| u32::from(fighter.stats.curr_hp))
            .sum();
        let before = self.battle.round_number();
        let frame = FRAME_START.wrapping_add(FRAME_STEP.wrapping_mul(before + 1));
        let events = match self.battle.round(
            &RoundOrders::Commands(vec![chosen.command]),
            &self.data,
            &mut Rng2::with_surrogate(&mut self.rng, frame),
        ) {
            Ok(events) => events,
            Err(_) => return json!({"ok": false, "error": "battle_error"}),
        };
        self.revision += 1;
        let actor = FighterId::new(1).expect("party slot one");
        let applied = events.iter().any(|event| match (chosen.command, event) {
            (
                Command::Attack | Command::AttackTarget(_),
                BattleEvent::Attacked { actor: who, .. },
            ) => *who == actor,
            (Command::Defend, BattleEvent::Defended { actor: who }) => *who == actor,
            (
                Command::Technique { technique, .. },
                BattleEvent::TechniqueUsed {
                    actor: who,
                    technique: used,
                    ..
                },
            ) => *who == actor && *used == technique,
            (
                Command::Skill { skill, .. },
                BattleEvent::SkillUsed {
                    actor: who,
                    skill: used,
                    ..
                },
            ) => *who == actor && *used == skill,
            _ => false,
        });
        let rejected = events.iter().any(|event| matches!(event,
            BattleEvent::TechniqueRejected { actor: who, .. } | BattleEvent::SkillRejected { actor: who, .. }
                if *who == actor));
        let queued_after_enemy = events.iter().any(|event| match event {
            BattleEvent::RoundBegan { order, .. } => order
                .iter()
                .position(|fighter| *fighter == actor)
                .is_some_and(|index| {
                    index > 0 && order[..index].iter().any(|id| id.side() == Side::Enemy)
                }),
            _ => false,
        });
        let skipped = events.iter().find_map(|event| match event {
            BattleEvent::TurnSkipped { actor: who, reason } if *who == actor => {
                Some(match reason {
                    Skipped::Dead => "dead",
                    Skipped::Incapacitated => "incapacitated",
                    Skipped::NoTarget => "no_target",
                    Skipped::Unarmed => "unarmed",
                    Skipped::JustRevived => "just_revived",
                })
            }
            _ => None,
        });
        let defeat_before_turn = !applied
            && !rejected
            && skipped.is_none()
            && queued_after_enemy
            && self.battle.outcome() == Some(Outcome::Defeat)
            && self
                .battle
                .roster()
                .get(actor)
                .is_some_and(|fighter| fighter.stats.curr_hp == 0)
            && events.iter().any(|event| {
                matches!(
                    event,
                    BattleEvent::Ended {
                        outcome: Outcome::Defeat
                    }
                )
            });
        let skipped_reason = skipped.or(if defeat_before_turn {
            Some("defeat_before_turn")
        } else {
            None
        });
        let healed: u16 = events
            .iter()
            .filter_map(|event| match event {
                BattleEvent::Healed {
                    actor: who, amount, ..
                } if *who == actor => Some(*amount),
                _ => None,
            })
            .sum();
        let sleep_effective = events
            .iter()
            .filter_map(|event| match event {
                BattleEvent::FellAsleep { actor: who, target } if *who == actor => Some(*target),
                _ => None,
            })
            .any(|target| {
                self.battle
                    .roster()
                    .get(target)
                    .is_some_and(|f| f.is_alive() && f.stats.status & status::ASLEEP != 0)
                    || events.iter().any(|event| {
                        matches!(event,
                    BattleEvent::TurnSkipped { actor: skipped, .. } if *skipped == target)
                    })
            });
        let after_party_hp = self
            .battle
            .roster()
            .get(actor)
            .expect("authored pilot")
            .stats
            .curr_hp;
        let after_enemy_hp: u32 = self
            .battle
            .roster()
            .side(Side::Enemy)
            .map(|fighter| u32::from(fighter.stats.curr_hp))
            .sum();
        self.last = Some(json!({"revision": self.revision, "selected": action,
                                "applied": applied, "rejected": rejected,
                                "skipped_reason": skipped_reason,
                                "party_hp_delta": i32::from(after_party_hp) - i32::from(before_party_hp),
                                "party_hp_damage": (u32::from(before_party_hp) + u32::from(healed))
                                    .saturating_sub(u32::from(after_party_hp)),
                                "enemy_hp_lost": before_enemy_hp.saturating_sub(after_enemy_hp),
                                "healed": healed, "sleep_effective": sleep_effective}));
        json!({"ok": true, "acknowledged": true, "revision": self.revision})
    }
}

fn run(case: Scenario) -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let mut probe = Probe::new(case.clone());
    for line in stdin.lock().lines() {
        let line = line?;
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => {
                writeln!(stdout, "{}", json!({"ok": false, "error": "invalid_json"}))?;
                stdout.flush()?;
                continue;
            }
        };
        let reply = match request.get("op").and_then(Value::as_str) {
            Some("reset") if request.as_object().is_some_and(|obj| obj.len() == 1) => {
                probe = Probe::new(case.clone());
                json!({"ok": true, "state": probe.state()})
            }
            Some("state") if request.as_object().is_some_and(|obj| obj.len() == 1) => {
                json!({"ok": true, "state": probe.state()})
            }
            Some("step") if request.as_object().is_some_and(|obj| obj.len() == 3) => {
                match (
                    request.get("revision").and_then(Value::as_u64),
                    request.get("action").and_then(Value::as_str),
                ) {
                    (Some(revision), Some(action)) if revision <= u16::MAX as u64 => {
                        probe.step(revision as u16, action)
                    }
                    _ => json!({"ok": false, "error": "invalid_step"}),
                }
            }
            Some("close") if request.as_object().is_some_and(|obj| obj.len() == 1) => {
                writeln!(stdout, "{}", json!({"ok": true}))?;
                stdout.flush()?;
                break;
            }
            _ => json!({"ok": false, "error": "invalid_request"}),
        };
        writeln!(stdout, "{reply}")?;
        stdout.flush()?;
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--list-cases"] {
        println!(
            "{}",
            json!(
                CASES
                    .iter()
                    .map(|case| json!({
                        "id": case.id, "split": case.split, "seed": case.seed
                    }))
                    .collect::<Vec<_>>()
            )
        );
        return;
    }
    let (kind, value, describe) = match args.as_slice() {
        [flag, value] if flag == "--case" || flag == "--fixture" => {
            (flag.as_str(), value.as_str(), false)
        }
        [flag, value, description]
            if (flag == "--case" || flag == "--fixture") && description == "--describe" =>
        {
            (flag.as_str(), value.as_str(), true)
        }
        _ => {
            eprintln!(
                "usage: redshirt_battle (--case ID | --fixture PATH) [--describe] | --list-cases"
            );
            std::process::exit(2);
        }
    };
    let case = if kind == "--case" {
        let Some(case) = CASES.iter().find(|case| case.id == value).copied() else {
            eprintln!("unknown case");
            std::process::exit(2);
        };
        Scenario::from(case)
    } else {
        match fixture(Path::new(value)) {
            Ok(case) => case,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        }
    };
    if describe {
        println!("{}", case_config(&case));
    } else if run(case).is_err() {
        eprintln!("probe I/O error");
        std::process::exit(1);
    }
}
