//! In-game mod UI: startup error screen, title-screen active mods indicator,
//! lightweight Mods manager panel, and save-profile mismatch notices (#785).
//!
//! Pure state, visibility rules, formatting, and persistence helpers live here
//! so they are directly covered by Rust unit tests without requiring Godot.
//! Godot `CanvasLayer` presentation lives in [`error_screen`] and
//! [`title_overlay`], reusing the procedural PSIV chrome in
//! [`crate::pack_setup_style`].

mod error_screen;
#[cfg(test)]
mod tests;
mod title_overlay;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use godot::prelude::*;
use psiv_data::mods::{
    ModError, ModInventoryEntry, ModManifest, find_mod_dir_by_id, inspect_mods_dir, load_mod,
    set_mod_enabled,
};
use psiv_runtime::{TitlePhase, TitleView};

use crate::Field;
use crate::install::{IgnoredModSource, Install, ModBootResult, ModSourceMode};
use crate::save_dir::SaveProfileNotice;

pub(crate) use error_screen::ModErrorScreen;
pub(crate) use title_overlay::TitleModsOverlay;

/// Maximum active mod names listed inline on the title-screen corner badge
/// before summarizing the rest as `(+N more)`.
const MAX_INLINE_ACTIVE_MODS: usize = 3;

/// Writable folder target that the error screen can disable in `mod.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DisableTarget {
    pub(crate) mod_id: String,
    pub(crate) mod_dir: PathBuf,
}

/// Pure view model for the on-screen mod load error screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModErrorView {
    pub(crate) headline: String,
    pub(crate) subtitle: String,
    pub(crate) mod_label: String,
    pub(crate) file_label: String,
    pub(crate) detail: String,
    pub(crate) disable_target: Option<DisableTarget>,
    pub(crate) disable_button_label: Option<String>,
    pub(crate) recovery_hint: String,
}

impl ModErrorView {
    /// Builds the structured error view from a failed mod boot.
    pub(crate) fn from_error(source: &ModSourceMode, error: &ModError) -> Self {
        let disable_target = match source {
            ModSourceMode::Directory { mods_dir } => find_mod_dir_by_id(mods_dir, &error.mod_id)
                .map(|mod_dir| DisableTarget {
                    mod_id: error.mod_id.clone(),
                    mod_dir,
                }),
            ModSourceMode::ExplicitPaths { .. }
            | ModSourceMode::DisabledByEnv { .. }
            | ModSourceMode::IgnoredForSession { .. } => None,
        };
        let disable_button_label = disable_target
            .as_ref()
            .map(|target| format!("Disable `{}` & Retry", target.mod_id));
        let recovery_hint = match source {
            ModSourceMode::Directory { mods_dir } => format!(
                "Fix or remove this mod under {}, set \"enabled\": false in its mod.json, \
                 or launch unmodified with PSIV_MODS=none.",
                mods_dir.display()
            ),
            ModSourceMode::ExplicitPaths { .. } => {
                "Mods were loaded via PSIV_MODS. Fix the listed file, unset PSIV_MODS, \
                 or launch with PSIV_MODS=none (or choose Launch Unmodified below)."
                    .to_owned()
            }
            ModSourceMode::DisabledByEnv { .. } | ModSourceMode::IgnoredForSession { .. } => {
                "Launch unmodified below or retry after fixing your mods folder.".to_owned()
            }
        };
        Self {
            headline: "Mod failed to load".to_owned(),
            subtitle: "MOD LOADER  ·  STARTUP ERROR".to_owned(),
            mod_label: error.mod_id.clone(),
            file_label: error.file.clone(),
            detail: error.detail.clone(),
            disable_target,
            disable_button_label,
            recovery_hint,
        }
    }
}

/// Which title-screen HUD elements should be visible on a given title frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct TitleHudVisibility {
    pub(crate) show_active_mods_banner: bool,
    pub(crate) show_mods_button: bool,
    pub(crate) show_save_notice: bool,
}

