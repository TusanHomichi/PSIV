//! The comparator: the first place a round's timeline disagrees with the log.
//!
//! Walking the timeline in the order the port resolves it is what makes "the
//! first divergence" well defined: the queue comes first, then each action's
//! swing, its per-target resolutions and its deaths. An enemy action is one of
//! two shapes and the fixture's log says which: a **basic attack** swings
//! (`Attacked`, then one `Resolved` per slot it covered), and an **ability**
//! runs `Enemy_Attack`'s own arm - `EnemySkillUsed` with the id the log's
//! `eN_ability` byte held, then one `Resolved` per living party slot, or
//! `EnemyAbilityWasted` when the arm spends the turn and does nothing. An
//! ability the port does not implement is `UnsupportedAbility` followed by an
//! ordinary physical swing, which is a divergence from a log that shows the
//! ability resolving.

use super::*;

use crate::battle::*;

pub(crate) fn verdict_of(hit: &str) -> Verdict {
    match hit {
        "00" => Verdict::Normal,
        "01" => Verdict::Critical,
        other => panic!("a resolved target with hit flag {other} is not a hit"),
    }
}

/// The first place a round's timeline disagrees with the log.
#[derive(Debug, PartialEq)]
pub(crate) enum Divergence {
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
    /// The log's action ran an ability; the port ran something else.
    Ability {
        frame: u32,
        actor: FighterId,
        log_ability: u8,
        port: Option<u8>,
    },
    /// The log's action ran an ability the port does not implement, so it fell
    /// back to a physical swing.
    Unsupported {
        frame: u32,
        actor: FighterId,
        ability: u8,
    },
    /// The log's action spent the turn without an effect; the port did not.
    NotWasted {
        frame: u32,
        actor: FighterId,
        log_ability: u8,
    },
}

impl Divergence {
    pub(crate) fn frame(&self) -> u32 {
        match self {
            Divergence::Queue { .. } => 0,
            Divergence::NoSwing { frame, .. }
            | Divergence::Targets { frame, .. }
            | Divergence::NoResolution { frame, .. }
            | Divergence::Value { frame, .. }
            | Divergence::Hp { frame, .. }
            | Divergence::NoDeath { frame, .. }
            | Divergence::Ability { frame, .. }
            | Divergence::Unsupported { frame, .. }
            | Divergence::NotWasted { frame, .. } => *frame,
        }
    }

    /// The name `replay_fixtures/divergences.json` carries for this finding.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Divergence::Queue { .. } => "queue",
            Divergence::NoSwing { .. } => "no-swing",
            Divergence::Targets { .. } => "targets",
            Divergence::NoResolution { .. } => "no-resolution",
            Divergence::Value { .. } => "value",
            Divergence::Hp { .. } => "hp",
            Divergence::NoDeath { .. } => "no-death",
            Divergence::Ability { .. } => "ability",
            Divergence::Unsupported { .. } => "unsupported",
            Divergence::NotWasted { .. } => "not-wasted",
        }
    }

    /// What the log says, and what the port did: the pair a reader needs to
    /// judge the finding without re-running anything.
    pub(crate) fn expectation(&self) -> (String, String) {
        match self {
            Divergence::Queue { round, port, log } => (
                format!("round {round}'s queue {log:?}"),
                format!("queue {port:?}"),
            ),
            Divergence::NoSwing {
                actor, log_targets, ..
            } => (
                format!("actor {actor:?} resolving {log_targets} target(s)"),
                "no swing".to_owned(),
            ),
            Divergence::Targets { port, log, .. } => {
                (format!("targets {log:?}"), format!("targets {port:?}"))
            }
            Divergence::NoResolution { actor, target, .. } => (
                format!("{actor:?} resolving {target:?}"),
                "no resolution".to_owned(),
            ),
            Divergence::Value {
                target,
                port,
                port_damage,
                log_hit,
                log_damage,
                ..
            } => (
                format!("{target:?}: hit flag {log_hit:02X}, damage {log_damage:?}"),
                format!("{port:?} with {port_damage:?}"),
            ),
            Divergence::Hp {
                target, port, log, ..
            } => (format!("{target:?} at {log} HP"), format!("{port} HP")),
            Divergence::NoDeath { target, .. } => {
                (format!("{target:?} dying"), "no Died event".to_owned())
            }
            Divergence::Ability {
                log_ability, port, ..
            } => (
                format!("ability ${log_ability:02X}"),
                match port {
                    Some(ability) => format!("ability ${ability:02X}"),
                    None => "a physical swing".to_owned(),
                },
            ),
            Divergence::Unsupported { ability, .. } => (
                format!("ability ${ability:02X} resolving"),
                format!("UnsupportedAbility ${ability:02X} and a physical swing"),
            ),
            Divergence::NotWasted { log_ability, .. } => (
                format!("ability ${log_ability:02X} spending the turn"),
                "an effect".to_owned(),
            ),
        }
    }
}

