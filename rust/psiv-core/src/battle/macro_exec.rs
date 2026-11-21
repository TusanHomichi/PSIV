//! Retail battle macro turn-order enforcement and dynamic auto-targeting
//! (`loc_55DE`, `MacroSpecialActionTable @ 0x5838`, `MacroSpecialActionOffs`,
//! `loc_5A98`, `ps4.asm:7824-8409`).

use crate::state::{MacroCommand, MacroRecord};

use super::action::{Reach, weapon_reach};
use super::engine::Command;
use super::fighters::{FighterId, Roster, Side};
use super::item::ItemSource;
use super::order::QueueEntry;
use super::records::BattleData;
use super::retarget::{Deficit, deficit_for_effect, scan_enemies};
use super::rng::Rolls;
use super::stats::status;

/// `loc_55DE` (`ps4.asm:7836-7927`): adjusts the sorted agility turn queue so
/// consecutive active characters listed in `macro_rec` execute in macro order.
///
/// For each adjacent active pair `(first, second)` in `macro_rec`, if `first`
/// sits after `second` in `queue`, `second` is rotated down into `first`'s
/// slot (preserving the slower participant's position in the round and keeping
/// macro participants consecutive for Combination Attacks).
pub(super) fn reorder_queue_for_macro(
    queue: &mut [QueueEntry],
    roster: &Roster,
    macro_rec: &MacroRecord,
) {
    let mut active_fighters = Vec::with_capacity(macro_rec.commands.len());
    for cmd in &macro_rec.commands {
        if cmd.command_index == 0 {
            break;
        }
        let Some(fighter) = roster
            .side(Side::Party)
            .find(|f| f.character == Some(cmd.character_id))
        else {
            continue;
        };
        if !fighter.is_alive() || !fighter.stats.can_act() || fighter.transient {
            continue;
        }
        if queue.iter().any(|e| e.fighter == fighter.id) {
            active_fighters.push(fighter.id);
        }
    }

    for pair in active_fighters.windows(2) {
        let [first, second] = [pair[0], pair[1]];
        let Some(pos_first) = queue.iter().position(|e| e.fighter == first) else {
            continue;
        };
        let Some(pos_second) = queue.iter().position(|e| e.fighter == second) else {
            continue;
        };
        if pos_first > pos_second {
            let moved = queue[pos_second];
            queue.copy_within(pos_second + 1..=pos_first, pos_second);
            queue[pos_first] = moved;
        }
    }
}

/// Finds the active `MacroCommand` for `actor` (matched by `character_id`).
pub(super) fn command_entry(
    roster: &Roster,
    actor: FighterId,
    macro_rec: &MacroRecord,
) -> Option<MacroCommand> {
    let character_id = roster.get(actor)?.character?;
    macro_rec
        .commands
        .iter()
        .copied()
        .take_while(|c| c.command_index != 0)
        .find(|c| c.character_id == character_id)
}