/// Whether `phase` is an interactive title phase where mod HUD elements may
/// appear (never over the Sega logo, initial reveal animation, or Sound Test).
pub(crate) fn is_interactive_title_phase(phase: TitlePhase) -> bool {
    matches!(
        phase,
        TitlePhase::PressStart
            | TitlePhase::Menu
            | TitlePhase::Slots
            | TitlePhase::EraseSlots
            | TitlePhase::EraseConfirm
    )
}

/// Pure visibility rules for the title-screen mod indicator, Mods button, and
/// save-profile mismatch notice.
///
/// Guarantees 100% vanilla parity: when no mods are active, no mods exist in
/// `mods/` (or `PSIV_MODS=none` is set), and there is no save-profile mismatch
/// notice, every flag is `false`.
pub(crate) fn title_hud_visibility(
    phase: TitlePhase,
    active_mods: &[ModManifest],
    discovered_count: usize,
    source: &ModSourceMode,
    save_notice: Option<&SaveProfileNotice>,
    notice_dismissed: bool,
) -> TitleHudVisibility {
    if !is_interactive_title_phase(phase) {
        return TitleHudVisibility::default();
    }
    let show_save_notice = save_notice.is_some() && !notice_dismissed;
    if matches!(source, ModSourceMode::DisabledByEnv { .. }) {
        return TitleHudVisibility {
            show_active_mods_banner: false,
            show_mods_button: false,
            show_save_notice,
        };
    }
    let show_active_mods_banner = !active_mods.is_empty();
    let show_mods_button = !active_mods.is_empty()
        || discovered_count > 0
        || matches!(source, ModSourceMode::IgnoredForSession { .. });
    TitleHudVisibility {
        show_active_mods_banner,
        show_mods_button,
        show_save_notice,
    }
}

/// Concise title-screen active-mod indicator text in load order.
/// Returns `None` on vanilla runs (`active_mods.is_empty()`).
pub(crate) fn format_active_mods_banner(active_mods: &[ModManifest]) -> Option<String> {
    if active_mods.is_empty() {
        return None;
    }
    let listed: Vec<String> = active_mods
        .iter()
        .take(MAX_INLINE_ACTIVE_MODS)
        .map(|m| format!("{} v{}", m.name, m.version))
        .collect();
    let overflow = active_mods.len().saturating_sub(MAX_INLINE_ACTIVE_MODS);
    let summary = if overflow > 0 {
        format!("{} (+{overflow} more)", listed.join(", "))
    } else {
        listed.join(", ")
    };
    let count = active_mods.len();
    let noun = if count == 1 { "MOD" } else { "MODS" };
    Some(format!("{count} {noun} ACTIVE: {summary}"))
}

/// Compact label for the title-screen Mods manager button.
pub(crate) fn format_mods_button_label(
    active_mods: &[ModManifest],
    discovered_count: usize,
    source: &ModSourceMode,
) -> String {
    match source {
        ModSourceMode::IgnoredForSession { .. } => "Mods (bypassed) [M]".to_owned(),
        _ if !active_mods.is_empty() => format!("Mods ({}) [M]", active_mods.len()),
        _ if discovered_count > 0 => format!("Mods (0/{discovered_count}) [M]"),
        _ => "Mods [M]".to_owned(),
    }
}

/// Formatted in-game notice for a `mod_profile.json` mismatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SaveNoticeView {
    pub(crate) title: String,
    pub(crate) details: Vec<String>,
}

/// Formats a [`SaveProfileNotice`] for the in-game title/continue banner.
/// Returns `None` for an empty diff (negative control).
pub(crate) fn format_save_profile_notice(notice: &SaveProfileNotice) -> Option<SaveNoticeView> {
    match notice {
        SaveProfileNotice::Diff(diff) => {
            if diff.is_empty() {
                return None;
            }
            let mut details = Vec::new();
            if !diff.added.is_empty() {
                details.push(format!("Added: {}", diff.added.join(", ")));
            }
            if !diff.removed.is_empty() {
                details.push(format!("Removed: {}", diff.removed.join(", ")));
            }
            if diff.order_changed {
                details.push("Load order changed".to_owned());
            }
            if !diff.version_changes.is_empty() {
                let versions: Vec<String> = diff
                    .version_changes
                    .iter()
                    .map(|(id, prev, curr)| format!("{id} ({prev} -> {curr})"))
                    .collect();
                details.push(format!("Updated: {}", versions.join(", ")));
            }
            Some(SaveNoticeView {
                title: "Save profile differs from active mods".to_owned(),
                details,
            })
        }
        SaveProfileNotice::Unreadable(error) => Some(SaveNoticeView {
            title: "Save mod profile could not be read".to_owned(),
            details: vec![error.clone()],
        }),
    }
}

