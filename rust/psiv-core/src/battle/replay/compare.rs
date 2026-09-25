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

#[cfg(test)]
#[path = "compare_tests.rs"]
mod tests;

pub(crate) fn verdict_of(hit: &str) -> Verdict {
    match hit {
        "00" => Verdict::Normal,
        "01" => Verdict::Critical,
        other => panic!("a resolved target with hit flag {other} is not a hit"),
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

pub(crate) fn divergence(round: &Round, timeline: &[BattleEvent]) -> Option<Divergence> {
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
        // the verdict the port drew, rather than guessed at from the byte. Each
        // byte is the one the action's **last** pass wrote - `loc_B6A2` presets
        // all nine before every pass (`ps4.asm:17493-17498`), so an earlier
        // pass's verdicts are overwritten - which is the frame
        // `oracle/fixture/observations.py`'s `pass_frame` picks and what makes
        // the verdict comparison below a strict one.
        let entry =
            |wanted: FighterId| action.targets.iter().find(|target| id(target.id) == wanted);
        let resolved: Vec<&Target> = action
            .targets
            .iter()
            .filter(|target| target.hit != "FF")
            .collect();
        // The slots the log shows a *damage* on: the ability branch's own
        // list, because a resolved slot and a damaged slot are not the same
        // claim (an ability's pass covers its whole range).
        let damaged: Vec<&Target> = action
            .targets
            .iter()
            .filter(|target| target.damage.is_some())
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
            // ability that left nothing behind - and `oracle/fixture/
            // enemies.py` says why that reading cannot be a strict one: a turn
            // whose arm ran and found nothing to do (a status the target
            // already carries) is the same log as a turn whose arm does not
            // exist, so the port may answer either way. What it may not do is
            // run a *different* ability, or none at all.
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
                (Kind::Wasted, EnemyTurn::Ability(skill)) if Some(*skill) == action.ability => {
                    // The log cannot tell this turn from one the arm spent:
                    // both draw the ability roll and nothing else, and neither
                    // leaves a cell behind (`oracle/fixture/enemies.py`). What
                    // it *can* say is that nothing was resolved - so the arm is
                    // accepted only if nothing was.
                    let resolved = events
                        .clone()
                        .take_while(|event| {
                            !matches!(
                                event,
                                BattleEvent::Attacked { .. } | BattleEvent::RoundEnded { .. }
                            )
                        })
                        .any(|event| {
                            matches!(event, BattleEvent::Resolved { actor: who, .. }
                                     if *who == actor)
                        });
                    if resolved {
                        return Some(Divergence::NotWasted {
                            frame: action.start_frame,
                            actor,
                            log_ability: action.ability.expect("the extractor records the id"),
                        });
                    }
                }
                (Kind::Ability, EnemyTurn::Wasted(_)) => {
                    // The log's ability resolved something - a slot, a damage
                    // word, a status or a stat cell - and the port spent the
                    // turn instead: the arm it took differs.
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
            // The status bytes the action moved: the effect the log sees that
            // the port reports as an event of its own, checked before the
            // slot-by-slot walk below - which only reads the slots the log
            // resolved, and an ability's status arm resolves a slot with no
            // damage word at all.
            for (target, _before, after) in &action.effect.status {
                let target = id(*target);
                let inflicted = events
                    .clone()
                    .take_while(|event| {
                        !matches!(
                            event,
                            BattleEvent::Attacked { .. } | BattleEvent::RoundEnded { .. }
                        )
                    })
                    .any(|event| {
                        matches!(event, BattleEvent::StatusInflicted { target: hit, .. }
                                 if *hit == target)
                    });
                if !inflicted {
                    return Some(Divergence::Status {
                        frame: action.start_frame,
                        actor,
                        target,
                        log_status: *after,
                    });
                }
            }
            // The slots the ability **damaged**: the log's own list, walked
            // slot by slot below. Not the pass's coverage - an ability marks
            // every slot in its range as resolved whether or not its effect
            // reaches them (a status arm that misses, or finds the status
            // already set, writes no damage and no status), so the coverage is
            // evidence of a range and not of a resolution. What the walk can
            // check is that each damaged slot was resolved; every *other* slot
            // the port resolves is checked by the scan that follows it, which
            // is what keeps a port damage on a slot the log left clean from
            // passing unread.
            damaged.iter().map(|target| id(target.id)).collect()
        };

        // This actor's whole turn, captured before the walk below: the walk
        // steps over events that belong to slots it is not looking for, and an
        // ability's own check has to see them all. Empty for a swing, whose
        // target list *is* the walk.
        let turn: Vec<&BattleEvent> = if action.kind == Kind::Attack {
            Vec::new()
        } else {
            events
                .clone()
                .take_while(|event| {
                    !matches!(
                        event,
                        BattleEvent::Attacked { .. } | BattleEvent::RoundEnded { .. }
                    )
                })
                .collect()
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
                    // The verdict is always compared; the damage word only
                    // where the log shows one moving. A word rewritten to the
                    // value it already held is invisible
                    // (`oracle/fixture/assembly.py`'s undetermined list), and
                    // the HP left on the slot below is what pins the number
                    // either way.
                    let matches_damage = target.damage.is_none_or(|d| damage == Some(d));
                    if verdict != want_verdict || !matches_damage {
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

        // The ability branch's second half. The walk above read only the slots
        // the log shows damage on - an ability's pass marks every slot in its
        // range as resolved whether or not its effect reaches them, so the
        // coverage cannot be what a port resolution is demanded for - and that
        // leaves the other direction unread: a port resolution on a slot the
        // log left clean. A bare miss there is still accepted (the range's own
        // slots, reached and missed), and anything else is not: a port that
        // deals damage, or lands a non-miss, on a slot whose log shows neither
        // has diverged, and this is the only place it can be seen - the walk
        // steps over events for slots it is not looking for, and nothing else
        // compares a slot's HP at the round's end.
        if action.kind != Kind::Attack {
            for event in &turn {
                let BattleEvent::Resolved {
                    actor: who,
                    target: hit,
                    verdict,
                    damage,
                    ..
                } = event
                else {
                    continue;
                };
                if *who != actor || (*verdict == Verdict::Miss && damage.is_none()) {
                    continue;
                }
                if damaged.iter().any(|target| id(target.id) == *hit) {
                    continue;
                }
                let logged = entry(*hit);
                return Some(Divergence::Value {
                    frame: action.start_frame,
                    actor,
                    target: *hit,
                    port: *verdict,
                    port_damage: *damage,
                    log_hit: logged.map_or(0xFF, |target| {
                        u8::from_str_radix(&target.hit, 16).expect("a hex hit flag")
                    }),
                    log_damage: logged.and_then(|target| target.damage),
                });
            }
        }
    }
    None
}
