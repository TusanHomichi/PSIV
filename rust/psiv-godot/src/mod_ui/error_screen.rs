//! On-screen PSIV-styled error panel shown when a mod fails to load (#785).
//!
//! Replaces the silent early return in `Field::ready()` so a broken mod names
//! the offending mod, file, and error detail on screen with direct recovery
//! actions (Disable & Retry, Retry, or Launch Unmodified for this session).

use godot::classes::box_container::AlignmentMode;
use godot::classes::control::{LayoutPreset, SizeFlags};
use godot::classes::{
    Button, CanvasLayer, CenterContainer, HBoxContainer, ICanvasLayer, Label, MarginContainer,
    VBoxContainer,
};
use godot::global::HorizontalAlignment;
use godot::prelude::*;
use psiv_data::mods::{ModError, set_mod_enabled};

use crate::install::{ModSourceMode, set_session_ignore_mods};
use crate::mod_ui::ModErrorView;
use crate::pack_setup_style::{
    BORDER_STEEL, SetupBackdrop, TEXT_BODY, TEXT_CYAN, TEXT_ERROR, TEXT_GOLD, TEXT_MUTED,
    TEXT_WHITE, divider, psiv_compact_panel, psiv_window, style_button, styled_label,
    styled_label_aligned,
};

#[derive(GodotClass)]
#[class(base=CanvasLayer, no_init)]
pub(crate) struct ModErrorScreen {
    base: Base<CanvasLayer>,
    view: ModErrorView,
    status: Option<Gd<Label>>,
    primary_button: Option<Gd<Button>>,
    frames: u32,
    shot_taken: bool,
}

impl ModErrorScreen {
    pub(crate) fn create(source: ModSourceMode, error: ModError) -> Gd<Self> {
        let view = ModErrorView::from_error(&source, &error);
        Gd::from_init_fn(|base| Self {
            base,
            view,
            status: None,
            primary_button: None,
            frames: 0,
            shot_taken: false,
        })
    }

    fn on_disable_and_retry(&mut self) {
        let Some(target) = self.view.disable_target.clone() else {
            return;
        };
        match set_mod_enabled(&target.mod_dir, false) {
            Ok(_) => {
                godot_print!(
                    "mod error screen: disabled `{}` at {}; reloading",
                    target.mod_id,
                    target.mod_dir.display()
                );
                set_session_ignore_mods(false);
                self.reload_scene();
            }
            Err(error) => {
                self.show_status(&format!("Could not disable mod: {error}"));
            }
        }
    }

    fn on_retry(&mut self) {
        godot_print!("mod error screen: retrying mod load");
        set_session_ignore_mods(false);
        self.reload_scene();
    }

    fn on_launch_unmodified(&mut self) {
        godot_print!("mod error screen: launching unmodified for this session");
        set_session_ignore_mods(true);
        self.reload_scene();
    }

    fn reload_scene(&mut self) {
        let base = self.base_mut();
        base.get_tree().reload_current_scene();
    }

    fn show_status(&mut self, text: &str) {
        if let Some(status) = self.status.as_mut() {
            status.set_text(text);
            status.set_visible(!text.is_empty());
        }
    }

    fn maybe_capture_debug_shot(&mut self) {
        if self.shot_taken || self.frames < 2 {
            return;
        }
        let Ok(path) = std::env::var("PSIV_DEBUG_SHOT") else {
            self.shot_taken = true;
            return;
        };
        self.shot_taken = true;
        let Some(viewport) = self.base().get_viewport() else {
            return;
        };
        let Some(texture) = viewport.get_texture() else {
            return;
        };
        let Some(image) = texture.get_image() else {
            return;
        };
        let error = image.save_png(&GString::from(path.as_str()));
        godot_print!("debug: mod error screenshot -> {path} ({error:?})");
    }
}