/// What the port did with one enemy turn, read off its timeline.
#[derive(Debug, PartialEq)]
enum EnemyTurn {
    /// An ability, with the id the port resolved.
    Ability(u8),
    /// An ability that spent the turn: `EnemyAbilityWasted`.
    Wasted(u8),
    /// An ability the port does not implement; the enemy swings physically
    /// instead (`UnsupportedAbility`).
    Unsupported(u8),
    /// A physical swing.
    Attack,
    /// The round ended before the actor's turn came up.
    Nothing,
}

/// Consumes the timeline up to the actor's turn and reports what it was.
///
/// Everything the actor's own arm emits before the swing is part of the same
/// turn, so the walk stops at the first of those events - or at the round's
/// end, which means the port never gave the actor a turn at all.
fn enemy_turn<'a, I>(events: &mut I, actor: FighterId) -> EnemyTurn
where
    I: Iterator<Item = &'a BattleEvent>,
{
    for event in events.by_ref() {
        match event {
            BattleEvent::EnemySkillUsed {
                actor: who, skill, ..
            } if *who == actor => return EnemyTurn::Ability(*skill),
            BattleEvent::EnemyAbilityWasted {
                actor: who,
                ability,
                ..
            } if *who == actor => return EnemyTurn::Wasted(*ability),
            BattleEvent::UnsupportedAbility {
                actor: who,
                ability,
            } if *who == actor => return EnemyTurn::Unsupported(*ability),
            BattleEvent::Attacked { actor: who, .. } if *who == actor => {
                return EnemyTurn::Attack;
            }
            BattleEvent::RoundEnded { .. } => return EnemyTurn::Nothing,
            _ => {}
        }
    }
    EnemyTurn::Nothing
}

/// Whether this action's `hit` byte was sampled before the swing's last
/// `loc_B6A2` pass.
///
/// `Fighters_Hit_Flags` is read once a frame, and the fixture's per-target
/// `hit` is the byte at the action's **hit frame** — the first frame the flags
/// moved (`oracle/fixture/observations.py`'s `action_record`). A swing whose
/// passes all arrive in that frame leaves the swing's own last write there:
/// every character's single pass, and the two Alys's and Kyra's animation runs
/// in one frame (`CharAttack_AlysKyra`, `ps4.asm:13958`). A swing whose passes
/// arrive in *different* frames leaves only its first pass's verdict: the
/// vehicle's three-pass command-6 swing is the case, and rounds 3 and 6 of
/// `forced_53_desrtleach` are exactly that — a `$01` first pass over a 154 and
/// a 189 that only a normal hit's arithmetic produces (`(45+8)*200)>>6 + 200 =
/// 365`, `365*2>>2 - 28 = 154`; with the `atk >> 2` bonus it would be 204).
fn flag_lags_the_swing(rolls: &[Roll], round: u16, actor: FighterId) -> bool {
    let frames: Vec<u32> = rolls
        .iter()
        .filter(|roll| roll.round == round && roll.action == actor.get() && roll.role == "hit")
        .map(|roll| roll.frame)
        .collect();
    frames
        .first()
        .is_some_and(|first| frames.iter().any(|frame| frame != first))
}