/// Translates `actor`'s entry in `macro_rec` into a concrete [`Command`] with
/// retail macro auto-targeting (`MacroSpecialActionTable` + `loc_5A98`).
pub(super) fn resolve_macro_command(
    roster: &Roster,
    inventory: &crate::Inventory,
    actor: FighterId,
    macro_rec: &MacroRecord,
    data: &BattleData,
    rolls: &mut impl Rolls,
) -> Command {
    let Some(entry) = command_entry(roster, actor, macro_rec) else {
        return Command::Attack;
    };
    match entry.command_index {
        1 => {
            let multi = roster
                .get(actor)
                .and_then(|f| weapon_reach(&f.stats, data).ok().flatten())
                == Some(Reach::All);
            if multi {
                Command::Attack
            } else {
                match scan_enemies(roster, Deficit::Largest, rolls) {
                    Some(target) => Command::AttackTarget(target),
                    None => Command::Attack,
                }
            }
        }
        2 => {
            let id = entry.ability_id;
            let target = special_target(2, id, roster, actor).or_else(|| {
                let tech = data.technique(id)?;
                if !tech.single_target() {
                    return None;
                }
                if (tech.targeting & 0x0F) == 1 {
                    scan_enemies(roster, deficit_for_effect(tech.effect), rolls)
                } else {
                    Some(actor)
                }
            });
            Command::Technique {
                technique: id,
                target,
            }
        }
        3 => {
            let id = entry.ability_id;
            let target = special_target(3, id, roster, actor).or_else(|| {
                let skill = data.skill(id)?;
                if !skill.single_target() {
                    return None;
                }
                if (skill.targeting & 0x0F) == 1 {
                    scan_enemies(roster, deficit_for_effect(skill.effect), rolls)
                } else {
                    Some(actor)
                }
            });
            Command::Skill { skill: id, target }
        }
        4 => {
            let id = entry.ability_id;
            let source = find_item_source(roster, inventory, actor, id);
            let target = special_target(4, id, roster, actor).or_else(|| {
                let item = data.battle_item(id)?;
                if !item.single_target() {
                    return None;
                }
                if (item.targeting & 0x0F) == 1 {
                    scan_enemies(roster, deficit_for_effect(item.effect), rolls)
                } else {
                    Some(actor)
                }
            });
            Command::Item {
                item: id,
                source,
                target,
            }
        }
        5 => Command::Defend,
        _ => Command::Attack,
    }
}

/// `loc_4D96` (`ps4.asm:7099-7131`): equipment slots first, then shared pack.
fn find_item_source(
    roster: &Roster,
    inventory: &crate::Inventory,
    actor: FighterId,
    item_id: u8,
) -> ItemSource {
    if let Some(fighter) = roster.get(actor)
        && let Some(slot) = fighter.stats.equipment.iter().position(|&id| id == item_id)
    {
        return ItemSource::Equipment(slot as u8);
    }
    if let Some(slot) = inventory.slots().iter().position(|&id| id == item_id) {
        return ItemSource::Inventory(slot as u8);
    }
    ItemSource::Inventory(0)
}

/// `MacroSpecialActionTable` (`0x005838`, `ps4.asm:8095-8116`) +
/// `MacroSpecialActionOffs` (`ps4.asm:8079-8086`).
fn special_target(cmd: u8, id: u8, roster: &Roster, actor: FighterId) -> Option<FighterId> {
    let handler = match (cmd, id) {
        // Techniques: Res (24), Gires (25), Nares (26) -> 0; Shift (30) -> 1;
        // Anti (34) -> 2; Rimpa (35) -> 3; Rever (36), Regen (37) -> 4.
        (2, 24..=26) => 0,
        (2, 30) => 1,
        (2, 34) => 2,
        (2, 35) => 3,
        (2, 36 | 37) => 4,
        // Skills: Medice (43) -> 5.
        (3, 43) => 5,
        // Items: WoodCane (16), Monomate (125), Dimate (126), Trimate (127),
        // ShortCake (145), Perolymate (147) -> 0; Antidote (128) -> 2;
        // CureParal (129) -> 3; MoonDew (130), SolDew (134) -> 4;
        // RepairKit (144) -> 6.
        (4, 16 | 125..=127 | 145 | 147) => 0,
        (4, 128) => 2,
        (4, 129) => 3,
        (4, 130 | 134) => 4,
        (4, 144) => 6,
        _ => return None,
    };
    Some(match handler {
        0 => target_largest_hp_diff_human(roster),
        1 => target_smallest_hp_diff_human(roster),
        2 => target_poisoned(roster, actor),
        3 => target_paralyzed(roster, actor),
        4 => target_dead(roster),
        5 => target_largest_hp_diff_any(roster),
        6 => target_largest_hp_diff_android(roster),
        _ => actor,
    })
}

fn default_party_id(roster: &Roster) -> FighterId {
    roster
        .side(Side::Party)
        .next()
        .map_or_else(|| FighterId::new(1).expect("slot 1"), |f| f.id)
}

