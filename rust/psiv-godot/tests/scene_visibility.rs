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

#[allow(dead_code)]
#[path = "../src/cutscene/panel_stack.rs"]
mod panel_stack;

#[test]
fn alshline_destroy_loops_pop_lifo_and_commit_only_at_retail_dma_edges() {
    use psiv_core::{EventIndex, SceneOp, scene_for};

    let scene = scene_for(EventIndex(0x8005)).unwrap();
    let mut panels = panel_stack::PanelStack::default();
    let mut pops = Vec::new();
    let mut visible_before_pop = Vec::new();
    let mut destroy_edges = Vec::new();
    let mut pending_pops = 0;
    for op in scene.ops {
        match *op {
            SceneOp::PanelCreate { id } => panels.staged.push(id),
            SceneOp::PanelDestroyLast => {
                let visible = panels.visible.clone();
                visible_before_pop.push(visible.clone());
                pops.push(panels.pop().expect("Alshline must not underflow"));
                pending_pops += 1;
                assert_eq!(panels.visible, visible, "pop must wait for DMA");
            }
            SceneOp::DmaPlanes => {
                panels.dma();
                if pending_pops > 0 {
                    destroy_edges.push((pending_pops, panels.visible.clone()));
                    pending_pops = 0;
                }
            }
            _ => {}
        }
    }
    assert_eq!(pops, [0x1e, 0x1d, 0x1c, 0x1b, 0x1a, 0x20, 0x1f]);
    assert_eq!(destroy_edges, [(5, vec![]), (1, vec![0x1f]), (1, vec![])]);
    assert_eq!(
        visible_before_pop,
        [
            vec![0x1a, 0x1b, 0x1c, 0x1d, 0x1e],
            vec![0x1a, 0x1b, 0x1c, 0x1d, 0x1e],
            vec![0x1a, 0x1b, 0x1c, 0x1d, 0x1e],
            vec![0x1a, 0x1b, 0x1c, 0x1d, 0x1e],
            vec![0x1a, 0x1b, 0x1c, 0x1d, 0x1e],
            vec![0x1f, 0x20],
            vec![0x1f],
        ]
    );
    assert_eq!(pending_pops, 0);
}

#[test]
fn empty_panel_stack_reports_underflow_without_changing_visible_planes() {
    let mut panels = panel_stack::PanelStack::default();
    panels.staged.push(0x1a);
    panels.dma();
    assert_eq!(panels.pop(), Some(0x1a));
    assert_eq!(panels.pop(), None);
    assert_eq!(panels.visible, [0x1a]);
}
