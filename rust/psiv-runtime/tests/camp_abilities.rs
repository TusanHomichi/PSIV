//! Real-pack field TECH/SKILL resource, targeting, status and RNG behavior.
use psiv_core::battle::Lcg41;
use psiv_core::{CharId, GameState, RetailLocation, RetailSave, StepFrames, field_healing};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{
    CampAbilityKind::{Skill, Technique},
    CampUseResult, Runtime,
};
use std::path::Path;

fn fixture(caster_status: u8, tp: u16) -> Option<Runtime> {
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    if !pack.join("manifest.json").exists() {
        return None;
    }
    let data = GameData::load(pack).unwrap();
    let files = BattleFiles::load(pack).unwrap();
    let mut initial = Runtime::new_game(data.clone(), StepFrames::default()).unwrap();
    initial.enable_battles(&files).unwrap();
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(2)),
        Some(CharId(6)),
        None,
        None,
    ]);
    for (id, hp, max, status) in [
        (0, 10, 400, caster_status),
        (2, 0, 101, 5),
        (6, 0, 999, 0x40),
    ] {
        let stats = game.roster_mut().get_mut(CharId(id)).unwrap();
        stats.curr_hp = hp;
        stats.max_hp = max;
        stats.status = status;
        stats.curr_tp = tp;
        stats.max_tp = 500;
        stats.mental.modified = 31;
        stats.strength.modified = 40;
    }
    let chaz = game.roster_mut().get_mut(CharId(0)).unwrap();
    chaz.techniques = [1, 34, 24, 27, 36, 37, 35, 25, 26, 28, 29, 39, 40, 0, 0, 0];
    chaz.skills = [43, 44, 6, 0, 0, 0, 0, 0];
    chaz.curr_skill_uses = [3; 8];
    let demi = game.roster_mut().get_mut(CharId(6)).unwrap();
    demi.skills = [42, 45, 54, 0, 0, 0, 0, 0];
    demi.curr_skill_uses = [3; 8];
    let mut rt = Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x13,
                char_x: 768,
                char_y: 304,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&files).unwrap();
    rt.set_rng_seed(0x0101_5678);
    Some(rt)
}

fn amount(result: CampUseResult) -> u16 {
    match result {
        CampUseResult::Used { amount, .. } => amount,
        other => panic!("{other:?}"),
    }
}

#[test]
fn field_techniques_use_learned_order_confirmation_costs_and_caster_mental() {
    let Some(mut rt) = fixture(0x10, 300) else {
        return;
    };
    assert_eq!(
        rt.camp_abilities(0, Technique)
            .iter()
            .map(|a| a.id)
            .collect::<Vec<_>>(),
        [34, 24, 27, 36, 37, 35, 25, 26, 28, 29, 39, 40]
    );
    let before = rt.game().snapshot();
    for (id, target) in [(2, 0), (39, 0), (24, 9)] {
        assert!(matches!(
            rt.use_camp_ability(Technique, 0, id, target),
            CampUseResult::Unavailable { .. }
        ));
        assert_eq!(rt.game().snapshot(), before);
    }
    let mut rolls = Lcg41::new(0x0101_5678);
    let _ = field_healing(31, 16, &mut rolls); // RES on dead Hahn still rolls and costs 3 TP.
    assert!(matches!(
        rt.use_camp_ability(Technique, 0, 24, 1),
        CampUseResult::NoEffect { .. }
    ));
    assert_eq!(rt.game().roster().get(CharId(0)).unwrap().curr_tp, 297);
    assert_eq!(amount(rt.use_camp_ability(Technique, 0, 36, 1)), 25);
    assert_eq!(rt.game().roster().get(CharId(2)).unwrap().status, 0);
    let healing = field_healing(31, 16, &mut rolls);
    assert_eq!(amount(rt.use_camp_ability(Technique, 0, 24, 0)), healing);
    assert_eq!(
        rt.game().roster().get(CharId(0)).unwrap().status,
        0,
        "field seal does not prevent casting"
    );
    let first = field_healing(31, 0, &mut rolls);
    let second = field_healing(31, 0, &mut rolls);
    let _android = field_healing(31, 0, &mut rolls);
    let _empty = field_healing(31, 0, &mut rolls);
    assert_eq!(
        amount(rt.use_camp_ability(Technique, 0, 27, 0)),
        first + second
    );
    assert_eq!(rt.game().roster().get(CharId(6)).unwrap().curr_hp, 0);
    let next = field_healing(31, 16, &mut rolls);
    assert_eq!(amount(rt.use_camp_ability(Technique, 0, 24, 0)), next);
    assert_eq!(rt.game().roster().get(CharId(0)).unwrap().curr_tp, 267);
    assert_eq!(
        amount(rt.use_camp_ability(Technique, 0, 37, 1)),
        101 - 25 - second
    );
    assert!(matches!(
        rt.use_camp_ability(Technique, 0, 37, 2),
        CampUseResult::NoEffect { .. }
    ));
    assert_eq!(rt.game().roster().get(CharId(6)).unwrap().status, 0x40);
    if let Some(dir) = std::env::var_os("PSIV_CAMP_ABILITY_SAVE_DIR") {
        rt.save_slot(Path::new(&dir), 0).unwrap();
    }
}

