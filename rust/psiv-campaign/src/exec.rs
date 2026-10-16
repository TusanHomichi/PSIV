//! One objective: which controller it calls, and how many frames it may use.

use crate::driver::Driver;
use crate::halt::{Halt, HaltKind, Res};
use crate::route::{Face, Objective};

/// What the run remembers between objectives.
#[derive(Debug, Default)]
pub struct Memory {
    /// Event (scripted) battles already claimed by a `fight_scripted`.
    pub claimed_event_battles: usize,
}

/// The frames an objective may spend before it is declared stuck.
///
/// Generous on purpose: a walk meets random battles, which are most of its
/// frames, and a scene's dialogue runs a few hundred frames a page. A patrol is
/// the route's grinding and gets a budget to match.
#[must_use]
pub fn budget_for(objective: &Objective) -> u64 {
    match objective {
        Objective::GoTo { .. } | Objective::GoToMap { .. } | Objective::StepOnto { .. } => 120_000,
        Objective::Wait { frames } => u64::from(*frames) + 20_000,
        Objective::Talk { .. }
        | Objective::Answer { .. }
        | Objective::Interact { .. }
        | Objective::OpenChest { .. }
        | Objective::FightScripted => 120_000,
        // The flight alone is some 1,600 frames; the budget covers a walk onto
        // the row and a retry.
        Objective::Board { .. } => 40_000,
        Objective::Buy { .. }
        | Objective::Sell { .. }
        | Objective::RestInn { .. }
        | Objective::Equip { .. }
        | Objective::UseTechnique { .. }
        | Objective::UseItem { .. }
        | Objective::Reorder { .. }
        | Objective::Dismount
        | Objective::Save { .. } => 20_000,
        Objective::Patrol { .. } => 6_000_000,
        Objective::Expect(_) => 60_000,
    }
}

/// Runs `objective` to completion.
///
/// # Errors
///
/// The controller's [`Halt`].
pub fn execute(driver: &mut Driver, memory: &mut Memory, objective: &Objective) -> Res {
    let face = |face: &Option<Face>| face.map(Face::direction);
    match objective {
        Objective::GoTo { map, cell } => driver.go_to(*map, cell.cell()),
        Objective::StepOnto { map, cell } => driver.step_onto(*map, cell.cell()),
        Objective::GoToMap {
            map,
            via_warp,
            arrival_scene: false,
        } => driver.go_to_map(*map, *via_warp),
        Objective::GoToMap {
            map,
            via_warp,
            arrival_scene: true,
        } => driver.go_to_map_scene(*map, *via_warp),
        Objective::Talk { npc, .. } => driver.talk(*npc as usize).map(|_| ()),
        Objective::Wait { frames } => {
            driver.settle(false)?;
            driver.neutral(*frames)?;
            driver.settle(false).map(|_| ())
        }
        Objective::Answer { yes } => driver.answer(*yes).map(|_| ()),
        Objective::Interact { cell, face, .. } => {
            driver.interact(cell.cell(), face.direction()).map(|_| ())
        }
        Objective::OpenChest { chest } => driver.open_chest(*chest as usize),
        Objective::Buy {
            item,
            count,
            face: f,
        } => driver.buy(item, *count, face(f)),
        Objective::Sell { item, face: f } => driver.sell(item, face(f)),
        Objective::RestInn { face: f } => driver.rest_inn(face(f)),
        Objective::Equip { member, item } => driver.equip(member, item),
        Objective::UseTechnique {
            caster,
            technique,
            target,
        } => driver.use_technique(caster, technique, target),
        Objective::UseItem { item, target } => driver.use_item(item, target),
        Objective::Reorder { order } => driver.reorder(order),
        Objective::Save { slot } => driver.save_slot(*slot),
        Objective::Board { step, to } => {
            let world = crate::ship::world_of(to, driver.runtime().data())
                .map_err(|reason| Halt::new(HaltKind::UnexpectedState, reason))?;
            match step {
                Some(step) => driver.board(step.direction(), world),
                None => driver.board_from_scene(world),
            }
        }
        Objective::FightScripted => fight_scripted(driver, memory),
        Objective::Dismount => driver.dismount(),
        Objective::Patrol {
            map,
            a,
            b,
            until,
            refuge,
        } => driver.patrol(*map, a.cell(), b.cell(), until, refuge),
        Objective::Expect(expect) => driver.expect(expect),
    }
}

/// Wins the scripted battle: one that began since the last claim, or one the
/// game starts while the controllers wait for it.
fn fight_scripted(driver: &mut Driver, memory: &mut Memory) -> Res {
    driver.settle(false)?;
    let events = driver
        .battles()
        .iter()
        .filter(|b| b.kind == "event")
        .count();
    if events <= memory.claimed_event_battles {
        return Err(Halt::new(
            HaltKind::UnexpectedState,
            "no scripted battle began: the objective before this one should start it",
        ));
    }
    memory.claimed_event_battles = events;
    Ok(())
}
