//! Tape 07's first battle, replayed against the cartridge's own rolls.
//!
//! `oracle/battle_fixture.py` turns the oracle's RNG trace and RAM log into
//! `replay_fixtures/tape07_first_battle.json`: the battle's start state, the
//! rolls the cartridge drew between its first round's queue build and the party
//! wiping the field out - each with the frame it was drawn in and the role it
//! played - and what the log shows every action doing. This module builds the
//! battle the fixture describes, feeds those rolls through [`SliceRolls`], and
//! compares the port's timeline with the cartridge's, action by action.
//!
//! A roll is `hv + frame_count - high_word(RNG_Seed)`: `UpdateRNGSeed2`
//! (`ps4.asm:86097`) subtracts the word at `$FFFFEF0C`, which on a big-endian
//! 68000 is the seed longword's *high* half. The fixture derives each roll from
//! the trace's raw columns for that reason; `docs/BATTLE_ORACLE_REPLAY.md` has
//! the proof, the trace's own `roll` column being the low-half subtraction.
//!
//! # Two ways to feed one stream
//!
//! The cartridge draws rolls the port has no consumer for: `Enemy_Attack`
//! re-rolls its ability while the draw equals `$FFFFEEA8` (`ps4.asm:19146`),
//! and the encounter's formation draw and the post-victory item drop draw
//! happen outside the battle's own state machine. Feeding the cartridge's
//! stream verbatim therefore puts the port one roll behind at the enemy's turn.
//! (The other divergence the ledger's first cut held — `AlysKyraAttack_Init`
//! running `loc_B6A2` a second time, `ps4.asm:13975-13976`) is modelled now:
//! `resolve_attack` draws both hit passes for Alys and Kyra, so her swing
//! consumes the 36 calls the log's own frames hold.)
//!
//! So the same rolls are replayed two ways:
//!
//! * [`tape07_orders_its_rounds_the_way_the_cartridge_did`] feeds each round
//!   the cartridge's rolls in order and asserts the part that has to line up
//!   before any divergence, then pins the divergence itself.
//! * [`tape07s_actions_match_once_the_unmodelled_rolls_are_removed`] feeds each
//!   round the rolls of the roles the port models and asserts the whole battle,
//!   plus that the port consumed exactly those rolls.
//!
//! [`tape07_full_battle_diverges_at_alyss_swing_draw_count`] is the full-battle
//! assertion on the verbatim stream, parked behind `#[ignore]`.

use super::*;

use crate::battle::FormationEnemy;
use serde::Deserialize;

const FIXTURE: &str = include_str!("replay_fixtures/tape07_first_battle.json");

// ---------------------------------------------------------------------------
// The fixture's shape
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Fixture {
    format_version: u32,
    provenance: Provenance,
    formation: Formation,
    party: Vec<PartyEntry>,
    rolls: RollTable,
    outside_rolls: RollTable,
    rounds: Vec<Round>,
    outcome: OutcomeEntry,
}

