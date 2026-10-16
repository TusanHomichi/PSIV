//! The party policy on a live battle over the real pack: the planner's orders
//! for a whole round, read from the core's own target lists, with an android
//! in the party. The planner's rules on constructed boards are the library's
//! `policy_plan_tests.rs`; this target holds what needs a `Session`.
//!
//! Cases that need the pack skip with a message when `runtime-pack` is absent.

mod common;

use psiv_core::battle::{FighterId, Side, item_targets, skill_targets, technique_targets};
use psiv_core::{CharId, GameState, RetailLocation, RetailSave};
use psiv_runtime::{Session, TechniqueEntry};

use common::{menu_window, pack};

/// The mixed party of `mixed_party_cures_use_the_menus_eligible_targets`:
/// Chaz (who knows RES), Demi the android and Rika (who knows GISAR), with one
/// MONOMATE in the pack, in formation `0x4e`: two enemies with plain attacks
/// only and more HP than one round of this party's offence removes, so the
/// planner's "finish it" rule cannot stand in for a cure. `hurt` sets each
/// member's HP as a share of their maximum, in percent (Chaz, Demi, Rika); 0
/// keeps one HP. Without `recover`, Demi's skill uses are spent, so her own
/// RECOVER (a self-heal) is not on offer and no cure reaches her.
fn mixed_party_battle(item_id: u8, hurt: [u16; 3], recover: bool) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(6)),
        Some(CharId(5)),
        None,
        None,
    ]);
    for (character, percent) in [CharId(0), CharId(6), CharId(5)].into_iter().zip(hurt) {
        let member = game.roster_mut().get_mut(character).unwrap();
        member.curr_hp = (member.max_hp * percent / 100).max(1);
    }
    if !recover {
        game.roster_mut()
            .get_mut(CharId(6))
            .unwrap()
            .curr_skill_uses = [0; _];
    }
    let chaz = game.roster_mut().get_mut(CharId(0)).unwrap();
    chaz.curr_tp = 20;
    chaz.techniques[0] = 24;
    let rika = game.roster_mut().get_mut(CharId(5)).unwrap();
    rika.curr_tp = 50;
    rika.techniques[0] = 28;
    game.inventory_mut().add(item_id).unwrap();
    let mut session = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .from_save(RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x15,
                char_x: 20 * 16,
                char_y: 10 * 16,
            },
        })
        .expect("the mixed party loads");
    assert!(session.debug_battle(0x4e).fault.is_none());
    Some(session)
}

/// Member `actor`'s command window as the menu builds it: every technique the
/// member knows, enabled when affordable, and every skill with a use left.
fn window_of(runtime: &psiv_runtime::Runtime, actor: u8) -> psiv_runtime::CommandMenuView {
    let roster = runtime.battle_roster().expect("the battle has a roster");
    let member = roster.get(FighterId::new(actor).unwrap()).unwrap();
    let mut menu = menu_window(actor);
    menu.techniques = runtime
        .battle_techniques()
        .filter(|tech| member.stats.techniques.contains(&tech.id))
        .map(|tech| TechniqueEntry {
            id: tech.id,
            name: tech.name.clone(),
            cost: tech.cost,
            available: member.stats.curr_tp >= u16::from(tech.cost),
        })
        .collect();
    menu.skills = member
        .stats
        .skills
        .iter()
        .zip(member.stats.curr_skill_uses)
        .filter(|(id, uses)| **id != 0 && *uses > 0)
        .filter_map(|(id, uses)| {
            let skill = runtime.battle_skills().find(|s| s.id == *id)?;
            Some(psiv_runtime::SkillEntry {
                id: skill.id,
                name: skill.name.clone(),
                remaining: uses,
                available: true,
            })
        })
        .collect();
    menu
}

/// One round of the party policy over the live battle: every living member's
/// window in slot order, the first one ordering the round.
fn round_orders(session: &Session, scripted: bool) -> Vec<(u8, psiv_campaign::policy::Intent)> {
    use psiv_campaign::policy::{PartyPolicy, Policy};

    let runtime = session.runtime();
    let roster = runtime.battle_roster().expect("the battle has a roster");
    let mut policy = PartyPolicy::fighting("test_mixed");
    policy.battle_begins(scripted);
    let orders = roster
        .living(Side::Party)
        .map(|fighter| fighter.id.get())
        .collect::<Vec<_>>()
        .into_iter()
        .map(|actor| (actor, policy.choose(&window_of(runtime, actor), runtime)))
        .collect();
    policy.end_round();
    orders
}