#[godot_api]
impl ICanvasLayer for ModErrorScreen {
    fn ready(&mut self) {
        let mut backdrop = SetupBackdrop::new_alloc();
        backdrop.set_anchors_preset(LayoutPreset::FULL_RECT);
        self.base_mut().add_child(&backdrop);

        let mut margin = MarginContainer::new_alloc();
        margin.set_anchors_preset(LayoutPreset::FULL_RECT);
        for side in ["margin_left", "margin_right", "margin_top", "margin_bottom"] {
            margin.add_theme_constant_override(side, 24);
        }

        let mut center = CenterContainer::new_alloc();
        center.set_h_size_flags(SizeFlags::EXPAND_FILL);
        center.set_v_size_flags(SizeFlags::EXPAND_FILL);

        let mut stack = VBoxContainer::new_alloc();
        stack.set_alignment(AlignmentMode::CENTER);
        stack.add_theme_constant_override("separation", 20);

        let mut header_box = VBoxContainer::new_alloc();
        header_box.set_alignment(AlignmentMode::CENTER);
        header_box.add_theme_constant_override("separation", 4);
        header_box.add_child(&styled_label("PHANTASY STAR IV", 28, TEXT_GOLD, false));
        header_box.add_child(&styled_label(&self.view.subtitle, 14, TEXT_ERROR, false));
        let mut header_panel = psiv_window(&header_box, 36.0, 12.0);
        header_panel.set_h_size_flags(SizeFlags::SHRINK_CENTER);
        stack.add_child(&header_panel);

        let mut column = VBoxContainer::new_alloc();
        column.set_alignment(AlignmentMode::CENTER);
        column.set_custom_minimum_size(Vector2::new(760.0, 0.0));
        column.add_theme_constant_override("separation", 14);
        column.add_child(&styled_label(&self.view.headline, 24, TEXT_WHITE, false));
        column.add_child(&divider());

        let mut details = VBoxContainer::new_alloc();
        details.add_theme_constant_override("separation", 8);
        details.add_child(&styled_label_aligned(
            &format!("Mod:   {}", self.view.mod_label),
            16,
            TEXT_GOLD,
            true,
            HorizontalAlignment::LEFT,
        ));
        details.add_child(&styled_label_aligned(
            &format!("File:  {}", self.view.file_label),
            16,
            TEXT_CYAN,
            true,
            HorizontalAlignment::LEFT,
        ));
        details.add_child(&styled_label_aligned(
            &self.view.detail,
            15,
            TEXT_ERROR,
            true,
            HorizontalAlignment::LEFT,
        ));
        let detail_panel = psiv_compact_panel(&details, 18.0, 14.0, BORDER_STEEL);
        column.add_child(&detail_panel);

        column.add_child(&styled_label(&self.view.recovery_hint, 15, TEXT_BODY, true));

        let mut buttons = HBoxContainer::new_alloc();
        buttons.set_alignment(AlignmentMode::CENTER);
        buttons.add_theme_constant_override("separation", 14);

        let mut first_button: Option<Gd<Button>> = None;
        if let Some(label) = &self.view.disable_button_label {
            let mut disable = Button::new_alloc();
            disable.set_text(label);
            style_button(&mut disable);
            disable
                .signals()
                .pressed()
                .connect_other(&self.to_gd(), Self::on_disable_and_retry);
            buttons.add_child(&disable);
            first_button = Some(disable);
        }

        let mut retry = Button::new_alloc();
        retry.set_text("Retry");
        style_button(&mut retry);
        retry
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::on_retry);
        buttons.add_child(&retry);
        if first_button.is_none() {
            first_button = Some(retry);
        }

        let mut unmodified = Button::new_alloc();
        unmodified.set_text("Launch Unmodified");
        style_button(&mut unmodified);
        unmodified
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::on_launch_unmodified);
        buttons.add_child(&unmodified);

        column.add_child(&buttons);

        let mut status = styled_label("", 15, TEXT_MUTED, true);
        status.set_visible(false);
        column.add_child(&status);

        let main_panel = psiv_window(&column, 36.0, 24.0);
        stack.add_child(&main_panel);

        center.add_child(&stack);
        margin.add_child(&center);
        self.base_mut().add_child(&margin);

        if let Some(mut button) = first_button {
            button.grab_focus();
            self.primary_button = Some(button);
        }
        self.status = Some(status);

        godot_print!(
            "mod error screen: mod=`{}` file=`{}` detail={}",
            self.view.mod_label,
            self.view.file_label,
            self.view.detail
        );
    }

    fn process(&mut self, _delta: f64) {
        self.frames = self.frames.saturating_add(1);
        self.maybe_capture_debug_shot();
    }
}