/// Display status of one row in the in-game Mods panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModRowStatus {
    Active,
    EnabledPendingReload,
    Disabled,
    DisabledPendingReload,
    Bypassed,
    Invalid,
}

impl ModRowStatus {
    pub(crate) fn badge_text(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::EnabledPendingReload => "ENABLED (reload)",
            Self::Disabled => "DISABLED",
            Self::DisabledPendingReload => "DISABLED (reload)",
            Self::Bypassed => "BYPASSED",
            Self::Invalid => "INVALID",
        }
    }
}

/// One row shown in the in-game Mods manager panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModListRow {
    pub(crate) load_slot: Option<usize>,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) order: Option<i32>,
    pub(crate) description: Option<String>,
    pub(crate) status: ModRowStatus,
    /// Folder path when this row can be toggled in `mod.json`.
    pub(crate) toggle_dir: Option<PathBuf>,
    /// Target `enabled` value if the user clicks the row's toggle button.
    pub(crate) next_enabled: Option<bool>,
}

/// Complete view model for the in-game Mods panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModListView {
    pub(crate) source_summary: String,
    pub(crate) read_only_note: Option<String>,
    pub(crate) rows: Vec<ModListRow>,
    pub(crate) can_reload: bool,
}

/// Builds the in-game Mods panel rows from the active mod source and disk state.
pub(crate) fn build_mod_list(
    source: &ModSourceMode,
    active_mods: &[ModManifest],
    pending_reload: bool,
) -> ModListView {
    let active_ids: BTreeSet<&str> = active_mods.iter().map(|m| m.id.as_str()).collect();
    match source {
        ModSourceMode::Directory { mods_dir }
        | ModSourceMode::IgnoredForSession {
            underlying: IgnoredModSource::Directory { mods_dir },
        } => {
            let bypassed = matches!(source, ModSourceMode::IgnoredForSession { .. });
            let source_summary = if bypassed {
                format!("Mods bypassed for this session  ·  {}", mods_dir.display())
            } else {
                format!("Folder: {}", mods_dir.display())
            };
            let mut rows = Vec::new();
            let mut next_slot = 1usize;
            match inspect_mods_dir(mods_dir) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            ModInventoryEntry::Valid(discovered) => {
                                let m = discovered.manifest;
                                let is_active = active_ids.contains(m.id.as_str());
                                let load_slot = if m.enabled {
                                    let slot = next_slot;
                                    next_slot += 1;
                                    Some(slot)
                                } else {
                                    None
                                };
                                let status = if bypassed && m.enabled {
                                    ModRowStatus::Bypassed
                                } else if m.enabled && is_active {
                                    ModRowStatus::Active
                                } else if m.enabled {
                                    ModRowStatus::EnabledPendingReload
                                } else if is_active {
                                    ModRowStatus::DisabledPendingReload
                                } else {
                                    ModRowStatus::Disabled
                                };
                                rows.push(ModListRow {
                                    load_slot,
                                    id: m.id,
                                    name: m.name,
                                    version: m.version,
                                    order: Some(m.order),
                                    description: m.description,
                                    status,
                                    toggle_dir: Some(discovered.root),
                                    next_enabled: Some(!m.enabled),
                                });
                            }
                            ModInventoryEntry::Invalid { root, error } => {
                                let folder_name = root
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| root.display().to_string());
                                rows.push(ModListRow {
                                    load_slot: None,
                                    id: error.mod_id,
                                    name: folder_name,
                                    version: String::new(),
                                    order: None,
                                    description: Some(error.detail),
                                    status: ModRowStatus::Invalid,
                                    toggle_dir: None,
                                    next_enabled: None,
                                });
                            }
                        }
                    }
                }
                Err(error) => {
                    rows.push(ModListRow {
                        load_slot: None,
                        id: error.mod_id,
                        name: "mods/".to_owned(),
                        version: String::new(),
                        order: None,
                        description: Some(error.detail),
                        status: ModRowStatus::Invalid,
                        toggle_dir: None,
                        next_enabled: None,
                    });
                }
            }
            ModListView {
                source_summary,
                read_only_note: None,
                rows,
                can_reload: pending_reload || bypassed,
            }
        }
        ModSourceMode::ExplicitPaths { paths } => {
            let rows = active_mods
                .iter()
                .enumerate()
                .map(|(idx, m)| ModListRow {
                    load_slot: Some(idx + 1),
                    id: m.id.clone(),
                    name: m.name.clone(),
                    version: m.version.clone(),
                    order: Some(m.order),
                    description: m.description.clone(),
                    status: ModRowStatus::Active,
                    toggle_dir: None,
                    next_enabled: None,
                })
                .collect();
            ModListView {
                source_summary: format!("PSIV_MODS override ({} path(s))", paths.len()),
                read_only_note: Some(
                    "Loaded from PSIV_MODS (read-only). Unset PSIV_MODS to toggle mods in-game."
                        .to_owned(),
                ),
                rows,
                can_reload: false,
            }
        }
        ModSourceMode::IgnoredForSession {
            underlying: IgnoredModSource::ExplicitPaths { paths },
        } => {
            let rows = paths
                .iter()
                .enumerate()
                .map(|(idx, path)| match load_mod(path) {
                    Ok(discovered) => {
                        let m = discovered.manifest;
                        ModListRow {
                            load_slot: Some(idx + 1),
                            id: m.id,
                            name: m.name,
                            version: m.version,
                            order: Some(m.order),
                            description: m.description,
                            status: ModRowStatus::Bypassed,
                            toggle_dir: None,
                            next_enabled: None,
                        }
                    }
                    Err(error) => {
                        let folder_name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.display().to_string());
                        ModListRow {
                            load_slot: None,
                            id: error.mod_id,
                            name: folder_name,
                            version: String::new(),
                            order: None,
                            description: Some(error.detail),
                            status: ModRowStatus::Invalid,
                            toggle_dir: None,
                            next_enabled: None,
                        }
                    }
                })
                .collect();
            ModListView {
                source_summary: format!(
                    "PSIV_MODS override bypassed for this session ({} path(s))",
                    paths.len()
                ),
                read_only_note: Some(
                    "Loaded from PSIV_MODS (read-only). Fix the listed paths or unset PSIV_MODS, then reload."
                        .to_owned(),
                ),
                rows,
                can_reload: true,
            }
        }
        ModSourceMode::DisabledByEnv { raw } => ModListView {
            source_summary: format!("Mods disabled by PSIV_MODS={raw}"),
            read_only_note: Some(
                "Unset PSIV_MODS to discover and toggle mods from your mods/ folder.".to_owned(),
            ),
            rows: Vec::new(),
            can_reload: false,
        },
    }
}