#[derive(Debug, Deserialize)]
struct Provenance {
    trace_sha256: String,
    roll_count: u32,
    battle_roll_count: u32,
    roll_column: RollColumn,
    #[allow(dead_code)]
    undetermined: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RollColumn {
    agrees: u32,
    subtracts_low_word: u32,
}

#[derive(Debug, Deserialize)]
struct Formation {
    ambush_chance: u8,
    run_chance: u8,
    drop_rate: u8,
    priority: u16,
    enemies: Vec<FormationEnemyEntry>,
}

#[derive(Debug, Deserialize)]
struct FormationEnemyEntry {
    formation_slot: u8,
    id: u8,
    enemy_id: u16,
    hp: u16,
    status: u8,
    strength: u16,
    mental: u16,
    agility: u16,
    dexterity: u16,
    attack: u16,
    defence: u16,
}

#[derive(Debug, Deserialize)]
struct PartyEntry {
    id: u8,
    name: String,
    level: u16,
    hp: u16,
    max_hp: u16,
    tp: u16,
    max_tp: u16,
    status: u8,
    strength: u16,
    mental: u16,
    agility: u16,
    dexterity: u16,
    attack: u16,
    defence: u16,
}

/// The fixture's roll table: one row per call, and the column names it was
/// written with so a reader cannot mix them up silently.
#[derive(Debug, Deserialize)]
struct RollTable {
    columns: Vec<String>,
    rows: Vec<RollRow>,
}

/// `[frame, roll, role, target, pass, round, action]`. A tuple rather than a
/// map: the table is 134 rows, and `json.dump` puts every array element on its
/// own line.
#[derive(Debug, Clone, Deserialize)]
struct RollRow(u32, u16, String, Option<u8>, u8, u16, u8);

#[derive(Debug, Clone)]
struct Roll {
    frame: u32,
    roll: u16,
    role: String,
    target: Option<u8>,
    pass_number: u8,
    round: u16,
    action: u8,
}

const ROLL_COLUMNS: [&str; 7] = ["frame", "roll", "role", "target", "pass", "round", "action"];

impl RollTable {
    fn rolls(&self) -> Vec<Roll> {
        assert_eq!(self.columns, ROLL_COLUMNS, "the roll table's columns");
        self.rows
            .iter()
            .map(|row| Roll {
                frame: row.0,
                roll: row.1,
                role: row.2.clone(),
                target: row.3,
                pass_number: row.4,
                round: row.5,
                action: row.6,
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct Round {
    round: u16,
    order_frame: u32,
    order: Vec<u8>,
    ordering: Vec<u16>,
    roll_count: u32,
    actions: Vec<Action>,
}

#[derive(Debug, Deserialize)]
struct Action {
    actor: u8,
    start_frame: u32,
    end_frame: u32,
    roll_count: u32,
    targets: Vec<Target>,
}

#[derive(Debug, Clone, Deserialize)]
struct Target {
    id: u8,
    hit: String,
    damage: Option<u16>,
    hp_after: i32,
    died: bool,
}

#[derive(Debug, Deserialize)]
struct OutcomeEntry {
    victory: bool,
    experience_total: u16,
    meseta: u16,
    dead_enemy_ids: Vec<u8>,
}

fn fixture() -> Fixture {
    let parsed: Fixture =
        serde_json::from_str(FIXTURE).expect("the fixture is the extractor's output");
    assert_eq!(
        parsed.format_version, 1,
        "a fixture version this test reads"
    );
    parsed
}

// ---------------------------------------------------------------------------
// Building the battle the fixture describes
// ---------------------------------------------------------------------------

/// The fixture's records, checked against the hand-built fixtures' derivation.
///
/// `fixtures::alys()` and friends are the pack's numbers; the fixture's are
/// the oracle's live RAM. They have to agree, and this is where a drift in
/// either shows up.
fn party_member(entry: &PartyEntry, data: &BattleData) -> PartyMember {
    let record = match entry.id {
        1 => fixtures::alys(),
        2 => fixtures::chaz(),
        3 => fixtures::hahn(),
        other => panic!("no fixture record for character {other}"),
    };
    assert_eq!(record.name, entry.name, "character {0}", entry.id);
    assert_eq!(record.hp, entry.max_hp, "character {} max_hp", entry.id);
    assert_eq!(record.tp, entry.max_tp, "character {} max_tp", entry.id);
    let member = member(&record, data);
    let stats = &member.stats;
    let oracle = [
        ("level", stats.level, entry.level),
        ("strength", u16::from(stats.strength.battle), entry.strength),
        ("mental", u16::from(stats.mental.battle), entry.mental),
        ("agility", u16::from(stats.agility.battle), entry.agility),
        (
            "dexterity",
            u16::from(stats.dexterity.battle),
            entry.dexterity,
        ),
        ("attack", stats.attack.battle, entry.attack),
        ("defence", stats.defence.battle, entry.defence),
    ];
    for (what, derived, logged) in oracle {
        assert_eq!(
            derived, logged,
            "character {}: {what} derived from the record is not the oracle's",
            entry.id
        );
    }
    // The log's live HP, TP and status are what the battle starts with.
    let mut member = member;
    assert_eq!(
        member.stats.curr_hp, entry.hp,
        "character {}: HP the log shows at the battle's start",
        entry.id
    );
    assert_eq!(member.stats.curr_tp, entry.tp, "character {} TP", entry.id);
    assert_eq!(
        member.stats.status, entry.status,
        "character {} status",
        entry.id
    );
    member.stats.curr_hp = entry.hp;
    member.stats.curr_tp = entry.tp;
    member
}

fn formation(fixture: &Fixture) -> FormationRecord {
    FormationRecord {
        // The log carries no formation index (the encounter's `loc_7E4C` masks
        // it out of the draw before the battle exists), and nothing in the
        // battle reads one back.
        id: 0,
        ambush_chance: fixture.formation.ambush_chance,
        run_chance: fixture.formation.run_chance,
        drop_rate: fixture.formation.drop_rate,
        // `dropped_item` is the *result* byte the log shows, not the formation
        // header's item; the cartridge dropped nothing here.
        drop_item: None,
        enemies: fixture
            .formation
            .enemies
            .iter()
            .map(|entry| FormationEnemy {
                slot: entry.formation_slot,
                enemy_id: entry.enemy_id,
                // `position` is the on-screen placement byte.
                position: 0,
            })
            .collect(),
    }
}

/// Starts the battle from the fixture's state on the given stream.
fn start(fixture: &Fixture, data: &BattleData, rolls: &mut impl Rolls) -> Battle {
    let party = fixture
        .party
        .iter()
        .map(|entry| party_member(entry, data))
        .collect();
    let (battle, events) = Battle::start(&formation(fixture), party, data, false, rolls)
        .expect("the fixture's formation resolves");

    let started = events
        .iter()
        .find_map(|event| match event {
            BattleEvent::Started { priority, enemies } => Some((*priority, enemies.clone())),
            _ => None,
        })
        .expect("Battle::start reports what it rolled");
    assert_eq!(
        started.0,
        match fixture.formation.priority {
            0 => Priority::Normal,
            1 => Priority::Preemptive,
            _ => Priority::Ambush,
        },
        "the priority draw is the cartridge's own roll"
    );
    let expected: Vec<FighterId> = fixture.formation.enemies.iter().map(|e| id(e.id)).collect();
    assert_eq!(
        started.1, expected,
        "the formation seats the cartridge's enemies"
    );

    for entry in &fixture.formation.enemies {
        let fighter = battle.roster().get(id(entry.id)).expect("seated");
        let stats = &fighter.stats;
        assert_eq!(stats.curr_hp, entry.hp, "enemy {} HP", entry.id);
        assert_eq!(u32::from(stats.status), u32::from(entry.status));
        assert_eq!(u32::from(stats.attack.battle), u32::from(entry.attack));
        assert_eq!(u32::from(stats.defence.battle), u32::from(entry.defence));
        assert_eq!(u32::from(stats.agility.battle), u32::from(entry.agility));
        assert_eq!(u32::from(stats.strength.battle), u32::from(entry.strength));
        assert_eq!(u32::from(stats.mental.battle), u32::from(entry.mental));
        assert_eq!(
            u32::from(stats.dexterity.battle),
            u32::from(entry.dexterity)
        );
    }
    battle
}

fn orders(fixture: &Fixture, round: &Round) -> RoundOrders {
    // The fixture records one `attack` per party slot the round drew up; the
    // engine's own default for an absent slot is the same command, so this is
    // also what `RoundOrders::attack_all()` would say.
    let commands: Vec<Command> = fixture.party.iter().map(|_| Command::Attack).collect();
    assert_eq!(
        commands.len(),
        fixture.party.len(),
        "round {}: one command per party member",
        round.round
    );
    RoundOrders::Commands(commands)
}

// ---------------------------------------------------------------------------
// The two streams
// ---------------------------------------------------------------------------

/// The cartridge's rolls for a round, in the order it drew them.
fn verbatim(fixture: &Fixture, round: u16) -> Vec<u16> {
    fixture
        .rolls
        .rolls()
        .iter()
        .filter(|roll| roll.round == round)
        .map(|roll| roll.roll)
        .collect()
}

/// The same rolls, minus the ones no port consumer models.
///
/// Kept: the round's order pass, **both** hit passes per action (the second is
/// `AlysKyraAttack_Init`'s, and `resolve_attack` draws it — see
/// `takes_second_hit_pass`), the enemy ability roll, and every damage run.
/// Dropped: the ability re-roll, and - in the fixture's separate
/// `outside_rolls` - the encounter's formation draw and the post-victory item
/// drop.
fn modelled(fixture: &Fixture, round: u16) -> Vec<u16> {
    fixture
        .rolls
        .rolls()
        .iter()
        .filter(|roll| roll.round == round)
        .filter(|roll| matches!(roll.role.as_str(), "order" | "hit" | "ability" | "damage"))
        .map(|roll| roll.roll)
        .collect()
}

// ---------------------------------------------------------------------------
// The comparator
// ---------------------------------------------------------------------------

fn verdict_of(hit: &str) -> Verdict {
    match hit {
        "00" => Verdict::Normal,
        "01" => Verdict::Critical,
        other => panic!("a resolved target with hit flag {other} is not a hit"),
    }
}

/// The first place a round's timeline disagrees with the log.
///
/// Walking the timeline in the order the port resolves it is what makes "the
/// first divergence" well defined: the queue comes first, then each action's
/// swing, its per-target resolutions, and its deaths.
#[derive(Debug, PartialEq)]
enum Divergence {
    /// The queue build.
    Queue {
        round: u16,
        port: Vec<FighterId>,
        log: Vec<FighterId>,
    },
    /// A fighter the log shows resolving targets never swung.
    NoSwing {
        frame: u32,
        actor: FighterId,
        log_targets: usize,
    },
    /// A swing's target list.
    Targets {
        frame: u32,
        actor: FighterId,
        port: Vec<FighterId>,
        log: Vec<FighterId>,
    },
    /// A target resolved for the log was never resolved by the port.
    NoResolution {
        frame: u32,
        actor: FighterId,
        target: FighterId,
    },
    /// A verdict or a damage value.
    Value {
        frame: u32,
        actor: FighterId,
        target: FighterId,
        port: Verdict,
        port_damage: Option<u16>,
        log_hit: u8,
        log_damage: Option<u16>,
    },
    /// HP left on the target.
    Hp {
        frame: u32,
        actor: FighterId,
        target: FighterId,
        port: u16,
        log: i32,
    },
    /// A death the port did not report.
    NoDeath { frame: u32, target: FighterId },
}

fn divergence(round: &Round, timeline: &[BattleEvent]) -> Option<Divergence> {
    let mut events = timeline.iter();
    let mut began = false;
    for event in events.by_ref() {
        if let BattleEvent::RoundBegan {
            round: number,
            order,
        } = event
        {
            assert_eq!(*number, round.round);
            began = true;
            let log: Vec<FighterId> = round.order.iter().copied().map(id).collect();
            if *order != log {
                return Some(Divergence::Queue {
                    round: round.round,
                    port: order.clone(),
                    log,
                });
            }
            break;
        }
    }
    assert!(began, "every round opens with RoundBegan");

    for action in &round.actions {
        let actor = id(action.actor);
        // Only the targets the log resolved are compared: a `$FF` hit flag
        // cannot say whether the swing missed or never reached the slot.
        let resolved: Vec<&Target> = action
            .targets
            .iter()
            .filter(|target| target.hit != "FF")
            .collect();

        let mut swing = None;
        for event in events.by_ref() {
            match event {
                BattleEvent::Attacked {
                    actor: who,
                    targets,
                } if *who == actor => {
                    swing = Some(targets.clone());
                    break;
                }
                BattleEvent::RoundEnded { .. } => break,
                _ => {}
            }
        }
        let Some(targets) = swing else {
            return Some(Divergence::NoSwing {
                frame: action.start_frame,
                actor,
                log_targets: resolved.len(),
            });
        };
        let log: Vec<FighterId> = resolved.iter().map(|target| id(target.id)).collect();
        if targets != log {
            return Some(Divergence::Targets {
                frame: action.start_frame,
                actor,
                port: targets,
                log,
            });
        }

        for target in &resolved {
            let wanted = id(target.id);
            let mut seen = None;
            for event in events.by_ref() {
                match event {
                    BattleEvent::Resolved {
                        actor: who,
                        target: hit,
                        verdict,
                        damage,
                        remaining_hp,
                    } if *who == actor && *hit == wanted => {
                        seen = Some((*verdict, *damage, *remaining_hp));
                        break;
                    }
                    BattleEvent::Attacked { .. } | BattleEvent::RoundEnded { .. } => break,
                    _ => {}
                }
            }
            let Some((verdict, damage, remaining_hp)) = seen else {
                return Some(Divergence::NoResolution {
                    frame: action.start_frame,
                    actor,
                    target: wanted,
                });
            };
            let want_verdict = verdict_of(&target.hit);
            if verdict != want_verdict || damage != target.damage {
                return Some(Divergence::Value {
                    frame: action.start_frame,
                    actor,
                    target: wanted,
                    port: verdict,
                    port_damage: damage,
                    log_hit: u8::from_str_radix(&target.hit, 16).expect("a hex hit flag"),
                    log_damage: target.damage,
                });
            }
            // The cartridge lets stored HP go negative; the port floors at
            // zero, which is what a player sees either way.
            let want_hp = target.hp_after.max(0) as u16;
            if remaining_hp != want_hp {
                return Some(Divergence::Hp {
                    frame: action.start_frame,
                    actor,
                    target: wanted,
                    port: remaining_hp,
                    log: target.hp_after,
                });
            }
        }

        for target in &resolved {
            if !target.died {
                continue;
            }
            let wanted = id(target.id);
            let died = events
                .clone()
                .take_while(|event| {
                    !matches!(
                        event,
                        BattleEvent::Attacked { .. } | BattleEvent::RoundEnded { .. }
                    )
                })
                .any(|event| matches!(event, BattleEvent::Died { fighter } if *fighter == wanted));
            if !died {
                return Some(Divergence::NoDeath {
                    frame: action.start_frame,
                    target: wanted,
                });
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// The replay
// ---------------------------------------------------------------------------

/// Plays one round on a fresh slice of the given stream.
fn play(
    battle: &mut Battle,
    fixture: &Fixture,
    data: &BattleData,
    round: &Round,
    stream: &[u16],
) -> (Vec<BattleEvent>, usize) {
    let mut rolls = SliceRolls::new(stream);
    let timeline = battle
        .round(&orders(fixture, round), data, &mut rolls)
        .expect("the fixture's battle resolves");
    (timeline, rolls.drawn())
}

/// The rolls of one action the port has a consumer for.
fn modelled_rolls(fixture: &Fixture, round: &Round, action: &Action) -> Vec<Roll> {
    fixture
        .rolls
        .rolls()
        .iter()
        .filter(|roll| roll.round == round.round && roll.action == action.actor)
        .filter(|roll| matches!(roll.role.as_str(), "hit" | "ability" | "damage"))
        .cloned()
        .collect()
}

fn priority_roll(fixture: &Fixture) -> Vec<u16> {
    fixture
        .rolls
        .rolls()
        .iter()
        .filter(|roll| roll.role == "priority")
        .map(|roll| roll.roll)
        .collect()
}

/// The battle the fixture describes, started on its own priority roll.
fn started(fixture: &Fixture, data: &BattleData) -> Battle {
    let priority = priority_roll(fixture);
    assert_eq!(priority.len(), 1, "the battle opens on one draw");
    let mut rolls = SliceRolls::new(&priority);
    let battle = start(fixture, data, &mut rolls);
    assert_eq!(rolls.drawn(), 1, "Battle::start draws loc_B62A's roll");
    battle
}

#[test]
fn tape07_orders_its_rounds_the_way_the_cartridge_did() {
    let fixture = fixture();
    let data = fixtures::data();

    // The trace carries 136 calls; the battle's own stream is 134 of them: the
    // encounter's formation draw (f24807) and the post-victory item drop draw
    // (f30306) sit outside every battle routine.
    assert_eq!(fixture.provenance.roll_count, 136);
    assert_eq!(fixture.provenance.battle_roll_count, 134);
    assert_eq!(
        fixture.provenance.roll_column.agrees, 136,
        "every trace row's own roll column is the cartridge's roll, so the \
         fixture checks each row against the raw columns; see \
         docs/BATTLE_ORACLE_REPLAY.md"
    );
    assert_eq!(fixture.provenance.roll_column.subtracts_low_word, 0);
    assert_eq!(fixture.provenance.trace_sha256.len(), 64);

    // The two draws that are not the battle's own: the encounter's formation
    // draw, before the enemies existed in RAM, and the item drop draw after the
    // victory. They are recorded, and they are not replayed.
    let outside = fixture.outside_rolls.rolls();
    assert_eq!(
        outside
            .iter()
            .map(|roll| (roll.role.as_str(), roll.frame))
            .collect::<Vec<(&str, u32)>>(),
        vec![("formation", 24807), ("item_drop", 30306)]
    );

    let priority = priority_roll(&fixture);
    assert_eq!(priority.len(), 1, "the battle opens on one draw");
    let mut start_rolls = SliceRolls::new(&priority);
    let mut battle = start(&fixture, &data, &mut start_rolls);
    assert_eq!(
        start_rolls.drawn(),
        1,
        "Battle::start draws loc_B62A's roll"
    );

    let mut first_round = true;
    for round in &fixture.rounds {
        let stream = verbatim(&fixture, round.round);
        assert_eq!(
            stream.len() as u32,
            round.roll_count,
            "round {}: the fixture's own roll count",
            round.round
        );
        let (timeline, drawn) = play(&mut battle, &fixture, &data, round, &stream);
        let first = divergence(round, &timeline);

        if first_round {
            first_round = false;
            // The queue build is the last thing both sides do before an action
            // resolves, so a queue that matches is every roll of
            // `Battle_OrderTurns` matching: nine jitter draws and one target
            // draw per enemy slot, in the log's own order.
            assert!(
                !matches!(first, Some(Divergence::Queue { .. })),
                "round 1's queue is the log's Battle_Turn_Order at f{}: {first:?}",
                round.order_frame
            );
            // The log's own pair list says the same thing: sorting the
            // fighters by the ordering value it recorded, stable, reproduces
            // the order it recorded them in.
            let mut by_ordering: Vec<(u8, u16)> = round
                .order
                .iter()
                .copied()
                .zip(round.ordering.iter().copied())
                .collect();
            by_ordering.sort_by_key(|entry| std::cmp::Reverse(entry.1));
            assert_eq!(
                by_ordering
                    .iter()
                    .map(|(fighter, _)| *fighter)
                    .collect::<Vec<u8>>(),
                round.order,
                "f{}: Battle_Turn_Order's ids and ordering values agree",
                round.order_frame
            );
            // And this is where the two sides part. Everything the comparator
            // walks before it - the queue, Alys's swing at both enemies (both
            // passes of it), Chaz's swing and its kill, the enemy's ability,
            // hit and damage - is asserted by the comparator having returned
            // this instead of an earlier one. Only `Enemy_Attack`'s ability
            // re-roll is left: the port spends 18 rolls on the enemy's turn
            // where the log's frames hold 19, so its hit roll reads the
            // re-roll's value and the damage window is one early.
            assert_eq!(
                first,
                Some(Divergence::Value {
                    frame: 29789,
                    actor: id(7),
                    target: id(3),
                    port: Verdict::Critical,
                    port_damage: Some(10),
                    log_hit: 0x00,
                    log_damage: Some(6),
                }),
                "the ledger's first divergence now: the enemy ability re-roll, f29789"
            );
            // The counts behind it. Alys's swing's frames hold 36 calls and the
            // port models all of them now - four hit rolls at f29489 (two passes
            // over two enemies) and thirty-two damage draws at f29599.
            let alys = &round.actions[0];
            assert_eq!(alys.actor, 1);
            assert_eq!(alys.roll_count, 36);
            assert_eq!(modelled_rolls(&fixture, round, alys).len(), 36);
            // The enemy's turn is the one that still does not add up:
            // `Enemy_Attack` re-rolls an ability that equals $FFFFEEA8
            // (ps4.asm:19146), which this battle starts at zero, so it burns one
            // call the port does not model.
            let enemy = &round.actions[2];
            assert_eq!(enemy.actor, 7);
            assert_eq!(enemy.roll_count, 19);
            assert_eq!(modelled_rolls(&fixture, round, enemy).len(), 18);
            // On the cartridge's own stream the port reads 85 of the round's
            // 102 calls: thirteen for the order pass, thirty-six for Alys (both
            // passes), seventeen for Chaz, eighteen at the enemy's turn - one
            // short of the log's nineteen - and one for Hahn's swing, which the
            // shifted stream turns into a miss where the log has it landing 5
            // on Enemy2.
            assert_eq!(drawn, 85);

            // The fixture's own windows: each action sits inside the battle and
            // the actions follow each other.
            for pair in round.actions.windows(2) {
                assert!(
                    pair[0].end_frame < pair[1].start_frame,
                    "f{} closes before f{} opens",
                    pair[0].end_frame,
                    pair[1].start_frame
                );
            }
            assert!(round.actions[0].start_frame > round.order_frame);
        }
    }

    // A battle that went differently is still a battle: the engine carries the
    // divergence to an outcome instead of stalling.
    assert!(battle.outcome().is_some(), "the replay finishes");
}

#[test]
fn tape07s_actions_match_once_the_unmodelled_rolls_are_removed() {
    let fixture = fixture();
    let data = fixtures::data();

    let mut battle = started(&fixture, &data);

    for round in &fixture.rounds {
        let stream = modelled(&fixture, round.round);
        let (timeline, drawn) = play(&mut battle, &fixture, &data, round, &stream);

        // The port takes exactly the rolls the roles name: nothing left in the
        // slice and no draw beyond it. That is what makes the port's
        // consumption the modelled counts compared below.
        assert_eq!(
            drawn,
            stream.len(),
            "round {}: the port consumes the modelled rolls exactly",
            round.round
        );
        assert_eq!(
            divergence(round, &timeline),
            None,
            "round {} is the cartridge's, action for action",
            round.round
        );

        // And the counts, action by action: the log's frames against the rolls
        // the port models for the same action. What is left over is exactly
        // the one call the port has no consumer for - `Enemy_Attack` re-rolling
        // an ability equal to $FFFFEEA8 (ps4.asm:19146) - and nothing else.
        // Alys's two hit passes are modelled, so her swing's extra two calls
        // are the port's own now.
        let mut modelled_total = 0;
        for action in &round.actions {
            let modelled = modelled_rolls(&fixture, round, action);
            modelled_total += modelled.len() as u32;
            // The damage runs are labelled with the target the log shows them
            // landing on, so every one of them has to be a slot the action
            // resolved.
            let resolved: Vec<u8> = action
                .targets
                .iter()
                .filter(|target| target.hit != "FF")
                .map(|target| target.id)
                .collect();
            for roll in &modelled {
                if roll.role == "damage" {
                    let target = roll.target.expect("a damage run has a target");
                    assert!(
                        resolved.contains(&target),
                        "actor {} at f{}: a damage run is labelled {} but the \
                         log resolves {resolved:?}",
                        action.actor,
                        roll.frame,
                        target
                    );
                }
            }
            let rolls = fixture.rolls.rolls();
            let unmodelled: Vec<&Roll> = rolls
                .iter()
                .filter(|roll| roll.round == round.round && roll.action == action.actor)
                .filter(|roll| roll.role == "ability_reroll")
                .collect();
            assert_eq!(
                action.roll_count as usize,
                modelled.len() + unmodelled.len(),
                "actor {} at f{}: the log's rolls are the port's plus the \
                 calls it has no consumer for",
                action.actor,
                action.start_frame
            );
            for roll in unmodelled {
                assert_eq!(
                    roll.role, "ability_reroll",
                    "the one call left over is Enemy_Attack's ability re-roll"
                );
                assert_eq!(roll.pass_number, 0);
            }
        }
        assert_eq!(
            modelled_total + 13,
            modelled(&fixture, round.round).len() as u32,
            "round {}: the order pass plus the port's rolls",
            round.round
        );

        if let Some(BattleEvent::Rewarded {
            experience_total,
            experience_each,
            meseta,
            recipients,
        }) = timeline.last().and_then(|last| {
            timeline
                .iter()
                .rev()
                .find(|event| matches!(event, BattleEvent::Rewarded { .. }))
                .filter(|_| matches!(last, BattleEvent::Ended { .. }))
        }) {
            assert_eq!(*experience_total, fixture.outcome.experience_total);
            assert_eq!(*meseta, fixture.outcome.meseta);
            assert_eq!(recipients.len(), fixture.party.len());
            assert_eq!(
                *experience_each,
                fixture.outcome.experience_total / fixture.party.len() as u16,
                "the log's divisor rule: the pool over the living"
            );
        } else {
            assert_ne!(round.round, fixture.rounds.last().unwrap().round);
        }
    }

    assert_eq!(battle.outcome(), Some(Outcome::Victory));
    let outcome = &fixture.outcome;
    assert!(outcome.victory, "the log has every enemy down");
    assert_eq!(
        outcome.dead_enemy_ids,
        fixture
            .formation
            .enemies
            .iter()
            .map(|enemy| enemy.id)
            .collect::<Vec<u8>>(),
        "and the log has each of them at zero HP or below"
    );

    // The rewards the port computed, against the log's own accumulators: 24
    // experience over the three who lived (the log shows Chaz 0 -> 8, Hahn
    // 0 -> 8 and Alys 2457 -> 2465) and 6 meseta (600 -> 606).
    assert_eq!(outcome.experience_total, 24);
    assert_eq!(outcome.meseta, 6);

    let timeline = battle
        .round(&RoundOrders::attack_all(), &data, &mut SliceRolls::new(&[]))
        .expect("a battle that has ended resolves to nothing");
    assert!(timeline.is_empty(), "the battle is over: no third round");
}

#[test]
#[ignore = "the ledger's first divergence: AlysKyraAttack_Init re-runs loc_B6A2 \
            (ps4.asm:13975) and Enemy_Attack re-rolls an ability equal to \
            $FFFFEEA8 (ps4.asm:19146), so the verbatim stream drifts by two \
            rolls at Alys's first swing and one at the enemy's turn and every \
            later window is shifted. See docs/BATTLE_ORACLE_REPLAY.md."]
fn tape07_full_battle_diverges_at_alyss_swing_draw_count() {
    let fixture = fixture();
    let data = fixtures::data();

    let mut battle = started(&fixture, &data);

    // One stream for the whole battle, exactly as the trace holds it, with the
    // two outside draws left out: the port reads the cartridge's rolls and
    // nothing else.
    let stream: Vec<u16> = fixture.rolls.rolls().iter().map(|roll| roll.roll).collect();
    let mut rolls = SliceRolls::new(&stream);
    let mut consumed = 0;
    for round in &fixture.rounds {
        let timeline = battle
            .round(&orders(&fixture, round), &data, &mut rolls)
            .expect("resolves");
        assert_eq!(
            divergence(round, &timeline),
            None,
            "round {} on the cartridge's own stream",
            round.round
        );
        consumed += fixture
            .rolls
            .rolls()
            .iter()
            .filter(|roll| roll.round == round.round)
            .count();
        assert_eq!(
            rolls.drawn(),
            consumed,
            "round {}: the port consumed the cartridge's rolls",
            round.round
        );
    }
    assert_eq!(battle.outcome(), Some(Outcome::Victory));
}
