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
            BattleEvent::FirstZioAction {
                actor: who,
                action,
                target,
            } if *who == actor => {
                let ability = match action {
                    FirstZioAction::MagicBarrier => 0x6B,
                    FirstZioAction::Nightmare => 0x53,
                    // Zio3 exits with no target; Zio's kill arm names one
                    // (zio.rs, ps4.asm:19455-19461, 19502-19516).
                    FirstZioAction::BlackWave => {
                        if target.is_some() {
                            0x54
                        } else {
                            0x70
                        }
                    }
                    FirstZioAction::Invocation
                    | FirstZioAction::Pause
                    | FirstZioAction::DarkForceCharge
                    | FirstZioAction::DarkForceReveal => 0,
                };
                return EnemyTurn::Ability(ability);
            }
            BattleEvent::RoundEnded { .. } => return EnemyTurn::Nothing,
            _ => {}
        }
    }
    EnemyTurn::Nothing
}

/// The transient status bit 7 a freshly seated fighter carries (`$80`).
const SEATED: u32 = 0x80;

/// Whether `event` opens another actor's turn, which is where the previous
/// actor's events end.
fn opens_turn(event: &BattleEvent) -> bool {
    matches!(
        event,
        BattleEvent::Attacked { .. }
            | BattleEvent::EnemySkillUsed { .. }
            | BattleEvent::EnemyAbilityWasted { .. }
            | BattleEvent::UnsupportedAbility { .. }
            | BattleEvent::TurnSkipped { .. }
            | BattleEvent::Defended { .. }
            | BattleEvent::TechniqueUsed { .. }
            | BattleEvent::SkillUsed { .. }
            | BattleEvent::ItemUsed { .. }
            | BattleEvent::FirstZioAction { .. }
            | BattleEvent::RoundEnded { .. }
    )
}

/// The events one enemy ability emitted, from the cursor to the next turn.
fn turn_of<'a, I>(events: &I) -> Vec<&'a BattleEvent>
where
    I: Iterator<Item = &'a BattleEvent> + Clone,
{
    events
        .clone()
        .take_while(|event| !opens_turn(event))
        .collect()
}

/// A selected party command must execute that command, never an attack fallback.
fn party_turn<'a, I>(events: &mut I, actor: FighterId, action: &Action) -> bool
where
    I: Iterator<Item = &'a BattleEvent>,
{
    for event in events.by_ref() {
        match event {
            BattleEvent::Defended { actor: who } if *who == actor => {
                return action.kind == Kind::Defend;
            }
            BattleEvent::TechniqueUsed {
                actor: who,
                technique,
                ..
            } if *who == actor => {
                return action.kind == Kind::Technique && action.ability == Some(*technique);
            }
            BattleEvent::SkillUsed {
                actor: who, skill, ..
            } if *who == actor => {
                return action.kind == Kind::Skill && action.ability == Some(*skill);
            }
            BattleEvent::ItemUsed {
                actor: who, item, ..
            } if *who == actor => {
                return action.kind == Kind::Item && action.ability == Some(*item);
            }
            // The cartridge can reject a command after selection when an
            // earlier action changes status. Its observable effect stays checked.
            BattleEvent::TurnSkipped { actor: who, .. } if *who == actor => {
                return action.effect.status.is_empty()
                    && action.effect.hp.is_empty()
                    && action.effect.stats.is_empty();
            }
            BattleEvent::RoundEnded { .. }
            | BattleEvent::Attacked { .. }
            | BattleEvent::TechniqueRejected { .. }
            | BattleEvent::SkillRejected { .. }
            | BattleEvent::ItemRejected { .. } => return false,
            _ => {}
        }
    }
    false
}

