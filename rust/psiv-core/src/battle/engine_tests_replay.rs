//! Replaying an oracle tape's battle against the cartridge's own rolls.
//!
//! `oracle/battle_fixture.py` turns one oracle run's RNG trace and RAM log into
//! a fixture under `replay_fixtures/`: the battle's start state, the rolls the
//! cartridge drew (each with the frame it was drawn in and the role it played)
//! and what the log shows every action doing. This module is the harness the
//! tapes' replays share: it builds the battle a fixture describes, feeds those
//! rolls through [`SliceRolls`], and compares the port's timeline with the
//! cartridge's, action by action. One child module per tape:
//!
//! * `tape07` - the first basement battle of `oracle/tapes/07_first_battle.tape`.
//! * `tape09` - the second encounter, on its own seed path, with a critical
//!   (`oracle/tapes/09_second_battle.tape`).
//!
//! A roll is `hv + frame_count - high_word(RNG_Seed)`: `UpdateRNGSeed2`
//! (`ps4.asm:86097`) subtracts the word at `$FFFFEF0C`, which on a big-endian
//! 68000 is the seed longword's *high* half. The fixture derives each roll from
//! the trace's raw columns for that reason; `docs/BATTLE_ORACLE_REPLAY.md` has
//! the proof, the trace's own `roll` column being the low-half subtraction.
//!
//! # One stream, and what it costs
//!
//! The cartridge's stream is replayed verbatim: every call the battle's frames
//! hold, in the order it drew them. The two draw-count divergences this port
//! used to carry are closed, so no roll has to be filtered out any more.
//! `AlysKyraAttack_Init` re-runs `loc_B6A2` (`ps4.asm:13975-13976`) and
//! `resolve_attack` draws the second pass for the two attackers the cartridge
//! sends there; `Enemy_Attack`'s ability re-roll against `$FFFFEEA8`
//! (`ps4.asm:19146-19151`) is modelled, so a battle begins with the word at
//! zero (the `GameMode_LoadBattle` wipe of the page it lives in,
//! `ps4.asm:9992-9994`) and pays a second call for a first draw of zero.
//! Two draws stay outside the battle's own state machine and out of its stream:
//! the encounter's formation draw and the post-victory item drop, which the
//! fixture keeps in `outside_rolls`.
//!
//! What the harness asserts, per tape:
//!
//! * [`replay_verbatim`] - the port resolves what the log resolved, action for
//!   action, and consumes the cartridge's rolls and no more;
//! * [`account_for_every_roll`] - every call the log's frames hold is one the
//!   port's own consumers account for, down to the ability re-roll.
use super::*;

use crate::battle::FormationEnemy;
use serde::Deserialize;

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

