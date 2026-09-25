//! End-to-end tests for [`super::Battle`].
//!
//! Split out of `engine.rs` under the repo's 1,000-line rule; included with
//! `#[path]` so it stays a child module and can reach the engine's private
//! state, which several of these assert against directly.
//!
//! The cartridge-derived numbers come from `docs/battle/BATTLE_SCOUT.md` §12 (the
//! worked example) and `oracle/README.md` "Battle ground truth" (tapes 07 and
//! 09), by way of [`crate::battle::fixtures`].
//!
//! What stays here is the harness the cases share — the fixture battle
//! driver and the reward reader — plus the module wiring. The cases
//! themselves sit in the topic modules below: `worked_example` (Scout §12
//! end to end), `rounds` (round structure, priority, escape/RUN), `defend`,
//! `turn_order`, `abilities` (enemy abilities and vehicle skills), `rewards`
//! and `handoff`.

use super::*;
use crate::battle::damage::{DAMAGE_DRAWS, calculate_damage, clamp_damage};
use crate::battle::fighters::FIGHTER_SLOTS;
use crate::battle::fixtures;
use crate::battle::records::{CharacterRecord, FormationRecord};
use crate::battle::rewards::level_up;
use crate::battle::rng::{Lcg41, Rng2, SliceRolls};
use crate::battle::stats::status;
use crate::state::VehicleRecord;

fn id(n: u8) -> FighterId {
    FighterId::new(n).expect("a valid id")
}

fn member(record: &CharacterRecord, data: &BattleData) -> PartyMember {
    let item = |id: u8| data.item(id).ok().cloned();
    PartyMember {
        character: record.id,
        name: record.name.clone(),
        stats: Stats::from_character(record, item),
    }
}

/// Tape 07 and 09's party: Alys, Chaz, Hahn, in that slot order.
fn basement_party(data: &BattleData) -> Vec<PartyMember> {
    vec![
        member(&fixtures::alys(), data),
        member(&fixtures::chaz(), data),
        member(&fixtures::hahn(), data),
    ]
}

fn start(
    formation: &FormationRecord,
    party: Vec<PartyMember>,
    data: &BattleData,
    rolls: &mut impl Rolls,
) -> Battle {
    Battle::start(formation, party, data, false, rolls)
        .expect("the fixtures resolve")
        .0
}

/// Runs a battle to its end on a deterministic source, returning the whole
/// timeline. Bounded so a stalled engine fails loudly instead of hanging.
fn play_out(
    battle: &mut Battle,
    orders: &RoundOrders,
    data: &BattleData,
    rolls: &mut impl Rolls,
) -> Vec<BattleEvent> {
    let mut timeline = Vec::new();
    for _ in 0..64 {
        if battle.outcome().is_some() {
            return timeline;
        }
        timeline.extend(battle.round(orders, data, rolls).expect("resolves"));
    }
    panic!("a battle that will not end in 64 rounds");
}

fn rewarded(timeline: &[BattleEvent]) -> Option<(u16, u16, u16, usize)> {
    timeline.iter().find_map(|event| match event {
        BattleEvent::Rewarded {
            experience_total,
            experience_each,
            meseta,
            recipients,
        } => Some((
            *experience_total,
            *experience_each,
            *meseta,
            recipients.len(),
        )),
        _ => None,
    })
}

#[path = "engine_tests_worked_example.rs"]
mod worked_example;

#[path = "engine_tests_rounds.rs"]
mod rounds;

#[path = "engine_tests_defend.rs"]
mod defend;

#[path = "engine_tests_turn_order.rs"]
mod turn_order;

#[path = "engine_tests_abilities.rs"]
mod abilities;

#[path = "engine_tests_rewards.rs"]
mod rewards;

#[path = "engine_tests_handoff.rs"]
mod handoff;

#[path = "engine_tests_oracle.rs"]
mod oracle;

#[path = "engine_tests_replay.rs"]
mod replay;

#[path = "engine_tests_tail.rs"]
mod tail;

use tail::achievable_impl as achievable;
