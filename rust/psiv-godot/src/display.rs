//! VDP register 1's display-enable bit, presented.
//!
//! While the runtime's bit is clear ([`psiv_runtime::CramView::display`]) the
//! cartridge shows the backdrop alone: no plane, sprite or window. The shell
//! draws that as one rect over the 320x224 frame in the backdrop's colour,
//! above the windows and the scene's covers. A battle and the title enable
//! the display themselves, so neither shows it. Off a palette that is not
//! live the shell's covers stand in, as they do for the fades.

use godot::classes::ColorRect;
use godot::prelude::*;
use psiv_runtime::CramView;

use crate::Field;
use crate::battle::{BATTLE_FRAME_HEIGHT, BATTLE_FRAME_WIDTH};

/// Above the dialogue window and the menus (z 1000).
const DISPLAY_OFF_Z: i32 = 1001;

/// The colour the whole screen is while the display is off, or `None` while
/// it is on. Off a palette that is not live the fades are the shell's covers
/// (`transitions.rs`), which stand in for the dark frames too, so nothing is
/// drawn here.
pub(crate) fn display_off_colour(view: &CramView) -> Option<[u8; 3]> {
    let word = view.shown_backdrop().filter(|_| view.live)?;
    Some(psiv_data::color::cram_rgb(word))
}

impl Field {
    /// Shows or hides the display-off rect for this frame. `field` is false
    /// on the frames a battle or the title presents.
    pub(super) fn sync_display(&mut self, field: bool) {
        let colour = self
            .session
            .as_ref()
            .filter(|_| field)
            .and_then(|session| display_off_colour(session.runtime().cram()));
        let Some([r, g, b]) = colour else {
            if let Some(node) = self.display_off.as_mut() {
                node.set_visible(false);
            }
            return;
        };
        // The authentic 320x224 frame; the letterbox masks the rest.
        let Some(camera) = self.camera.as_ref() else {
            return;
        };
        let frame = Vector2::new(BATTLE_FRAME_WIDTH, BATTLE_FRAME_HEIGHT);
        let top_left = camera.get_position() - frame / 2.0;
        if self.display_off.is_none() {
            let mut node = ColorRect::new_alloc();
            node.set_z_index(DISPLAY_OFF_Z);
            self.base_mut().add_child(&node);
            self.display_off = Some(node);
        }
        if let Some(node) = self.display_off.as_mut() {
            node.set_color(Color::from_rgba8(r, g, b, 255));
            node.set_position(top_left);
            node.set_size(frame);
            node.set_visible(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::display_off_colour;
    use psiv_runtime::CramView;

    fn view(display: bool, live: bool, backdrop: u16) -> CramView {
        let mut view = CramView {
            display,
            live,
            ..CramView::default()
        };
        view.words[0] = backdrop;
        // A lit line the screen must not show while the display is off.
        view.words[33] = 0x0EEE;
        view
    }

    /// Off: the backdrop's colour, whatever the other words hold. On, or off
    /// a palette that is not live: nothing.
    #[test]
    fn the_display_off_rect_is_the_backdrop_alone() {
        assert_eq!(display_off_colour(&view(false, true, 0)), Some([0, 0, 0]));
        assert_eq!(
            display_off_colour(&view(false, true, 0x000E)),
            Some(psiv_data::color::cram_rgb(0x000E))
        );
        assert_ne!(psiv_data::color::cram_rgb(0x000E), [0, 0, 0]);
        assert_eq!(display_off_colour(&view(false, false, 0x000E)), None);
        assert_eq!(display_off_colour(&view(true, true, 0)), None);
        assert_eq!(display_off_colour(&view(true, false, 0x000E)), None);
    }
}