/// The tape's fixture, parsed and checked for the version this harness reads.
fn fixture(json: &str) -> Fixture {
    let parsed: Fixture =
        serde_json::from_str(json).expect("the fixture is the extractor's output");
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
// The stream
// ---------------------------------------------------------------------------

/// The cartridge's rolls for a round, in the order it drew them.
fn verbatim_round(fixture: &Fixture, round: u16) -> Vec<u16> {
    fixture
        .rolls
        .rolls()
        .iter()
        .filter(|roll| roll.round == round)
        .map(|roll| roll.roll)
        .collect()
}

/// The battle's whole stream, in the order the cartridge drew it - what a
/// replay is fed once its rounds start.
///
/// The priority draw is [`started`]'s, not a round's: `Battle::start` takes it
/// as part of setting the battle up, exactly as the cartridge resolves
/// `loc_B62A` while loading the battle rather than inside a round.
fn verbatim_all(fixture: &Fixture) -> Vec<u16> {
    fixture
        .rolls
        .rolls()
        .iter()
        .filter(|roll| roll.role != "priority")
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
        // The log's reading of this action, slot by slot. `$FF` in
        // `Fighters_Hit_Flags` means "this slot was not resolved as a hit" and
        // nothing more: `loc_B6A2` blanks all nine flags before every pass, so
        // a swing that reaches a slot and misses leaves the same `$FF` a slot
        // the swing never touched does. The slots the log did resolve are the
        // ones it reports a flag for; the `$FF` ones are checked below against
        // the verdict the port drew, rather than guessed at from the byte.
        let entry =
            |wanted: FighterId| action.targets.iter().find(|target| id(target.id) == wanted);
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
        // The port's swing, minus the slots it missed, is the log's target
        // list: same slots, same order. A slot the log left at `$FF` because
        // the port missed it is not part of that list - but a slot the log
        // *hit* that the port missed, or an extra slot the port hit, shows up
        // here.
        let hits: Vec<FighterId> = targets
            .iter()
            .copied()
            .filter(|target| entry(*target).is_some_and(|entry| entry.hit != "FF"))
            .collect();
        let log: Vec<FighterId> = resolved.iter().map(|target| id(target.id)).collect();
        if hits != log {
            return Some(Divergence::Targets {
                frame: action.start_frame,
                actor,
                port: targets,
                log,
            });
        }

        // The port's resolutions, in the order it made them: one per slot it
        // swung at, misses included. A slot the log left at `$FF` - or never
        // listed at all, because neither its flag nor its damage word moved -
        // has to be one of those misses; a hit there would have written both.
        for swung in &targets {
            let wanted = *swung;
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
            let logged = entry(wanted);
            match logged.map(|entry| entry.hit.as_str()) {
                Some(flag) if flag != "FF" => {
                    let target = logged.expect("just matched");
                    let want_verdict = verdict_of(flag);
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
                }
                _ => {
                    // The log resolved nothing here, so the port must have
                    // missed: a `Normal`/`Critical` verdict with damage is a
                    // divergence even though the log has no flag to compare.
                    if verdict != Verdict::Miss || damage.is_some() {
                        return Some(Divergence::Value {
                            frame: action.start_frame,
                            actor,
                            target: wanted,
                            port: verdict,
                            port_damage: damage,
                            log_hit: 0xFF,
                            log_damage: None,
                        });
                    }
                }
            }
            // The cartridge lets stored HP go negative; the port floors at
            // zero, which is what a player sees either way.
            if let Some(target) = logged {
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

/// Every roll the log's frames attribute to one action.
///
/// The roles are the ones an action's own frames hold: the hit pass (both
/// passes of it, `AlysKyraAttack_Init`'s included), the ability roll, the
/// ability **re-roll** that follows a draw equal to `$FFFFEEA8`, and the
/// damage runs. The order pass is the round's, not an action's, and comes from
/// the fixture's own `roll_count` for the round.
fn action_rolls(fixture: &Fixture, round: &Round, action: &Action) -> Vec<Roll> {
    fixture
        .rolls
        .rolls()
        .iter()
        .filter(|roll| roll.round == round.round && roll.action == action.actor)
        .filter(|roll| {
            matches!(
                roll.role.as_str(),
                "hit" | "ability" | "ability_reroll" | "damage"
            )
        })
        .cloned()
        .collect()
}

/// Replays every round of `fixture` on the cartridge's own stream.
///
/// Per round it asserts both halves of the claim: the timeline is the log's,
/// action for action, and the port drew exactly the rolls the log's frames
/// hold - no slack and no surplus. Returns the finished battle and each round's
/// timeline.
fn replay_verbatim(
    fixture: &Fixture,
    data: &BattleData,
    tape: &str,
) -> (Battle, Vec<Vec<BattleEvent>>) {
    let mut battle = started(fixture, data);
    let stream = verbatim_all(fixture);
    let mut rolls = SliceRolls::new(&stream);
    let mut timelines = Vec::new();
    let mut consumed = 0;
    for round in &fixture.rounds {
        let timeline = battle
            .round(&orders(fixture, round), data, &mut rolls)
            .expect("the fixture's battle resolves");
        assert_eq!(
            divergence(round, &timeline),
            None,
            "{tape}: round {} is the cartridge's, action for action",
            round.round
        );
        consumed += round.roll_count as usize;
        assert_eq!(
            rolls.drawn(),
            consumed,
            "{tape}: round {} consumes the cartridge's rolls, and no more",
            round.round
        );
        timelines.push(timeline);
    }
    (battle, timelines)
}

/// Every call the log's frames hold is one the port accounts for.
///
/// The count is per action, against the roles that action's frames carry: the
/// round's own order pass is the difference between the round's `roll_count`
/// and the sum over its actions. A roll with no consumer - the divergence this
/// test exists to catch - shows up as an action whose log count is larger than
/// the port's, and a consumer with no roll in the log shows up in the other
/// direction.
fn account_for_every_roll(fixture: &Fixture, tape: &str) {
    let rolls = fixture.rolls.rolls();
    for round in &fixture.rounds {
        let order = rolls
            .iter()
            .filter(|roll| roll.round == round.round && roll.role == "order")
            .count();
        let mut actions = 0;
        for action in &round.actions {
            let port = action_rolls(fixture, round, action);
            assert_eq!(
                action.roll_count as usize,
                port.len(),
                "{tape}: actor {} at f{} draws the log's {} calls",
                action.actor,
                action.start_frame,
                action.roll_count
            );
            for roll in &port {
                if roll.role == "damage" {
                    let target = roll.target.expect("a damage run has a target");
                    let resolved: Vec<u8> = action
                        .targets
                        .iter()
                        .filter(|target| target.hit != "FF")
                        .map(|target| target.id)
                        .collect();
                    assert!(
                        resolved.contains(&target),
                        "{tape}: actor {} at f{}: a damage run is labelled {} but the \
                         log resolves {resolved:?}",
                        action.actor,
                        roll.frame,
                        target
                    );
                }
            }
            actions += port.len();
        }
        assert_eq!(
            round.roll_count as usize,
            actions + order,
            "{tape}: round {}'s frames hold the order pass and its actions, nothing else",
            round.round
        );
    }
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

// ---------------------------------------------------------------------------
// The tapes
// ---------------------------------------------------------------------------

#[path = "engine_tests_replay_tape07.rs"]
mod tape07;

#[path = "engine_tests_replay_tape09.rs"]
mod tape09;
