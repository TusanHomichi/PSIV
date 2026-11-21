//! Retail-pack combination attack integration tests.

use std::path::Path;

use psiv_core::battle::{
    Battle, BattleEvent, Command, FighterId, PartyMember, RoundOrders, SliceRolls,
};
use psiv_data::BattleFiles;

use crate::encounters::battle_data;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn id(v: u8) -> FighterId {
    FighterId::new(v).expect("valid fighter id")
}

#[test]
fn retail_pack_loads_all_fourteen_combos_and_executes_fire_storm() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let files = BattleFiles::load(pack).expect("battle files load");
    assert_eq!(files.abilities.combos.len(), 14);
    let data = battle_data(&files).expect("battle data builds");
    assert_eq!(data.combos().count(), 14);

    // Seat Chaz (knows Foi=1) and Alys (knows Zan=10) from retail character records.
    let mut chaz = files
        .characters
        .characters
        .iter()
        .find(|c| c.character_id == 0)
        .expect("Chaz")
        .clone();
    let mut alys = files
        .characters
        .characters
        .iter()
        .find(|c| c.character_id == 1)
        .expect("Alys")
        .clone();
    // Ensure both know Foi (1) and Zan (10) and outspeed the enemy.
    chaz.techniques.slots[0] = 1;
    alys.techniques.slots[0] = 10;
    let chaz_rec = crate::encounters::character_record(&chaz, &files.enemies.properties).unwrap();
    let alys_rec = crate::encounters::character_record(&alys, &files.enemies.properties).unwrap();
    let mut p_chaz = PartyMember::seat(&chaz_rec, &data).unwrap();
    let mut p_alys = PartyMember::seat(&alys_rec, &data).unwrap();
    p_chaz.stats.curr_tp = 30;
    p_chaz.stats.max_tp = 30;
    p_chaz.stats.agility.battle = 40;
    p_alys.stats.curr_tp = 30;
    p_alys.stats.max_tp = 30;
    p_alys.stats.agility.battle = 50;

    let formation = files
        .formations
        .formations
        .iter()
        .find(|f| f.enemies.len() >= 2)
        .expect("multi-enemy formation");
    let rec = psiv_core::battle::FormationRecord {
        id: formation.id.unwrap_or(0),
        ambush_chance: 0,
        run_chance: 0,
        drop_rate: 0,
        drop_item: None,
        enemies: formation
            .enemies
            .iter()
            .map(|e| psiv_core::battle::FormationEnemy {
                slot: e.slot,
                enemy_id: e.enemy_id,
                position: e.position,
            })
            .collect(),
    };

    let mut rolls = SliceRolls::new(&[0; 256]);
    let (mut battle, _) =
        Battle::start(&rec, vec![p_chaz, p_alys], &data, true, &mut rolls).unwrap();
    let events = battle
        .round(
            &RoundOrders::Commands(vec![
                Command::Technique {
                    technique: 1,
                    target: Some(id(6)),
                },
                Command::Technique {
                    technique: 10,
                    target: None,
                },
            ]),
            &data,
            &mut rolls,
        )
        .unwrap();

    assert!(
        events.iter().any(|e| matches!(
            e,
            BattleEvent::ComboUsed {
                combo: 2,
                participants,
                ..
            } if participants.len() == 2
        )),
        "expected FIRE STORM combo in retail pack battle: {events:?}"
    );
}
