//! Retail field ailment flash.
//!
//! The ailment windows a defeat queues and the fade that ends play are the
//! session's (`psiv-runtime/src/session/notices.rs`, `session/game_over.rs`):
//! this file keeps the one thing they are not — the red poison flash the
//! presentation draws over the field.

use godot::prelude::*;

use crate::Field;

#[derive(Default)]
pub(super) struct StatusPresentation {
    flash: Option<Gd<godot::classes::ColorRect>>,
}

impl Field {
    pub(super) fn poison_flash_visible(&self) -> bool {
        self.status_presentation
            .flash
            .as_ref()
            .is_some_and(|node| node.is_visible())
    }

    pub(super) fn clear_poison_flash(&mut self) {
        if let Some(flash) = self.status_presentation.flash.as_mut() {
            flash.hide();
        }
    }

    pub(super) fn flash_field_poison(&mut self) {
        let mut flash = self.status_presentation.flash.take().unwrap_or_else(|| {
            let mut rect = godot::classes::ColorRect::new_alloc();
            rect.set_color(Color::from_rgb(1.0, 0.0, 0.0));
            rect.set_z_index(1000);
            self.base_mut().add_child(&rect);
            rect
        });
        let viewport = self.base().get_viewport_rect();
        let inverse = self.base().get_canvas_transform().affine_inverse();
        let start = inverse * viewport.position;
        let end = inverse * (viewport.position + viewport.size);
        flash.set_position(start - Vector2::new(4.0, 4.0));
        flash.set_size(end - start + Vector2::new(8.0, 8.0));
        flash.show();
        self.status_presentation.flash = Some(flash);
    }
}