/// Persists an `enabled` toggle for `mod_dir` when the current mod source is
/// writable (directory mode, including directory session bypass), refusing
/// read-only `PSIV_MODS` overrides even when bypassed.
pub(crate) fn toggle_mod_in_source(
    source: &ModSourceMode,
    mod_dir: &Path,
    enabled: bool,
) -> Result<ModManifest, String> {
    match source {
        ModSourceMode::Directory { .. }
        | ModSourceMode::IgnoredForSession {
            underlying: IgnoredModSource::Directory { .. },
        } => set_mod_enabled(mod_dir, enabled).map_err(|err| err.to_string()),
        ModSourceMode::ExplicitPaths { .. }
        | ModSourceMode::IgnoredForSession {
            underlying: IgnoredModSource::ExplicitPaths { .. },
        } => Err(
            "Cannot toggle mods while PSIV_MODS specifies explicit paths (read-only override)."
                .to_owned(),
        ),
        ModSourceMode::DisabledByEnv { raw } => {
            Err(format!("Cannot toggle mods while PSIV_MODS={raw} is set."))
        }
    }
}

/// Counts discovered mod folders under `source` (0 if `mods/` is absent or
/// disabled via `PSIV_MODS=none`).
pub(crate) fn count_discovered_mods(source: &ModSourceMode) -> usize {
    match source {
        ModSourceMode::Directory { mods_dir }
        | ModSourceMode::IgnoredForSession {
            underlying: IgnoredModSource::Directory { mods_dir },
        } => inspect_mods_dir(mods_dir).map_or(0, |entries| entries.len()),
        ModSourceMode::ExplicitPaths { paths }
        | ModSourceMode::IgnoredForSession {
            underlying: IgnoredModSource::ExplicitPaths { paths },
        } => paths.len(),
        ModSourceMode::DisabledByEnv { .. } => 0,
    }
}

