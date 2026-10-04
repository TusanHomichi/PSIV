//! Type-0 area interactions through ordinary Session pads, issue #79.
//! Fixture saves place the party; these are not connected campaign routes.

use std::path::PathBuf;
use std::sync::OnceLock;

use psiv_core::{GameState, RetailLocation, RetailSave};
use psiv_data::{GameData, MapId};
use psiv_runtime::{Button, DialogueSignal, Pad, Session};

fn pack() -> Option<&'static GameData> {
    static DATA: OnceLock<Option<GameData>> = OnceLock::new();
    DATA.get_or_init(|| {
        let path = std::env::var_os("PSIV_RUNTIME_PACK").map_or_else(
            || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack"),
            PathBuf::from,
        );
        if !path.join("manifest.json").is_file() {
            eprintln!("skipping: runtime pack absent at {}", path.display());
            return None;
        }
        let data = GameData::load(&path).expect("runtime pack loads");
        if data.dialogue().unwrap().trees.world_interaction.is_none() {
            eprintln!("skipping: rebuild local pack for world_interaction data");
            return None;
        }
        Some(data)
    })
    .as_ref()
}

fn at(data: &GameData, world: u8, map: u16, x: u16, y: u16) -> Session {
    Session::start(data.clone())
        .from_save(RetailSave {
            snapshot: GameState::new().snapshot(),
            location: RetailLocation {
                world_index: u16::from(world) << 8,
                map_index_2: 0,
                map_index: map,
                char_x: x * 16,
                char_y: y * 16,
            },
        })
        .expect("fixture save starts")
}

fn speak(session: &mut Session) {
    let frame = session.frame(Pad::new(Button::Speak));
    for signal in frame.window_signals.iter().chain(&frame.signals) {
        assert!(!matches!(signal, DialogueSignal::Fault(_)), "{signal:?}");
    }
}

#[test]
fn piata_area_uses_live_world_instead_of_the_maps_empty_entry() {
    let Some(data) = pack() else {
        return;
    };
    let map_tree = data.map(MapId(0x10)).unwrap().dialogue_tree;
    let dialogue = data.dialogue().unwrap();
    assert!(
        dialogue.entry(map_tree, 0).unwrap().segments.is_empty(),
        "the old per-map selection is the negative control"
    );
    // Piata's first area's rectangle starts at (14,22); CONTINUE faces down.
    // Worlds 1..5 here are counterfactual fixtures proving no geography cache.
    for world in 0..6 {
        let mut session = at(data, world, 0x10, 14, 21);
        speak(&mut session);
        let view = session
            .runtime()
            .dialogue_view()
            .expect("area opens its window");
        let expected = dialogue
            .trees
            .world_interaction
            .as_ref()
            .unwrap()
            .world_trees[usize::from(world)];
        assert_ne!(expected, map_tree);
        assert_eq!(view.tree, expected);
        assert!(!view.scene_dialogue);
        assert_eq!(
            view.lines,
            dialogue.entry(expected, 0).unwrap().pages[0].lines
        );
    }
}

#[test]
fn aiedo_high_entry_uses_the_first_world_override() {
    let Some(data) = pack() else {
        return;
    };
    let area = &data.map(MapId(0x54)).unwrap().interaction_areas[0];
    let dialogue = data.dialogue().unwrap();
    let selection = dialogue.trees.world_interaction.as_ref().unwrap();
    assert!(area.parameter >= selection.first_world_override.entry_from);
    assert!(
        dialogue
            .entry(selection.world_trees[0], u16::from(area.parameter))
            .is_none(),
        "negative control: ignoring the high-entry branch cannot read this entry"
    );
    let mut session = at(data, 0, 0x54, 17, 33);
    speak(&mut session);
    let view = session.runtime().dialogue_view().expect("high area opens");
    assert_eq!(view.tree, selection.first_world_override.tree);
    assert_eq!(
        view.lines,
        dialogue
            .entry(view.tree, u16::from(area.parameter))
            .unwrap()
            .pages[0]
            .lines
    );
}

#[test]
fn neutral_pad_does_not_open_an_area() {
    let Some(data) = pack() else {
        return;
    };
    let mut session = at(data, 0, 0x10, 14, 21);
    for _ in 0..30 {
        session.frame(Pad::NEUTRAL);
        assert!(!session.runtime().dialogue_open());
    }
}

#[test]
fn npc_talk_keeps_the_map_tree_on_every_world() {
    let Some(data) = pack() else {
        return;
    };
    let record = data.map(MapId(0x1A)).unwrap();
    let npc = &record.npcs[0];
    for world in 0..6 {
        let mut session = at(data, world, 0x1A, npc.x_cell as u16, npc.y_cell as u16 - 1);
        speak(&mut session);
        let view = session.runtime().dialogue_view().expect("NPC opens");
        assert_eq!(view.tree, record.dialogue_tree);
    }
}
