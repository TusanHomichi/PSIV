//! Selection schema validation, with malformed-pack negative controls.

use std::path::PathBuf;

use super::DialogueSet;

fn packed() -> Option<(PathBuf, DialogueSet)> {
    let path = std::env::var_os("PSIV_RUNTIME_PACK").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack"),
        PathBuf::from,
    );
    if !path.join("dialogue/trees.json").is_file() {
        eprintln!("skipping: dialogue pack absent at {}", path.display());
        return None;
    }
    Some((
        path.clone(),
        DialogueSet::load(&path).expect("dialogue pack loads"),
    ))
}

#[test]
fn world_selection_validates_count_and_every_tree_reference() {
    let Some((path, good)) = packed() else {
        return;
    };
    if good.trees.world_interaction.is_none() {
        eprintln!("skipping: rebuild local pack for world_interaction data");
        return;
    }
    good.validate(&path).expect("emitted selection validates");
    let mut short = good.clone();
    short
        .trees
        .world_interaction
        .as_mut()
        .unwrap()
        .world_trees
        .pop();
    assert!(
        short
            .validate(&path)
            .unwrap_err()
            .to_string()
            .contains("six world trees")
    );
    let mut absent = good.clone();
    absent.trees.world_interaction.as_mut().unwrap().world_trees[1] = 0;
    assert!(
        absent
            .validate(&path)
            .unwrap_err()
            .to_string()
            .contains("absent tree 0")
    );
    let mut high = good;
    high.trees
        .world_interaction
        .as_mut()
        .unwrap()
        .first_world_override
        .tree = 0;
    assert!(
        high.validate(&path)
            .unwrap_err()
            .to_string()
            .contains("absent tree 0")
    );
}

#[test]
fn legacy_pack_can_read_map_dialogue_without_inventing_world_selection() {
    let Some((path, mut legacy)) = packed() else {
        return;
    };
    legacy.trees.world_interaction = None;
    legacy
        .validate(&path)
        .expect("NPC and scene data remain readable");
    assert!(legacy.trees.world_interaction.is_none());
}
