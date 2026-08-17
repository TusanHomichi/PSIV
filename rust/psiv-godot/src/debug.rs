use super::*;

impl Field {
    /// Debug-only automation for the fix loop (no effect without the debug
    /// selectors): `PSIV_DEBUG_BATTLE=<formation hex>` or
    /// `--psiv-debug-battle=<formation hex>` starts that battle a few frames
    /// after boot with no play needed; `PSIV_DEBUG_SHOT=<path.png>` (with
    /// optional `PSIV_DEBUG_SHOT_FRAME=<n>`, default 180) saves a viewport
    /// screenshot so an agent can see what a player would. `PSIV_DEBUG_CAMP=1`
    /// opens the field camp at tick 30.
    pub(super) fn debug_hooks_tick(&mut self) {
        let formation = std::env::var("PSIV_DEBUG_BATTLE").ok().or_else(|| {
            std::env::args().find_map(|argument| {
                argument
                    .strip_prefix("--psiv-debug-battle=")
                    .map(str::to_owned)
            })
        });
        if self.anim_tick == 30
            && let Some(formation) = formation
        {
            let trimmed = formation.trim_start_matches("0x");
            match u16::from_str_radix(trimmed, 16) {
                Ok(id) => {
                    godot_print!("debug: starting battle {id:#05x}");
                    if id == 0x88 {
                        self.play_sound(0x95);
                        self.start_oracle_debug_battle();
                    } else {
                        self.play_sound(0x8f);
                        self.start_random_battle(id);
                    }
                }
                Err(_) => godot_error!("debug battle selector {formation} is not hex"),
            }
        }
        if self.anim_tick == 30 && std::env::var("PSIV_DEBUG_CAMP").is_ok_and(|value| value == "1")
        {
            godot_print!("debug: opening camp menu");
            self.open_camp_menu();
        }
        if self.anim_tick == 30
            && let Ok(value) = std::env::var("PSIV_DEBUG_SHOP")
            && let Ok(index) = value.parse::<usize>()
        {
            let opened = match (self.shop.as_mut(), self.runtime.as_ref()) {
                (Some(shop), Some(runtime)) => shop.bind_mut().open_index(index, runtime),
                _ => false,
            };
            if opened {
                godot_print!("debug: opening shop counter {index}");
                self.place_shop_window();
            }
        }
        if let Ok(path) = std::env::var("PSIV_DEBUG_SHOT") {
            let at: u64 = std::env::var("PSIV_DEBUG_SHOT_FRAME")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(180);
            if self.anim_tick == at
                && let Some(viewport) = self.base().get_viewport()
                && let Some(texture) = viewport.get_texture()
                && let Some(image) = texture.get_image()
            {
                let err = image.save_png(&GString::from(path.as_str()));
                godot_print!("debug: screenshot -> {path} ({err:?})");
            }
        }
    }
}
