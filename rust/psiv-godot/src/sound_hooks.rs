use godot::classes::Input;
use godot::prelude::*;

use super::{Field, audio};

impl Field {
    pub(super) fn play_sound(&mut self, id: u8) {
        if let Some(audio) = self.audio.as_mut() {
            audio.play(id);
        }
    }

    /// Scene `PlaySound` writes the same live Sound_Index consumed by the
    /// field/battle shell. Keeping this wrapper separate makes the ownership
    /// boundary explicit: scene audio is not routed through battle SFX.
    pub(super) fn play_scene_sound(&mut self, id: u8) {
        self.play_sound(id);
        godot_print!("scene sound: {id:#04x}");
    }

    /// The runtime has already applied this persistent saved-word write.
    /// Runtime-less debug fixtures retain their separate one-shot cue.
    pub(super) fn save_scene_music(&mut self, id: u8) {
        if self.runtime().is_none() {
            self.presentation.set_debug_saved_music(id);
        }
        godot_print!("scene saved music: {id:#04x}");
    }

    /// At real-session restore edges, read the persistent cartridge word from
    /// runtime. `loc_33DEC` and other retail readers do not consume `$ECEC`.
    /// Only a runtime-less debug fixture falls back to the shell's old cue.
    pub(super) fn restore_saved_music(&mut self) -> bool {
        let runtime_saved = self.runtime().map(|runtime| runtime.saved_sound_index());
        let Some(id) = self.presentation.restore_music_id(runtime_saved) else {
            return false;
        };
        self.play_sound(id);
        godot_print!("restored saved music: {id:#04x}");
        true
    }

    pub(super) fn play_map_music(&mut self) {
        let Some((id, symbol, changes_music)) = self.runtime().and_then(|runtime| {
            runtime.map_record().map(|record| {
                (
                    record.music.id,
                    record.music.symbol.clone(),
                    record.music.changes_music,
                )
            })
        }) else {
            return;
        };
        let debug_override = self
            .audio
            .as_ref()
            .is_some_and(audio::AudioOutput::has_debug_override);
        if changes_music && id != 0 && !debug_override {
            self.play_sound(id);
            godot_print!(
                "music: map request {id:#04x} {}",
                symbol.unwrap_or_default()
            );
        }
    }

    pub(super) fn service_ui_audio(&mut self) {
        let modal = self.battle_presentation_active()
            || self
                .dialogue
                .as_ref()
                .is_some_and(|window| window.bind().is_open())
            || self.shop.as_ref().is_some_and(|shop| shop.bind().is_open())
            || self
                .camp_menu
                .as_ref()
                .is_some_and(|camp| camp.bind().is_open());
        if !modal {
            return;
        }
        let input = Input::singleton();
        let moved = ["ui_up", "ui_down", "ui_left", "ui_right"]
            .into_iter()
            .any(|action| input.is_action_just_pressed(action));
        if moved {
            self.play_sound(0xf2);
        } else if input.is_action_just_pressed("ui_accept")
            || input.is_action_just_pressed("ui_cancel")
        {
            self.play_sound(0xf3);
        }
    }
}