/// RES and MONOMATE offer humans, not Demi; GISAR reaches the humans only,
/// and Demi's RECOVER reaches Demi only. The party policy orders a round from
/// those lists:
///
/// * a desperate android with no cure that reaches her, beside a hurt human:
///   the human is cured once and no cure names the android;
/// * the same with Demi's RECOVER on offer: Demi cures herself (the lowest
///   share of HP), and Rika's cure follows in a scripted battle only (a random
///   encounter orders one single cure a round);
/// * only the android hurt, with nothing that reaches her: no cure at all;
/// * two humans under the group line: the learned GISAR, before either falls
///   below half (waiting costs a lethal extra enemy round); one hurt human is
///   no group cure.
///
/// Each case is checked in a random encounter and in a scripted battle.
#[test]
fn mixed_party_cures_use_the_menus_eligible_targets() {
    use psiv_campaign::policy::Intent;

    let Some(set) = pack() else {
        return;
    };
    let item_id = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots")
        .runtime()
        .battle_items()
        .find(|item| item.name == "MONOMATE")
        .expect("MONOMATE is in the battle pack")
        .id;
    let chaz = FighterId::new(1).unwrap();
    let demi = FighterId::new(2).unwrap();
    let rika = FighterId::new(3).unwrap();

    let session = mixed_party_battle(item_id, [100, 0, 33], false).unwrap();
    let runtime = session.runtime();
    let roster = runtime.battle_roster().expect("the battle has a roster");
    let tech = |id: u8| runtime.battle_techniques().find(|t| t.id == id).unwrap();
    let (res, gisar) = (tech(24), tech(28));
    let recover = runtime.battle_skills().find(|s| s.id == 42).unwrap();
    let monomate = runtime
        .battle_items()
        .find(|item| item.id == item_id)
        .unwrap();
    for targets in [
        technique_targets(roster, chaz, res),
        item_targets(roster, chaz, monomate),
        technique_targets(roster, rika, gisar),
    ] {
        assert!(targets.contains(&rika), "Rika must be selectable");
        assert!(
            !targets.contains(&demi),
            "Demi must not appear in the cure menu"
        );
    }
    assert_eq!(skill_targets(roster, demi, recover), [demi]);
    let is_cure = |intent: &Intent| match intent {
        Intent::Technique { id, .. } => [res.id, gisar.id].contains(id),
        Intent::Skill { id, .. } => *id == recover.id,
        Intent::Item { name, .. } => *name == monomate.name,
        _ => false,
    };
    let cures_of = |orders: &[(u8, Intent)]| -> Vec<(u8, Intent)> {
        orders.iter().filter(|(_, i)| is_cure(i)).cloned().collect()
    };
    let on_rika = |intent: &Intent| {
        matches!(
            intent,
            Intent::Technique { target: Some(t), .. } | Intent::Item { target: Some(t), .. }
                if *t == rika.get()
        )
    };

    for scripted in [false, true] {
        let orders = round_orders(&session, scripted);
        let cures = cures_of(&orders);
        assert_eq!(cures.len(), 1, "scripted {scripted}: {orders:?}");
        assert!(
            on_rika(&cures[0].1),
            "scripted {scripted}: Rika gets the one cure: {orders:?}"
        );
    }

    let self_cure = Intent::Skill {
        id: recover.id,
        target: None,
    };
    let with_recover = mixed_party_battle(item_id, [100, 0, 33], true).unwrap();
    for scripted in [false, true] {
        let orders = round_orders(&with_recover, scripted);
        let cures = cures_of(&orders);
        assert!(
            cures.contains(&(demi.get(), self_cure.clone())),
            "scripted {scripted}: {orders:?}"
        );
        let rika_cures = cures.iter().filter(|(_, i)| on_rika(i)).count();
        assert_eq!(
            (cures.len(), rika_cures),
            if scripted { (2, 1) } else { (1, 0) },
            "scripted {scripted}: {orders:?}"
        );
    }

    // Only the android needs HP and nothing reaches her: nobody may order a
    // human cure or spend the item on the wrong character.
    let android_only = mixed_party_battle(item_id, [100, 0, 100], false).unwrap();
    for scripted in [false, true] {
        let orders = round_orders(&android_only, scripted);
        assert!(
            cures_of(&orders).is_empty(),
            "scripted {scripted}: {orders:?}"
        );
    }

    // Two humans at 60%: Rika's GISAR, ordered at Chaz's window for the round.
    let group = Intent::Technique {
        id: gisar.id,
        target: None,
    };
    let two_hurt = mixed_party_battle(item_id, [60, 100, 60], false).unwrap();
    let one_hurt = mixed_party_battle(item_id, [100, 100, 60], false).unwrap();
    for scripted in [false, true] {
        let orders = round_orders(&two_hurt, scripted);
        assert!(
            orders.contains(&(rika.get(), group.clone())),
            "scripted {scripted}: {orders:?}"
        );
        let orders = round_orders(&one_hurt, scripted);
        assert!(
            !orders.iter().any(|(_, i)| *i == group),
            "scripted {scripted}: one hurt human is no group cure: {orders:?}"
        );
    }
}