/// Mod presentation context retained by `Field` for the title screen.
#[derive(Clone, Debug)]
pub(crate) struct ModTitleState {
    pub(crate) source: ModSourceMode,
    pub(crate) active_mods: Vec<ModManifest>,
    pub(crate) discovered_count: usize,
    pub(crate) save_notice: Option<SaveProfileNotice>,
}

impl ModTitleState {
    pub(crate) fn new(
        source: ModSourceMode,
        active_mods: Vec<ModManifest>,
        save_notice: Option<SaveProfileNotice>,
    ) -> Self {
        let discovered_count = count_discovered_mods(&source);
        Self {
            source,
            active_mods,
            discovered_count,
            save_notice,
        }
    }

    /// True iff any title-screen mod HUD or save notice could ever be shown.
    /// When false (all pure vanilla runs), `Field` allocates zero overlay nodes.
    pub(crate) fn needs_title_overlay(&self) -> bool {
        let vis = title_hud_visibility(
            TitlePhase::Menu,
            &self.active_mods,
            self.discovered_count,
            &self.source,
            self.save_notice.as_ref(),
            false,
        );
        vis.show_active_mods_banner || vis.show_mods_button || vis.show_save_notice
    }
}

impl Field {
    /// Applies active mods or mounts [`ModErrorScreen`] on failure so a broken
    /// mod never leaves a blank screen.
    pub(crate) fn apply_mods_or_show_error(
        &mut self,
        install: &Install,
        mut pack: PathBuf,
    ) -> Option<(PathBuf, ModBootResult)> {
        match crate::install::apply_active_mods(install, &pack) {
            Ok(boot) => {
                if let Some(applied) = &boot.applied {
                    for m in &applied.active {
                        godot_print!("loaded mod: {} ({} v{})", m.name, m.id, m.version);
                    }
                    pack = applied.effective_pack.clone();
                    self.pack_dir = pack.to_string_lossy().into_owned();
                }
                Some((pack, boot))
            }
            Err((source, error)) => {
                godot_error!("mod load failed: {error}");
                let screen = ModErrorScreen::create(source, error);
                self.base_mut().add_child(&screen);
                None
            }
        }
    }

    /// Ensures the title mod HUD overlay exists when modded or noticed, and
    /// syncs it with the current [`TitleView`]. Never allocates nodes on vanilla runs.
    pub(crate) fn sync_title_mods(&mut self, view: &TitleView) {
        let Some(state) = self.mod_title_state.as_ref() else {
            return;
        };
        if !state.needs_title_overlay() {
            return;
        }
        if self.title_mods.is_none() {
            let overlay = TitleModsOverlay::create(state.clone());
            self.base_mut().add_child(&overlay);
            self.title_mods = Some(overlay);
        }
        if let Some(overlay) = self.title_mods.as_mut() {
            overlay.bind_mut().sync_phase(view.phase);
        }
    }

    /// Whether the in-game Mods modal panel currently owns keyboard/pad input.
    pub(crate) fn title_mods_modal_open(&self) -> bool {
        self.title_mods
            .as_ref()
            .is_some_and(|overlay| overlay.bind().is_modal_open())
    }

    /// Tears down the title mod overlay when leaving the title for gameplay.
    pub(crate) fn dispose_title_mods(&mut self) {
        if let Some(mut overlay) = self.title_mods.take() {
            overlay.queue_free();
        }
    }
}
