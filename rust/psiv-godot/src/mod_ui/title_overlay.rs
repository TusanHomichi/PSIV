//! Title-screen active mods indicator, save-profile mismatch notice banner,
//! and lightweight in-game Mods manager panel (#785).
//!
//! Only constructed when [`ModTitleState::needs_title_overlay`] is true, so
//! vanilla runs allocate zero nodes and preserve 100% retail title parity.

use std::path::PathBuf;

use godot::classes::box_container::AlignmentMode;
use godot::classes::control::{LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::{
    Button, CanvasLayer, CenterContainer, ColorRect, Control, HBoxContainer, ICanvasLayer, Input,
    Label, MarginContainer, PanelContainer, ScrollContainer, VBoxContainer,
};
use godot::global::{HorizontalAlignment, Key};
use godot::prelude::*;
use psiv_runtime::TitlePhase;

use crate::install::set_session_ignore_mods;
use crate::mod_ui::{
    ModRowStatus, ModTitleState, build_mod_list, format_active_mods_banner,
    format_mods_button_label, format_save_profile_notice, title_hud_visibility,
    toggle_mod_in_source,
};
use crate::pack_setup_style::{
    BORDER_SILVER, BORDER_STEEL, TEXT_BODY, TEXT_CYAN, TEXT_ERROR, TEXT_GOLD, TEXT_GREEN,
    TEXT_MUTED, TEXT_WHITE, divider, psiv_compact_panel, psiv_window, style_button,
    style_compact_button, styled_label, styled_label_aligned,
};

enum PendingAction {
    Toggle { mod_dir: PathBuf, enabled: bool },
}

#[derive(GodotClass)]
#[class(base=CanvasLayer, no_init)]
pub(crate) struct TitleModsOverlay {
    base: Base<CanvasLayer>,
    state: ModTitleState,
    phase: TitlePhase,
    notice_dismissed: bool,
    modal_open: bool,
    pending_reload: bool,
    pending_action: Option<PendingAction>,
    m_key_down: bool,
    esc_key_down: bool,

    // Top-left active-mods strip + Mods button.
    hud_bar: Option<Gd<MarginContainer>>,
    active_banner: Option<Gd<PanelContainer>>,
    mods_button: Option<Gd<Button>>,

    // Bottom-center save-profile mismatch notice banner.
    notice_container: Option<Gd<MarginContainer>>,

    // Centered modal Mods panel.
    modal_backdrop: Option<Gd<ColorRect>>,
    modal_rows: Option<Gd<VBoxContainer>>,
    modal_status: Option<Gd<Label>>,
    reload_button: Option<Gd<Button>>,
    close_button: Option<Gd<Button>>,
}

impl TitleModsOverlay {
    pub(crate) fn create(state: ModTitleState) -> Gd<Self> {
        Gd::from_init_fn(|base| Self {
            base,
            state,
            phase: TitlePhase::Sega,
            notice_dismissed: false,
            modal_open: false,
            pending_reload: false,
            pending_action: None,
            m_key_down: false,
            esc_key_down: false,
            hud_bar: None,
            active_banner: None,
            mods_button: None,
            notice_container: None,
            modal_backdrop: None,
            modal_rows: None,
            modal_status: None,
            reload_button: None,
            close_button: None,
        })
    }

    pub(crate) fn is_modal_open(&self) -> bool {
        self.modal_open
    }

    pub(crate) fn sync_phase(&mut self, phase: TitlePhase) {
        self.phase = phase;
        self.refresh_visibility();
    }

    fn refresh_visibility(&mut self) {
        let vis = title_hud_visibility(
            self.phase,
            &self.state.active_mods,
            self.state.discovered_count,
            &self.state.source,
            self.state.save_notice.as_ref(),
            self.notice_dismissed,
        );
        if !vis.show_mods_button && self.modal_open {
            self.modal_open = false;
        }
        if let Some(bar) = self.hud_bar.as_mut() {
            bar.set_visible(vis.show_active_mods_banner || vis.show_mods_button);
        }
        if let Some(banner) = self.active_banner.as_mut() {
            banner.set_visible(vis.show_active_mods_banner);
        }
        if let Some(button) = self.mods_button.as_mut() {
            button.set_visible(vis.show_mods_button);
        }
        if let Some(notice) = self.notice_container.as_mut() {
            notice.set_visible(vis.show_save_notice && !self.modal_open);
        }
        if let Some(modal) = self.modal_backdrop.as_mut() {
            modal.set_visible(self.modal_open);
        }
    }

    fn on_open_mods(&mut self) {
        if self.modal_open {
            self.close_modal();
            return;
        }
        self.modal_open = true;
        self.rebuild_modal_rows();
        self.refresh_visibility();
        if let Some(close) = self.close_button.as_mut() {
            close.grab_focus();
        }
    }

    fn close_modal(&mut self) {
        self.modal_open = false;
        self.refresh_visibility();
    }

    fn on_dismiss_notice(&mut self) {
        self.notice_dismissed = true;
        self.refresh_visibility();
    }

    fn on_apply_and_reload(&mut self) {
        godot_print!("title mods: applying mod changes and reloading");
        set_session_ignore_mods(false);
        let base = self.base_mut();
        base.get_tree().reload_current_scene();
    }

    fn queue_toggle(&mut self, mod_dir: PathBuf, enabled: bool) {
        self.pending_action = Some(PendingAction::Toggle { mod_dir, enabled });
    }

    fn apply_pending_action(&mut self) {
        let Some(action) = self.pending_action.take() else {
            return;
        };
        match action {
            PendingAction::Toggle { mod_dir, enabled } => {
                match toggle_mod_in_source(&self.state.source, &mod_dir, enabled) {
                    Ok(manifest) => {
                        self.pending_reload = true;
                        let verb = if enabled { "Enabled" } else { "Disabled" };
                        self.set_modal_status(
                            &format!(
                                "{verb} `{}`. Choose Apply & Reload to load the new mod setup.",
                                manifest.id
                            ),
                            TEXT_CYAN,
                        );
                        self.rebuild_modal_rows();
                    }
                    Err(error) => {
                        self.set_modal_status(&error, TEXT_ERROR);
                    }
                }
            }
        }
    }

    fn set_modal_status(&mut self, text: &str, color: Color) {
        if let Some(label) = self.modal_status.as_mut() {
            label.set_text(text);
            label.add_theme_color_override("font_color", color);
            label.set_visible(!text.is_empty());
        }
    }

    fn rebuild_modal_rows(&mut self) {
        let list_view = build_mod_list(
            &self.state.source,
            &self.state.active_mods,
            self.pending_reload,
        );
        let owner = self.to_gd();
        if let Some(rows_box) = self.modal_rows.as_mut() {
            let count = rows_box.get_child_count();
            for index in (0..count).rev() {
                if let Some(mut child) = rows_box.get_child(index) {
                    rows_box.remove_child(&child);
                    child.queue_free();
                }
            }
            rows_box.add_child(&styled_label_aligned(
                &list_view.source_summary,
                14,
                TEXT_CYAN,
                true,
                HorizontalAlignment::LEFT,
            ));
            if let Some(note) = &list_view.read_only_note {
                rows_box.add_child(&styled_label_aligned(
                    note,
                    14,
                    TEXT_MUTED,
                    true,
                    HorizontalAlignment::LEFT,
                ));
            }
            if list_view.rows.is_empty() {
                rows_box.add_child(&styled_label(
                    "No mods found in this folder.",
                    15,
                    TEXT_MUTED,
                    true,
                ));
            }
            for row in list_view.rows {
                let mut hbox = HBoxContainer::new_alloc();
                hbox.set_h_size_flags(SizeFlags::EXPAND_FILL);
                hbox.add_theme_constant_override("separation", 12);

                let mut info = VBoxContainer::new_alloc();
                info.set_h_size_flags(SizeFlags::EXPAND_FILL);
                info.add_theme_constant_override("separation", 2);

                let prefix = row
                    .load_slot
                    .map_or_else(|| "--".to_owned(), |slot| format!("#{slot}"));
                let version_part = if row.version.is_empty() {
                    String::new()
                } else {
                    format!(" v{}", row.version)
                };
                let order_part = row
                    .order
                    .map(|ord| format!("  (order {ord})"))
                    .unwrap_or_default();
                let title_line = format!(
                    "{prefix}  {}{version_part}  [{}]{order_part}",
                    row.name, row.id
                );
                info.add_child(&styled_label_aligned(
                    &title_line,
                    15,
                    TEXT_WHITE,
                    true,
                    HorizontalAlignment::LEFT,
                ));
                if let Some(desc) = &row.description {
                    let desc_color = if row.status == ModRowStatus::Invalid {
                        TEXT_ERROR
                    } else {
                        TEXT_MUTED
                    };
                    info.add_child(&styled_label_aligned(
                        desc,
                        13,
                        desc_color,
                        true,
                        HorizontalAlignment::LEFT,
                    ));
                }
                hbox.add_child(&info);

                let status_color = match row.status {
                    ModRowStatus::Active => TEXT_GREEN,
                    ModRowStatus::EnabledPendingReload => TEXT_CYAN,
                    ModRowStatus::Disabled | ModRowStatus::DisabledPendingReload => TEXT_MUTED,
                    ModRowStatus::Bypassed => TEXT_GOLD,
                    ModRowStatus::Invalid => TEXT_ERROR,
                };
                hbox.add_child(&styled_label(
                    row.status.badge_text(),
                    13,
                    status_color,
                    false,
                ));

                if let (Some(mod_dir), Some(next_enabled)) = (row.toggle_dir, row.next_enabled) {
                    let mut toggle = Button::new_alloc();
                    toggle.set_text(if next_enabled { "Enable" } else { "Disable" });
                    style_compact_button(&mut toggle);
                    let mut owner_clone = owner.clone();
                    toggle.signals().pressed().connect(move || {
                        owner_clone
                            .bind_mut()
                            .queue_toggle(mod_dir.clone(), next_enabled);
                    });
                    hbox.add_child(&toggle);
                }

                let row_panel = psiv_compact_panel(&hbox, 12.0, 8.0, BORDER_STEEL);
                rows_box.add_child(&row_panel);
            }
        }
        if let Some(reload) = self.reload_button.as_mut() {
            reload.set_visible(list_view.can_reload);
        }
    }

    fn build_hud_bar(&mut self) {
        let mut margin = MarginContainer::new_alloc();
        margin.set_anchors_preset(LayoutPreset::TOP_WIDE);
        margin.set_mouse_filter(MouseFilter::PASS);
        for (side, px) in [
            ("margin_left", 14),
            ("margin_right", 14),
            ("margin_top", 12),
            ("margin_bottom", 0),
        ] {
            margin.add_theme_constant_override(side, px);
        }

        let mut row = HBoxContainer::new_alloc();
        row.set_mouse_filter(MouseFilter::PASS);
        row.add_theme_constant_override("separation", 10);

        let mut mods_btn = Button::new_alloc();
        mods_btn.set_text(&format_mods_button_label(
            &self.state.active_mods,
            self.state.discovered_count,
            &self.state.source,
        ));
        style_compact_button(&mut mods_btn);
        mods_btn
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::on_open_mods);
        row.add_child(&mods_btn);

        if let Some(text) = format_active_mods_banner(&self.state.active_mods) {
            let label =
                styled_label_aligned(&text, 14, TEXT_GOLD, false, HorizontalAlignment::LEFT);
            let mut badge = psiv_compact_panel(&label, 12.0, 6.0, BORDER_SILVER);
            badge.set_mouse_filter(MouseFilter::IGNORE);
            row.add_child(&badge);
            self.active_banner = Some(badge);
        }

        margin.add_child(&row);
        margin.set_visible(false);
        self.base_mut().add_child(&margin);
        self.hud_bar = Some(margin);
        self.mods_button = Some(mods_btn);
    }

    fn build_save_notice_banner(&mut self) {
        let Some(notice) = self.state.save_notice.as_ref() else {
            return;
        };
        let Some(view) = format_save_profile_notice(notice) else {
            return;
        };

        let mut margin = MarginContainer::new_alloc();
        margin.set_anchors_preset(LayoutPreset::FULL_RECT);
        margin.set_mouse_filter(MouseFilter::IGNORE);
        for (side, px) in [
            ("margin_left", 24),
            ("margin_right", 24),
            ("margin_top", 12),
            ("margin_bottom", 12),
        ] {
            margin.add_theme_constant_override(side, px);
        }

        let mut bottom_box = VBoxContainer::new_alloc();
        bottom_box.set_alignment(AlignmentMode::END);
        bottom_box.set_h_size_flags(SizeFlags::EXPAND_FILL);
        bottom_box.set_v_size_flags(SizeFlags::EXPAND_FILL);
        bottom_box.set_mouse_filter(MouseFilter::IGNORE);

        let mut center = CenterContainer::new_alloc();
        center.set_mouse_filter(MouseFilter::IGNORE);

        let mut hbox = HBoxContainer::new_alloc();
        hbox.add_theme_constant_override("separation", 16);
        hbox.set_alignment(AlignmentMode::CENTER);

        let mut text_col = VBoxContainer::new_alloc();
        text_col.add_theme_constant_override("separation", 3);
        text_col.set_custom_minimum_size(Vector2::new(560.0, 0.0));
        text_col.add_child(&styled_label_aligned(
            &view.title,
            15,
            TEXT_GOLD,
            true,
            HorizontalAlignment::LEFT,
        ));
        text_col.add_child(&styled_label_aligned(
            &view.details.join("  ·  "),
            13,
            TEXT_BODY,
            true,
            HorizontalAlignment::LEFT,
        ));
        hbox.add_child(&text_col);

        let mut ok_btn = Button::new_alloc();
        ok_btn.set_text("OK");
        style_compact_button(&mut ok_btn);
        ok_btn
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::on_dismiss_notice);
        hbox.add_child(&ok_btn);

        let panel = psiv_compact_panel(&hbox, 16.0, 8.0, TEXT_GOLD);
        center.add_child(&panel);
        bottom_box.add_child(&center);
        margin.add_child(&bottom_box);
        margin.set_visible(false);
        self.base_mut().add_child(&margin);
        self.notice_container = Some(margin);
    }

    fn build_modal_panel(&mut self) {
        let mut backdrop = ColorRect::new_alloc();
        backdrop.set_anchors_preset(LayoutPreset::FULL_RECT);
        backdrop.set_color(Color::from_rgba(0.0, 0.02, 0.08, 0.78));
        backdrop.set_visible(false);

        let mut center = CenterContainer::new_alloc();
        center.set_anchors_preset(LayoutPreset::FULL_RECT);

        let mut col = VBoxContainer::new_alloc();
        col.set_custom_minimum_size(Vector2::new(720.0, 0.0));
        col.add_theme_constant_override("separation", 12);
        col.add_child(&styled_label("INSTALLED MODS", 22, TEXT_GOLD, false));
        col.add_child(&divider());

        let mut scroll = ScrollContainer::new_alloc();
        scroll.set_custom_minimum_size(Vector2::new(700.0, 280.0));
        scroll.set_h_size_flags(SizeFlags::EXPAND_FILL);

        let mut rows_box = VBoxContainer::new_alloc();
        rows_box.set_h_size_flags(SizeFlags::EXPAND_FILL);
        rows_box.add_theme_constant_override("separation", 8);
        scroll.add_child(&rows_box);
        col.add_child(&scroll);

        let mut status = styled_label("", 14, TEXT_CYAN, true);
        status.set_visible(false);
        col.add_child(&status);

        let mut footer = HBoxContainer::new_alloc();
        footer.set_alignment(AlignmentMode::CENTER);
        footer.add_theme_constant_override("separation", 16);

        let mut reload = Button::new_alloc();
        reload.set_text("Apply & Reload");
        style_button(&mut reload);
        reload.set_visible(false);
        reload
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::on_apply_and_reload);
        footer.add_child(&reload);

        let mut close = Button::new_alloc();
        close.set_text("Close");
        style_button(&mut close);
        close
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::close_modal);
        footer.add_child(&close);

        col.add_child(&footer);

        // Give the modal container a neutral Control wrapper so anchors fill cleanly.
        let panel = psiv_window(&col, 28.0, 20.0);
        let mut holder = Control::new_alloc();
        holder.set_anchors_preset(LayoutPreset::FULL_RECT);
        center.add_child(&panel);
        holder.add_child(&center);
        backdrop.add_child(&holder);
        self.base_mut().add_child(&backdrop);

        self.modal_backdrop = Some(backdrop);
        self.modal_rows = Some(rows_box);
        self.modal_status = Some(status);
        self.reload_button = Some(reload);
        self.close_button = Some(close);
    }
}

#[godot_api]
impl ICanvasLayer for TitleModsOverlay {
    fn ready(&mut self) {
        self.base_mut().set_layer(100);
        self.build_hud_bar();
        self.build_save_notice_banner();
        self.build_modal_panel();
        self.refresh_visibility();
    }

    fn process(&mut self, _delta: f64) {
        self.apply_pending_action();

        let vis = title_hud_visibility(
            self.phase,
            &self.state.active_mods,
            self.state.discovered_count,
            &self.state.source,
            self.state.save_notice.as_ref(),
            self.notice_dismissed,
        );
        let input = Input::singleton();
        let m_down = input.is_physical_key_pressed(Key::M);
        let m_pressed = m_down && !self.m_key_down;
        self.m_key_down = m_down;

        let esc_down = input.is_physical_key_pressed(Key::ESCAPE);
        let esc_pressed = esc_down && !self.esc_key_down;
        self.esc_key_down = esc_down;

        if vis.show_mods_button && m_pressed {
            self.on_open_mods();
        } else if self.modal_open && esc_pressed {
            self.close_modal();
        }
    }
}
