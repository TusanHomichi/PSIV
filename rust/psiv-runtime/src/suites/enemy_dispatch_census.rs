//! Every `(enemy, ability)` pair the pack's records can roll, held against the
//! enemy-turn dispatch (`psiv_core::battle::dispatch_owner`).
//!
//! An enemy record names eight regular ability ids and four conditional ones
//! (`EnemyAttack`'s roll, `ps4.asm:19146-19154`, and the instruction block's
//! replacements, `ps4.asm:19157-19168`). The ledger
//! (`docs/battle/ENEMY_ABILITIES.md`) keeps one status per *ability*, which
//! cannot show a carrier the dispatch does not route while another carrier of
//! the same ability is routed; this census is per pair, over every record in
//! the pack. A conditional slot whose arm is `EnemyAI_Nothing` (`$00` or `$13`,
//! `ps4.asm:19389`, a bare `rts`) never writes its ability, so it is not a
//! pair any turn can run (140 Zio2's `$53` NIGHTMARE is the one such slot).

use psiv_core::battle::{EnemyAiCondition, dispatch_owner};
use psiv_data::BattleFiles;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn pack_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(std::env::var_os("PSIV_RUNTIME_PACK").unwrap_or_else(|| "runtime-pack".into()))
}

#[test]
fn every_rollable_pair_has_a_resolver() {
    let pack = pack_path();
    if !pack.join("manifest.json").is_file() {
        eprintln!(
            "runtime pack absent at {}; skipping the dispatch census",
            pack.display()
        );
        return;
    }
    let files = BattleFiles::load(&pack).expect("validated battle pack");
    let data = crate::battle_data(&files).expect("the battle bridge");
    let mut unrouted = BTreeSet::new();
    let mut pairs = 0;
    for entry in &files.enemies.enemies {
        let record = data.enemy(entry.id).expect("bridged enemy");
        let conditional = record
            .condition_ids
            .iter()
            .zip(record.conditional_abilities.iter())
            .filter(|(condition, _)| {
                EnemyAiCondition::from_id(**condition) != Some(EnemyAiCondition::Nothing)
            })
            .map(|(_, ability)| *ability);
        let rollable: BTreeSet<u8> = record
            .regular_abilities
            .iter()
            .copied()
            .chain(conditional)
            .filter(|ability| *ability != 0)
            .collect();
        for ability in rollable {
            pairs += 1;
            if dispatch_owner(&data, record.id, ability).is_none() {
                unrouted.insert((record.id, ability));
            }
        }
    }
    let open: Vec<String> = unrouted
        .iter()
        .map(|(e, a)| {
            format!(
                "({e}, 0x{a:02X}) {} {}",
                data.enemy(*e).map_or("?", |r| r.name.as_str()),
                data.enemy_skill(*a).map_or("?", |s| s.name.as_str())
            )
        })
        .collect();
    assert!(pairs > 0, "the pack's records name no ability");
    assert!(open.is_empty(), "pairs no resolver runs: {open:#?}");
}