/// `Macro_TargetLargestHPDiff` (`ps4.asm:8122`): living human with largest `max_hp - curr_hp`.
fn target_largest_hp_diff_human(roster: &Roster) -> FighterId {
    let mut best_diff: i32 = 0;
    let mut best_id = default_party_id(roster);
    for fighter in roster.side(Side::Party) {
        if fighter.stats.is_android() || fighter.stats.status & status::DEAD != 0 {
            continue;
        }
        let diff = i32::from(fighter.stats.max_hp) - i32::from(fighter.stats.curr_hp);
        if diff >= best_diff {
            best_diff = diff;
            best_id = fighter.id;
        }
    }
    best_id
}

/// `Macro_TargetSmallestHPDiff` (`ps4.asm:8150`): matches retail's `d1 = 0`, `cmp.w d0, d1 / ble`.
fn target_smallest_hp_diff_human(roster: &Roster) -> FighterId {
    let mut best_diff: i32 = 0;
    let mut best_id = default_party_id(roster);
    for fighter in roster.side(Side::Party) {
        if fighter.stats.is_android() || fighter.stats.status & status::DEAD != 0 {
            continue;
        }
        let diff = i32::from(fighter.stats.max_hp) - i32::from(fighter.stats.curr_hp);
        if diff < best_diff {
            best_diff = diff;
            best_id = fighter.id;
        }
    }
    best_id
}

/// `Macro_TargetPoisoned` (`ps4.asm:8178`): first living poisoned member, else actor.
fn target_poisoned(roster: &Roster, actor: FighterId) -> FighterId {
    roster
        .side(Side::Party)
        .find(|f| f.stats.status & status::DEAD == 0 && f.stats.status & status::POISONED != 0)
        .map_or(actor, |f| f.id)
}

/// `Macro_TargetParalyzed` (`ps4.asm:8203`): first living paralyzed member, else actor.
fn target_paralyzed(roster: &Roster, actor: FighterId) -> FighterId {
    roster
        .side(Side::Party)
        .find(|f| f.stats.status & status::DEAD == 0 && f.stats.status & status::PARALYZED != 0)
        .map_or(actor, |f| f.id)
}

/// `Macro_TargetDead` (`ps4.asm:8228`): dead human, else living paralyzed, else largest HP diff.
fn target_dead(roster: &Roster) -> FighterId {
    if let Some(dead) = roster
        .side(Side::Party)
        .find(|f| f.stats.status & status::DEAD != 0)
    {
        return dead.id;
    }
    if let Some(paralyzed) = roster
        .side(Side::Party)
        .find(|f| f.stats.status & status::DEAD == 0 && f.stats.status & status::PARALYZED != 0)
    {
        return paralyzed.id;
    }
    target_largest_hp_diff_human(roster)
}

/// `Macro_TargetLargestHPDiff2` (`ps4.asm:8268`): living human or android with largest HP deficit.
fn target_largest_hp_diff_any(roster: &Roster) -> FighterId {
    let mut best_diff: i32 = 0;
    let mut best_id = default_party_id(roster);
    for fighter in roster.side(Side::Party) {
        if fighter.stats.status & status::OUT != 0 {
            continue;
        }
        let diff = i32::from(fighter.stats.max_hp) - i32::from(fighter.stats.curr_hp);
        if diff >= best_diff {
            best_diff = diff;
            best_id = fighter.id;
        }
    }
    best_id
}

/// `Macro_TargetLargestHPDiff3` (`ps4.asm:8296`): living android with largest HP deficit.
fn target_largest_hp_diff_android(roster: &Roster) -> FighterId {
    let mut best_diff: i32 = 0;
    let mut best_id = default_party_id(roster);
    for fighter in roster.side(Side::Party) {
        if !fighter.stats.is_android() || fighter.stats.status & status::ANDROID_DEAD != 0 {
            continue;
        }
        let diff = i32::from(fighter.stats.max_hp) - i32::from(fighter.stats.curr_hp);
        if diff >= best_diff {
            best_diff = diff;
            best_id = fighter.id;
        }
    }
    best_id
}

#[cfg(test)]
#[path = "macro_exec_tests.rs"]
mod tests;