#[test]
fn field_skill_recovery_handles_androids_and_medic_power_revives_humans() {
    let Some(mut rt) = fixture(0, 300) else {
        return;
    };
    assert_eq!(
        rt.camp_abilities(2, Skill)
            .iter()
            .map(|a| a.id)
            .collect::<Vec<_>>(),
        [42, 45, 54]
    );
    let mut rolls = Lcg41::new(0x0101_5678);
    let recover = field_healing(40, 240, &mut rolls);
    // Recover always targets its caster, even if a caller supplies another slot.
    assert_eq!(amount(rt.use_camp_ability(Skill, 2, 42, 0)), recover);
    assert_eq!(rt.game().roster().get(CharId(6)).unwrap().curr_hp, recover);
    assert_eq!(rt.game().roster().get(CharId(6)).unwrap().status, 0);
    assert_eq!(rt.game().roster().get(CharId(0)).unwrap().curr_hp, 10);
    let human = field_healing(40, 0, &mut rolls);
    let revive = field_healing(40, 0, &mut rolls);
    let _android = field_healing(40, 0, &mut rolls);
    let _empty = field_healing(40, 0, &mut rolls);
    assert_eq!(amount(rt.use_camp_ability(Skill, 2, 45, 0)), human + revive);
    assert_eq!(rt.game().roster().get(CharId(2)).unwrap().curr_hp, revive);
    assert_eq!(rt.game().roster().get(CharId(2)).unwrap().status, 0);
    assert_eq!(rt.game().roster().get(CharId(6)).unwrap().curr_hp, recover);
    let medice = field_healing(31, 128, &mut rolls);
    assert_eq!(amount(rt.use_camp_ability(Skill, 0, 43, 2)), medice);
    assert_eq!(
        rt.game().roster().get(CharId(6)).unwrap().curr_hp,
        recover + medice
    );
    let chaz_before = rt.game().roster().get(CharId(0)).unwrap().curr_hp;
    let demi_before = rt.game().roster().get(CharId(6)).unwrap().curr_hp;
    let chaz = field_healing(31, 80, &mut rolls);
    let hahn = field_healing(31, 80, &mut rolls).min(101 - revive);
    let demi = field_healing(31, 80, &mut rolls);
    let _empty = field_healing(31, 80, &mut rolls);
    assert_eq!(
        amount(rt.use_camp_ability(Skill, 0, 44, 0)),
        chaz + hahn + demi
    );
    assert_eq!(
        rt.game().roster().get(CharId(0)).unwrap().curr_hp,
        chaz_before + chaz
    );
    assert_eq!(
        rt.game().roster().get(CharId(6)).unwrap().curr_hp,
        demi_before + demi
    );
    assert_eq!(
        rt.game().roster().get(CharId(6)).unwrap().curr_skill_uses,
        [2, 2, 3, 3, 3, 3, 3, 3]
    );
    assert_eq!(
        rt.game().roster().get(CharId(0)).unwrap().curr_skill_uses,
        [2, 2, 3, 3, 3, 3, 3, 3]
    );
    assert_eq!(rt.game().roster().get(CharId(0)).unwrap().curr_tp, 300);
}

#[test]
fn dead_paralyzed_and_exhausted_casters_do_not_spend_or_change_targets() {
    for (status, tp) in [(4, 30), (2, 30), (0, 0)] {
        let Some(mut rt) = fixture(status, tp) else {
            return;
        };
        let before = rt.game().snapshot();
        assert!(matches!(
            rt.use_camp_ability(Technique, 0, 24, 0),
            CampUseResult::Unavailable { .. }
        ));
        assert_eq!(rt.game().snapshot(), before);
    }
    let Some(mut rt) = fixture(0, 300) else {
        return;
    };
    for _ in 0..3 {
        rt.use_camp_ability(Skill, 0, 43, 1);
    }
    let before = rt.game().snapshot();
    assert!(matches!(
        rt.use_camp_ability(Skill, 0, 43, 1),
        CampUseResult::Unavailable { .. }
    ));
    assert_eq!(rt.game().snapshot(), before);
}