pub(crate) fn divergence(
    round: &Round,
    timeline: &[BattleEvent],
    rolls: &[Roll],
) -> Option<Divergence> {
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
        // Whether the log's `hit` byte for this action predates the swing's
        // last pass: see [`flag_lags_the_swing`].
        let lagged = flag_lags_the_swing(rolls, round.round, actor);
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

        let swung: Vec<FighterId> = if action.kind == Kind::Attack {
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
            // list: same slots, same order. A slot the log left at `$FF`
            // because the port missed it is not part of that list - but a slot
            // the log *hit* that the port missed, or an extra slot the port
            // hit, shows up here.
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
            targets
        } else {
            // The log's action ran the ability roll, so the port's own arm has
            // to be what answers it. `Wasted` is the log's reading of an
            // ability that left nothing behind.
            let port = enemy_turn(&mut events, actor);
            match (&action.kind, &port) {
                (_, EnemyTurn::Unsupported(ability)) => {
                    return Some(Divergence::Unsupported {
                        frame: action.start_frame,
                        actor,
                        ability: *ability,
                    });
                }
                (Kind::Ability, EnemyTurn::Ability(skill)) if Some(*skill) == action.ability => {}
                (Kind::Wasted, EnemyTurn::Wasted(ability)) if Some(*ability) == action.ability => {}
                (Kind::Wasted, _) => {
                    // The log's ability spent the turn: the port giving it an
                    // effect is a divergence, and a different one from the
                    // port never reaching the arm at all.
                    return Some(Divergence::NotWasted {
                        frame: action.start_frame,
                        actor,
                        log_ability: action.ability.expect("the extractor records the id"),
                    });
                }
                (_, _) => {
                    return Some(Divergence::Ability {
                        frame: action.start_frame,
                        actor,
                        log_ability: action.ability.expect("the extractor records the id"),
                        port: match port {
                            EnemyTurn::Ability(skill) | EnemyTurn::Wasted(skill) => Some(skill),
                            EnemyTurn::Unsupported(skill) => Some(skill),
                            EnemyTurn::Attack | EnemyTurn::Nothing => None,
                        },
                    });
                }
            }
            // The slots the ability resolved on: the port's own list, checked
            // below slot by slot against the log's.
            resolved.iter().map(|target| id(target.id)).collect()
        };

        // The port's resolutions, in the order it made them: one per slot it
        // swung at, misses included. A slot the log left at `$FF` - or never
        // listed at all, because neither its flag nor its damage word moved -
        // has to be one of those misses; a hit there would have written both.
        for swung in &swung {
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
                    // A flag that lags the swing's last pass names an earlier
                    // pass, so it is only a claim that the swing reached this
                    // slot; the verdict the port reports is the one the damage
                    // carries, and the damage is what is compared here. A flag
                    // that is the swing's own last write is compared as the
                    // verdict itself.
                    if damage != target.damage || (!lagged && verdict != want_verdict) {
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
                // The death is checked here, where the timeline still holds it:
                // an ability resolves every living slot in one turn, so a
                // death that came before the last of them is behind the walk
                // once the slot's own resolution is read. `Died` follows its
                // own `Resolved`, which is what the take_while stops at.
                if target.died {
                    let died = events
                        .clone()
                        .take_while(|event| {
                            !matches!(
                                event,
                                BattleEvent::Attacked { .. } | BattleEvent::RoundEnded { .. }
                            )
                        })
                        .any(|event| {
                            matches!(event, BattleEvent::Died { fighter } if *fighter == wanted)
                        });
                    if !died {
                        return Some(Divergence::NoDeath {
                            frame: action.start_frame,
                            target: wanted,
                        });
                    }
                }
            }
        }
    }
    None
}
