//! Ordered scene visibility uses the op's snapshot after the live scene ends.
//! Import the production renderer state without starting Godot or loading a ROM.

#[allow(dead_code)]
#[path = "../src/cutscene/state.rs"]
mod state;

use state::PresentationState;

#[test]
fn an_ordered_panel_op_uses_its_snapshot_even_after_the_scene_ended() {
    let mut state = PresentationState::default();
    assert!(!state.scene_sprites_visible(true));
    assert!(state.sprites_visible(None, false));
    assert!(state.sprites_visible(None, true));
    assert!(state.scene_sprites_visible(false));
    state.set_vram_blanked(true);
    assert!(!state.scene_sprites_visible(false));
}