/// The status and agility comparison for an ability action: the first
/// fighter whose logged gain differs from what the port's turn did.
fn effect_divergence(
    action: &Action,
    actor: FighterId,
    turn: &[&BattleEvent],
) -> Option<Divergence> {
    let inflicted = |target: FighterId| -> u32 {
        turn.iter()
            .filter_map(|event| match event {
                BattleEvent::StatusInflicted {
                    target: hit,
                    status,
                    ..
                } if *hit == target => Some(u32::from(*status)),
                BattleEvent::FellAsleep { target: hit, .. } if *hit == target => {
                    Some(u32::from(status::ASLEEP))
                }
                _ => None,
            })
            .fold(0, |bits, status| bits | status)
    };
    let mut targets: Vec<FighterId> = action
        .effect
        .status
        .iter()
        .map(|(target, _, _)| id(*target))
        .collect();
    for event in turn {
        if let BattleEvent::StatusInflicted { target, .. } | BattleEvent::FellAsleep { target, .. } =
            event
            && !targets.contains(target)
        {
            targets.push(*target);
        }
    }
    for target in targets {
        // Bit 7 is not a status: `loc_14D46` (`ps4.asm:29735`) and `loc_14CBE`
        // set it on a fighter seated mid-battle so that the queue's `$EE` test
        // (`loc_5772`) skips it this round. The port keeps that out of `Stats`
        // and answers it with the turn skip instead (`take_turn`), which the
        // round's draw count checks.
        let gained = action
            .effect
            .status
            .iter()
            .filter(|(who, _, _)| id(*who) == target)
            .fold(0, |bits, (_, before, after)| {
                bits | (after & !before & !SEATED)
            });
        let port = inflicted(target);
        if gained != port {
            return Some(Divergence::Status {
                frame: action.start_frame,
                actor,
                target,
                log_status: gained,
                port_status: port,
            });
        }
    }
    // Deaths. A fighter the action took from positive HP to zero is one the
    // port's turn reports as `Died`, and no other is. Only for an action that
    // dealt no damage: a damage ability's HP drops across frames after the
    // action's window (an all-party hit shows no death at its end frame), and
    // the per-slot walk below already checks each damaged slot's HP and death.
    let damaging = action.targets.iter().any(|target| target.damage.is_some());
    let mut log_dead: Vec<FighterId> = action
        .effect
        .hp
        .iter()
        .filter(|(_, before, after)| *before > 0 && *after <= 0)
        .map(|(who, _, _)| id(*who))
        .collect();
    let mut port_dead: Vec<FighterId> = turn
        .iter()
        .filter_map(|event| match event {
            BattleEvent::Died { fighter } => Some(*fighter),
            _ => None,
        })
        .collect();
    log_dead.sort();
    port_dead.sort();
    if !damaging && log_dead != port_dead {
        return Some(Divergence::Deaths {
            frame: action.start_frame,
            actor,
            log: log_dead,
            port: port_dead,
        });
    }
    for (who, field, _before, after) in &action.effect.stats {
        // Psycho Wand's loc_3CF60/loc_7F22 reloads records, rather than
        // changing one stat. Check each observed cell against the same decoded
        // record the engine reloads; end-of-round state also pins the new form.
        // Fusion's and COMBINE's reload (`loc_14D46`) fills the seated
        // fighter's cells from its record the same way.
        let reloaded = turn.iter().find_map(|event| match event {
            BattleEvent::EnemyStatsReloaded {
                fighter, enemy_id, ..
            }
            | BattleEvent::EnemiesFused {
                fighter, enemy_id, ..
            } if *fighter == id(*who) => Some(*enemy_id),
            _ => None,
        });
        if let Some(enemy) = reloaded {
            let data = super::pack::data();
            let stats = Stats::from_enemy(data.enemy(enemy).expect("reloaded pack record"));
            let value = match field.as_str() {
                "agi_bat" => u16::from(stats.agility.battle),
                "atk" | "atk_bat" => stats.attack.battle,
                "dfs" | "dfs_bat" => stats.defence.battle,
                "men_bat" => u16::from(stats.mental.battle),
                "dex_bat" => u16::from(stats.dexterity.battle),
                "mdfs_bat" => stats.mental_defence.battle,
                _ => continue,
            };
            if i32::from(value) != *after {
                return Some(Divergence::Stat {
                    frame: action.start_frame,
                    actor,
                    target: id(*who),
                    field: field.clone(),
                    log_value: *after,
                    port_value: Some(value),
                });
            }
            continue;
        }
        // The cells a port event reports: battle agility (`agi_bat`, both
        // sides' fixtures) and the party's battle attack and defence
        // (`atk_bat`, `dfs_bat`, which `oracle/sweep/arc.py` adds because the
        // extractor reads the derived cells instead).
        let stat = match field.as_str() {
            "agi_bat" => TechniqueStat::Agility,
            "atk_bat" => TechniqueStat::Attack,
            "dfs_bat" => TechniqueStat::Defence,
            "mdfs_bat" => TechniqueStat::MentalDefence,
            _ => continue,
        };
        let target = id(*who);
        let set = turn.iter().rev().find_map(|event| match event {
            BattleEvent::StatChanged {
                target: hit,
                stat: changed,
                value,
                ..
            } if *hit == target && *changed == stat => Some(*value),
            _ => None,
        });
        // `AbilityEffect_Paralyze` sets `agility_battle` to 1 beside the
        // status bit (`ps4.asm:9431-9434`), and a fighter seated mid-battle
        // (Fusion) starts from its record.
        let implied = if stat == TechniqueStat::Agility {
            let paralysed = inflicted(target) & u32::from(status::PARALYZED) != 0;
            let seated = turn.iter().rev().find_map(|event| match event {
                BattleEvent::EnemiesFused {
                    fighter, agility, ..
                } if *fighter == target => Some(u16::from(*agility)),
                _ => None,
            });
            seated.or(paralysed.then_some(1))
        } else {
            None
        };
        let port_value = set.or(implied);
        if port_value.map(i32::from) != Some(*after) {
            return Some(Divergence::Stat {
                frame: action.start_frame,
                actor,
                target,
                field: field.clone(),
                log_value: *after,
                port_value,
            });
        }
    }
    None
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
                    // A queued fighter that cannot act - asleep, paralyzed -
                    // swings at nothing. The log's window for it is not a swing
                    // at all: `loc_576A` leaves the skipped actor's id in the
                    // actor field (`ps4.asm:8033`), so the calls that follow -
                    // the round's tail, one wake roll per sleeper
                    // (`Battle_RestoreStatsAtTurnEnd`, `ps4.asm:9792`) - open a
                    // window on it. The walk accepts it only when the log
                    // resolved nothing there, and the round's draw count checks
                    // that the port made the same calls.
                    BattleEvent::TurnSkipped { actor: who, .. } if *who == actor => {
                        swing = Some(Vec::new());
                        break;
                    }
                    // A scripted first action (Dark Force 1's latch turn) clears
                    // the ability byte, so the log files it as an attack that
                    // resolved no slot: the port's answer is the scripted event.
                    BattleEvent::FirstZioAction { actor: who, .. } if *who == actor => {
                        swing = Some(Vec::new());
                        break;
                    }
                    // COMBINE's arms clear `$24(a4)` before their object
                    // reloads the side (`ps4.asm:21550`, `21621`), so the log
                    // files the turn the same way: an attack that resolved no
                    // slot. Fusion keeps its id and is an ability turn.
                    BattleEvent::EnemiesFused { actor: who, .. } if *who == actor => {
                        swing = Some(Vec::new());
                        break;
                    }
                    // `EnemyAttack_FloatMine`'s fall-through (`loc_10406`,
                    // `ps4.asm:22781`) writes the rolled id and clears it again
                    // inside one frame, so the log's byte reads zero and the
                    // action is filed as an attack that resolved no slot. What
                    // tells the two apart is the draw count, which the round's
                    // own check pins: a swing draws a hit roll, the spent turn
                    // draws the ability roll and nothing after it.
                    BattleEvent::EnemyAbilityWasted { actor: who, .. } if *who == actor => {
                        swing = Some(Vec::new());
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
        } else if action.actor <= LAST_PARTY_ID {
            if !party_turn(&mut events, actor, action) {
                return Some(Divergence::Ability {
                    frame: action.start_frame,
                    actor,
                    log_ability: action.ability.unwrap_or(0),
                    port: None,
                });
            }
            if let Some(found) = effect_divergence(action, actor, &turn_of(&events)) {
                return Some(found);
            }
            damaged.iter().map(|target| id(target.id)).collect()
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
                (Kind::Wasted, EnemyTurn::Wasted(0x07 | 0x17))
                    if action.ability_cleared && action.ability == Some(0) => {}
                (Kind::Wasted, EnemyTurn::Ability(skill)) if Some(*skill) == action.ability => {
                    // The log cannot tell this turn from one the arm spent:
                    // both draw the ability roll and nothing else, and neither
                    // leaves a cell behind (`oracle/fixture/enemies.py`). What
                    // it *can* say is that nothing was resolved - so the arm is
                    // accepted only if nothing was.
                    let resolved =
                        events
                            .clone()
                            .take_while(|event| !opens_turn(event))
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
            // The status bits the action gave and the battle agility it set:
            // the effects the log sees that the port reports as events of
            // their own, checked before the slot-by-slot walk below - which
            // only reads the slots the log resolved, and an ability's status
            // arm resolves a slot with no damage word at all. Both
            // directions: a status the port inflicts that the log never
            // gained is as much a divergence as one it does not.
            if let Some(found) = effect_divergence(action, actor, &turn_of(&events)) {
                return Some(found);
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
                .take_while(|event| !opens_turn(event))
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
                    event if opens_turn(event) => break,
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
                            !opens_turn(event)
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
