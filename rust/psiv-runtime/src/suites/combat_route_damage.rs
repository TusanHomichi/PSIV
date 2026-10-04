//! Pack-backed inputs for the core's generic capture replay. Cartridge tables
//! pass through `psiv-data` and the sole runtime bridge; local exports are
//! deliberately ignored and are only written when explicitly requested.

use psiv_data::BattleFiles;
use serde_json::json;
use std::path::{Path, PathBuf};

fn pack_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(std::env::var_os("PSIV_RUNTIME_PACK").unwrap_or_else(|| "runtime-pack".into()))
}

#[test]
fn route_replay_records_load_through_the_validated_pack() {
    let pack = pack_path();
    if !pack.join("manifest.json").is_file() {
        eprintln!(
            "runtime pack absent at {}; skipping route replay input",
            pack.display()
        );
        return;
    }
    let files = BattleFiles::load(&pack).expect("validated battle pack");
    let data = crate::battle_data(&files).expect("the existing battle bridge");
    let enemies = files
        .enemies
        .enemies
        .iter()
        .map(|entry| {
            let e = data.enemy(entry.id).unwrap();
            json!({"id":e.id,"name":e.name,"hp":e.hp,"strength":e.strength,
            "mental":e.mental,"agility":e.agility,"dexterity":e.dexterity,
            "attack":e.attack,"defence":e.defence,"mental_defence":e.mental_defence,
            "attack_element":e.attack_element,"attack_status":e.attack_status,
            "properties":e.properties,"regular_abilities":e.regular_abilities,
            "condition_ids":e.condition_ids,"conditional_abilities":e.conditional_abilities,
            "experience":e.experience,"meseta":e.meseta})
        })
        .collect::<Vec<_>>();
    let skills = files
        .abilities
        .usable()
        .filter(|a| a.kind == "enemy_skills")
        .map(|entry| {
            let s = data.enemy_skill(u8::try_from(entry.id).unwrap()).unwrap();
            json!({"id":s.id,"name":s.name,"effect":s.effect,"power_stat":s.power_stat,
            "target":s.target,"power":s.power,"resistance":s.resistance,"element":s.element})
        })
        .collect::<Vec<_>>();
    assert_eq!(enemies.len(), files.enemies.enemies.len());
    assert!(!skills.is_empty());
    if let Some(output) = std::env::var_os("PSIV_REPLAY_PACK_OUT") {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(output);
        std::fs::create_dir_all(path.parent().expect("output directory")).unwrap();
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({
            "source":"psiv-data::BattleFiles + psiv-runtime::battle_data",
            "enemies":enemies,"enemy_skills":skills}))
            .unwrap(),
        )
        .unwrap();
        eprintln!("wrote local replay input {}", path.display());
    }
}

#[test]
fn flaeli_uses_each_carriers_pack_stats_and_only_its_stored_target() {
    use psiv_core::battle::{
        Battle, BattleEvent, Command, FighterId, FormationEnemy, FormationRecord, PartyMember,
        RoundOrders, SliceRolls, calculate_damage, clamp_damage,
    };
    let pack = pack_path();
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping pack-backed FLAELI carriers");
        return;
    }
    let files = BattleFiles::load(&pack).unwrap();
    let data = crate::battle_data(&files).unwrap();
    for enemy_id in [111, 112, 113, 138, 121, 125] {
        let mut enemy = data.enemy(enemy_id).unwrap().clone();
        // Constructed dispatch fixture only: keep every decoded stat and skill
        // record, but select FLAELI without waiting for another ability's turn.
        enemy.regular_abilities = [0x5A; 8];
        enemy.condition_ids = [0; 4];
        let data = data.clone().with_enemies([enemy]);
        let party = files
            .characters
            .characters
            .iter()
            .take(2)
            .map(|entry| {
                let record =
                    crate::encounters::character_record(entry, &files.enemies.properties).unwrap();
                let mut member = PartyMember::seat(&record, &data).unwrap();
                member.stats.curr_hp = 999;
                member.stats.max_hp = 999;
                member
            })
            .collect();
        let formation = FormationRecord {
            id: 999,
            ambush_chance: 0,
            run_chance: 0,
            drop_rate: 0,
            drop_item: None,
            enemies: vec![FormationEnemy {
                slot: 1,
                enemy_id,
                position: 0,
            }],
        };
        let (mut battle, _) =
            Battle::start(&formation, party, &data, false, &mut SliceRolls::new(&[20])).unwrap();
        let target = FighterId::new(2).unwrap();
        let caster = battle.roster().get(FighterId::new(6).unwrap()).unwrap();
        let recipient = battle.roster().get(target).unwrap();
        let skill = data.enemy_skill(0x5A).unwrap();
        // Enemy_DamageCharacter (ps4.asm:3775-3814), FLAELI's cited
        // mental/fire branch. No retail stats or record are copied into source.
        let expected = clamp_damage(calculate_damage(
            u16::from(caster.stats.mental.battle),
            recipient.stats.mental_defence.battle,
            u16::from(recipient.stats.element_factor(skill.element).unwrap()),
            u16::from(skill.power),
            &mut SliceRolls::new(&[1]),
        ));
        let mut rolls = SliceRolls::new(&[1]);
        let events = battle
            .round(
                &RoundOrders::Commands(vec![Command::Defend; 2]),
                &data,
                &mut rolls,
            )
            .unwrap();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, BattleEvent::UnsupportedAbility { .. }))
        );
        let hits: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                BattleEvent::Resolved {
                    target,
                    damage: Some(damage),
                    ..
                } => Some((*target, *damage)),
                _ => None,
            })
            .collect();
        assert_eq!(hits, [(target, expected)], "carrier {enemy_id}");
        assert_eq!(rolls.drawn(), 9 + 4 + 1 + 16);
    }
}
