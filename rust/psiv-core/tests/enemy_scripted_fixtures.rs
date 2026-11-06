//! The party-script captures of enemy abilities
//! (`src/battle/replay_fixtures/enemy_scripted/`) hold the cast each one was
//! taken for. `every_fixture_replays_as_recorded` replays them against the
//! runtime pack; this pins, without the pack, that the committed evidence is
//! the observation the documents cite (`docs/battle/ENEMY_EFFECT_ABILITIES.md`,
//! section 10).

use serde_json::Value;

fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/battle/replay_fixtures/enemy_scripted")
        .join(format!("{name}.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).expect("a fixture is JSON")
}

/// Every action of every round, in order.
fn actions(fixture: &Value) -> Vec<&Value> {
    fixture["rounds"]
        .as_array()
        .expect("rounds")
        .iter()
        .flat_map(|round| round["actions"].as_array().expect("actions"))
        .collect()
}

/// #65: 72 BloodSaber casts DEBAN (`$2D`) once Chaz's swing has hit it, and
/// the extractor reads the enemy-side `dfs_bat` (`$2A`) it raises.
#[test]
fn bloodsabers_deban_follows_a_hit_and_raises_both_sabres_defence() {
    let fixture = fixture("deban_bloodsaber");
    let enemies = fixture["formation"]["enemies"].as_array().expect("enemies");
    assert!(enemies.iter().all(|enemy| enemy["enemy_id"] == 72));
    let actions = actions(&fixture);
    let cast = actions
        .iter()
        .position(|action| action["actor"] == 6 && action["ability"] == 0x2D)
        .expect("fighter 6 casts DEBAN");
    assert!(
        actions[..cast].iter().any(|action| action["actor"] == 2
            && action["kind"] == "attack"
            && action["targets"][0]["id"] == 6
            && action["targets"][0]["damage"]
                .as_u64()
                .is_some_and(|d| d > 0)),
        "Chaz hits fighter 6 before its turn"
    );
    let stats = &actions[cast]["effect"]["stats"];
    for fighter in [6, 7] {
        assert!(
            stats
                .as_array()
                .expect("stats")
                .contains(&serde_json::json!([fighter, "dfs_bat", 22, 76])),
            "fighter {fighter}'s battle defence: {stats}"
        );
    }
}
