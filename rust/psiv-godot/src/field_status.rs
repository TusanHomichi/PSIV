//! Retail field ailment windows, poison flash, and defeat-to-title handoff.
use godot::prelude::*;
use psiv_runtime::FieldNotice;

use crate::{Field, save_directory, title, transitions::TransitionKind};

#[derive(Default)]
pub(super) struct StatusPresentation {
    notice_open: bool,
    flash: Option<Gd<godot::classes::ColorRect>>,
    defeat_fade: Option<u8>,
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

    /// Runs before ordinary windows; their existing input loop displays and
    /// dismisses the notice. The runtime owns the queue and stays parked.
    pub(super) fn service_field_notices(&mut self) {
        if self.status_presentation.notice_open {
            if self
                .dialogue
                .as_ref()
                .is_some_and(|window| window.bind().is_open())
            {
                return;
            }
            self.status_presentation.notice_open = false;
            if let Some(runtime) = self.runtime.as_mut() {
                runtime.acknowledge_field_notice();
            }
        }
        let Some(notice) = self
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.field_notice())
        else {
            return;
        };
        let lines = match notice {
            FieldNotice::Fallen(who) => {
                let name = self
                    .runtime
                    .as_ref()
                    .and_then(|rt| rt.game().roster().get(who))
                    .map(|stats| stats.display_name())
                    .unwrap_or_default();
                vec![format!("{name} is on the verge"), "of Death.".into()]
            }
            FieldNotice::Perished => {
                vec!["Chaz and his Companions".into(), "have perished.".into()]
            }
        };
        godot_print!("field status: {}", lines.join(" "));
        if let Some(window) = self.dialogue.as_mut() {
            self.status_presentation.notice_open = window.bind_mut().open_status(&lines);
        }
    }

    pub(super) fn begin_game_over(&mut self) {
        if self.status_presentation.defeat_fade.is_none() {
            self.status_presentation.defeat_fade = Some(0);
            self.start_transition(TransitionKind::SceneFadeOut);
        }
    }

    /// Battle defeat waits for its last message first; field defeat has
    /// already waited for the Perished acknowledgement.
    pub(super) fn drive_game_over(&mut self) -> bool {
        if !self.runtime.as_ref().is_some_and(|rt| rt.game_over()) {
            return false;
        }
        if self.status_presentation.defeat_fade.is_none() {
            if self.battle_presentation_active() {
                return false;
            }
            self.begin_game_over();
            return true;
        }
        let frames = self
            .status_presentation
            .defeat_fade
            .as_mut()
            .expect("fade begun");
        *frames += 1;
        if *frames < 14 {
            return true;
        }
        if let Some(screen) = self.battle_screen.as_mut() {
            screen.hide();
        }
        self.battle_field_visibility = None;
        self.presentation.reset_scene();
        if let Some(layer) = self.cutscene_layer.as_mut() {
            layer.bind_mut().end_opening();
            layer.bind_mut().panel_destroy_all();
        }
        self.set_letterbox(false);
        self.play_sound(0xFB);
        let slots = self
            .runtime
            .as_ref()
            .map(|rt| crate::boot::available_save_slots(rt.data(), &save_directory()))
            .unwrap_or([false; 3]);
        let pack_dir = self.pack_dir.clone();
        self.title = title::TitleScreen::build(&pack_dir, slots, self.base_mut());
        self.status_presentation.defeat_fade = None;
        self.accept_blocked = true;
        godot_print!("game over: title restored; saved slots unchanged");
        true
    }
}
