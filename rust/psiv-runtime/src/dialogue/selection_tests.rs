//! Live selector, scene-tree and missing-data controls for issue #79.

use std::path::PathBuf;
use std::sync::Arc;

use psiv_core::{Cell, Direction, StepFrames};
use psiv_data::GameData;

use super::DialogueSignal;
use super::selection::DialogueSource;
use crate::Runtime;

fn runtime() -> Option<Runtime> {
    let path = std::env::var_os("PSIV_RUNTIME_PACK").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack"),
        PathBuf::from,
    );
    if !path.join("manifest.json").is_file() {
        eprintln!("skipping: runtime pack absent at {}", path.display());
        return None;
    }
    Some(
        Runtime::new(
            GameData::load(&path).expect("pack loads"),
            0x10,
            Cell::new(14, 21),
            Direction::Down,
            StepFrames::default(),
        )
        .expect("Piata fixture starts"),
    )
}

#[test]
fn selector_reads_world_changes_without_reloading_the_map() {
    let Some(mut rt) = runtime() else {
        return;
    };
    let Some(table) = rt.dialogue_pack().trees.world_interaction.clone() else {
        eprintln!("skipping: rebuild local pack for world_interaction data");
        return;
    };
    let original_map = rt.map_id();
    let map_tree = rt.map_record().unwrap().dialogue_tree;
    let cutoff = table.first_world_override.entry_from;
    assert!(cutoff > 0);
    for world in 0..6 {
        rt.set_world_index(world);
        assert_eq!(
            rt.select_dialogue_tree(DialogueSource::Area { entry: cutoff - 1 }),
            Some(table.world_trees[usize::from(world)])
        );
        assert_eq!(
            rt.select_dialogue_tree(DialogueSource::Area { entry: cutoff }),
            Some(if world == 0 {
                table.first_world_override.tree
            } else {
                table.world_trees[usize::from(world)]
            })
        );
        assert_eq!(rt.select_dialogue_tree(DialogueSource::Npc), Some(map_tree));
        assert_eq!(rt.scene_dialogue_tree(), Some(map_tree));
        assert_eq!(rt.map_id(), original_map);
    }
    for invalid in [6, 255] {
        rt.set_world_index(invalid);
        assert_eq!(
            rt.select_dialogue_tree(DialogueSource::Area { entry: 0 }),
            None
        );
        assert!(!rt.open_area_dialogue(0));
        assert!(
            rt.dialogue
                .drain_signals()
                .iter()
                .any(|signal| matches!(signal, DialogueSignal::Fault(_)))
        );
        assert_eq!(rt.scene_dialogue_tree(), Some(map_tree));
    }
}

#[test]
fn scene_explicit_tree_is_not_replaced_by_the_world_selector() {
    let Some(mut rt) = runtime() else {
        return;
    };
    let explicit = rt
        .dialogue_pack()
        .trees
        .trees
        .iter()
        .find(|tree| tree.tree != rt.map_record().unwrap().dialogue_tree)
        .unwrap()
        .clone();
    rt.scene_tree_address = explicit.rom_address();
    for world in 0..6 {
        rt.set_world_index(world);
        assert_eq!(rt.scene_dialogue_tree(), Some(explicit.tree));
    }
    rt.scene_tree_address = Some(u32::MAX);
    assert_eq!(
        rt.scene_dialogue_tree(),
        None,
        "unknown scene address never falls back"
    );
}

#[test]
fn missing_table_faults_instead_of_falling_back_to_map_dialogue() {
    let Some(mut rt) = runtime() else {
        return;
    };
    Arc::make_mut(rt.dialogue.set.as_mut().unwrap())
        .trees
        .world_interaction = None;
    assert!(!rt.open_area_dialogue(0));
    assert!(!rt.dialogue_open());
    assert!(rt.dialogue.drain_signals().iter().any(
        |signal| matches!(signal, DialogueSignal::Fault(message) if message.contains("rebuild"))
    ));
    assert!(
        rt.scene_dialogue_tree().is_some(),
        "map dialogue remains independent"
    );
}

#[test]
fn an_open_message_keeps_its_tree_until_the_next_open() {
    let Some(mut rt) = runtime() else {
        return;
    };
    let Some(table) = rt.dialogue_pack().trees.world_interaction.clone() else {
        eprintln!("skipping: rebuild local pack for world_interaction data");
        return;
    };
    assert!(rt.open_area_dialogue(0));
    rt.set_world_index(1);
    rt.dialogue_tick();
    assert_eq!(rt.dialogue_view().unwrap().tree, table.world_trees[0]);
    rt.close_dialogue();
    assert!(rt.open_area_dialogue(0));
    assert_eq!(rt.dialogue_view().unwrap().tree, table.world_trees[1]);
}
