//! Inventory bridge: report the actual core dispatch gates, using psiv-data.
use psiv_data::BattleFiles;
use std::path::Path;

#[test]
fn player_ability_dispatch_inventory() {
    let pack = std::env::var_os("PSIV_PLAYER_PACK").map_or_else(
        || Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack")).to_path_buf(),
        Into::into,
    );
    let files = BattleFiles::load(&pack).expect("player inventory requires the decoded pack");
    let data = crate::battle_data(&files).unwrap();
    let techniques: Vec<_> = data.techniques().collect();
    let skills: Vec<_> = data.skills().collect();
    assert_eq!(techniques.len(), 40);
    assert_eq!(skills.len(), 54);
    for tech in &techniques {
        if tech.targeting & 0x10 != 0 {
            assert!(tech.supported(), "technique {} {}", tech.id, tech.name);
        }
        let mut invalid = (*tech).clone();
        invalid.effect = 255;
        assert!(
            !invalid.supported(),
            "unknown effect must fail before payment"
        );
    }
    for skill in &skills {
        assert!(skill.supported(), "skill {} {}", skill.id, skill.name);
        let mut invalid = (*skill).clone();
        invalid.effect = 255;
        assert!(
            !invalid.supported(),
            "unknown skill effect must be rejected"
        );
    }
    if let Some(path) = std::env::var_os("PSIV_PLAYER_INVENTORY") {
        let rows: Vec<_> = techniques
            .iter()
            .map(|t| {
                serde_json::json!({"kind": "technique", "id": t.id,
                "name": t.name, "battle_supported": t.supported()})
            })
            .chain(skills.iter().map(|s| {
                serde_json::json!({"kind": "skill", "id": s.id,
                "name": s.name, "battle_supported": s.supported()})
            }))
            .collect();
        std::fs::write(path, serde_json::to_string_pretty(&rows).unwrap()).unwrap();
    }
}

/// Export route learn bounds through the repository's retail save codec.
/// No party policy or parallel save decoder belongs in the inventory tool.
#[test]
fn player_route_observed_levels() {
    let Some(directory) = std::env::var_os("PSIV_PLAYER_ROUTE_DIR") else {
        return;
    };
    let report = std::env::var_os("PSIV_PLAYER_ROUTE_REPORT").expect("completed route report");
    let output = std::env::var_os("PSIV_PLAYER_ROUTE_LEVELS").expect("route levels output");
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(report["result"], "completed");
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().as_bytes().get(2) == Some(&b'-'))
        .collect();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "route must contain chapter checkpoint saves"
    );
    let chapters: Vec<_> = paths.iter().enumerate().map(|(index, path)| {
        let name = path.file_name().unwrap().to_string_lossy();
        assert_eq!(name[..2].parse::<usize>().unwrap(), index, "contiguous chapters");
        let bytes = std::fs::read(path.join("slot_1.sram")).unwrap();
        let save = psiv_core::RetailSlot::from_bytes(&bytes, 0).unwrap().decode().unwrap();
        let party: Vec<_> = save.snapshot.party.iter().filter(|id| **id != 255).map(|id| {
            let stats = save.snapshot.characters[usize::from(*id)].as_ref().unwrap();
            serde_json::json!({"character_id": id, "level": stats.level,
                "techniques": stats.techniques.iter().copied().filter(|id| *id != 0).collect::<Vec<_>>(),
                "skills": stats.skills.iter().copied().filter(|id| *id != 0).collect::<Vec<_>>()})
        }).collect();
        serde_json::json!({"chapter": name, "party": party,
            "save": std::fs::canonicalize(path.join("slot_1.sram")).unwrap()})
    }).collect();
    let document = serde_json::json!({"result": "completed", "digest": report["digest"],
        "chapters": chapters});
    std::fs::write(
        output,
        serde_json::to_string_pretty(&document).unwrap() + "\n",
    )
    .unwrap();
}
