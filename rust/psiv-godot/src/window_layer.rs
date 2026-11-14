//! A node that draws a list of window quads and nothing else.
//!
//! A canvas item draws through one material, and the window materials are
//! one per CRAM line (`window_palette::WindowNode::line`). A screen whose
//! window art sits on more than one line, or over a picture that is not
//! window art, hands each line's quads to its own [`WindowLayer`] child, and
//! the shell gives that child its line's material (`Field::attach_window_line`).

use godot::classes::{INode2D, Node2D};
use godot::prelude::*;

use crate::camp::chrome::Quad;

/// One line's window quads.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct WindowLayer {
    base: Base<Node2D>,
    quads: Vec<Quad>,
}

#[godot_api]
impl INode2D for WindowLayer {
    fn init(base: Base<Node2D>) -> Self {
        WindowLayer {
            base,
            quads: Vec::new(),
        }
    }

    fn draw(&mut self) {
        let quads = std::mem::take(&mut self.quads);
        for quad in &quads {
            self.base_mut()
                .draw_texture_rect_region(&quad.texture, quad.dest, quad.src);
        }
        self.quads = quads;
    }
}

impl WindowLayer {
    /// A new layer named `name`, for its screen to add as a child.
    pub(crate) fn named(name: &str) -> Gd<WindowLayer> {
        let mut layer = WindowLayer::new_alloc();
        layer.set_name(name);
        layer
    }

    /// Draws `quads` from the next frame on.
    pub(crate) fn show(&mut self, quads: Vec<Quad>) {
        self.quads = quads;
        self.base_mut().queue_redraw();
    }
}
