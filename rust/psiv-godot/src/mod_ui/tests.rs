use super::*;
use psiv_data::mods::{ModManifest, ModProfileDiff, init_mod_dir};
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DIR: AtomicUsize = AtomicUsize::new(1);

fn temp_mods_dir(label: &str) -> PathBuf {
    let n = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("psiv-mod-ui-{label}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn broken_mod_error_view_formats_fields_and_resolves_disable_target() {
    let mods_dir = temp_mods_dir("error-view");
    let bad_dir = mods_dir.join("tough-zoran");
    init_mod_dir(
        &bad_dir,
        &ModManifest::new("tough-zoran", "Tough Zoran", "1.0.0"),
    )
    .unwrap();

    let source = ModSourceMode::Directory {
        mods_dir: mods_dir.clone(),
    };
    let error = ModError::new(
        "tough-zoran",
        "battle/enemies.json",
        "invalid value: integer `70000`, expected u16",
    );
    let view = ModErrorView::from_error(&source, &error);
    assert_eq!(view.mod_label, "tough-zoran");
    assert_eq!(view.file_label, "battle/enemies.json");
    assert!(view.detail.contains("70000"), "{}", view.detail);
    assert_eq!(
        view.disable_target,
        Some(DisableTarget {
            mod_id: "tough-zoran".to_owned(),
            mod_dir: bad_dir.clone(),
        })
    );
    assert_eq!(
        view.disable_button_label.as_deref(),
        Some("Disable `tough-zoran` & Retry")
    );
    assert!(view.recovery_hint.contains("PSIV_MODS=none"));
    assert!(view.recovery_hint.contains(&mods_dir.display().to_string()));

    // Negative control 1: explicit PSIV_MODS override must NOT offer a disable button.
    let explicit = ModSourceMode::ExplicitPaths {
        paths: vec![bad_dir.clone()],
    };
    let explicit_view = ModErrorView::from_error(&explicit, &error);
    assert_eq!(explicit_view.disable_target, None);
    assert_eq!(explicit_view.disable_button_label, None);
    assert!(explicit_view.recovery_hint.contains("PSIV_MODS"));

    // Negative control 2: duplicate mod IDs must NOT guess which folder to disable.
    init_mod_dir(
        &mods_dir.join("tough-zoran-dup"),
        &ModManifest::new("tough-zoran", "Tough Zoran", "1.0.0"),
    )
    .unwrap();
    let dup_view = ModErrorView::from_error(&source, &error);
    assert_eq!(dup_view.disable_target, None);

    // Negative control 3: unparseable mod.json cannot be auto-disabled.
    let syntax_dir = mods_dir.join("syntax-err");
    fs::create_dir_all(&syntax_dir).unwrap();
    fs::write(syntax_dir.join("mod.json"), "{broken").unwrap();
    let syntax_err = ModError::new(
        syntax_dir.display().to_string(),
        "mod.json",
        "invalid JSON: key must be a string",
    );
    let syntax_view = ModErrorView::from_error(&source, &syntax_err);
    assert_eq!(syntax_view.disable_target, None);

    let _ = fs::remove_dir_all(&mods_dir);
}

#[test]
fn vanilla_title_visibility_shows_nothing_while_modded_and_notices_appear() {
    let dir_source = ModSourceMode::Directory {
        mods_dir: PathBuf::from("/tmp/nonexistent-mods"),
    };
    let env_none = ModSourceMode::DisabledByEnv {
        raw: "none".to_owned(),
    };
    let ignored = ModSourceMode::IgnoredForSession {
        mods_dir: PathBuf::from("/tmp/nonexistent-mods"),
    };
    let mod_a = ModManifest::new("hard-mode", "Hard Mode", "1.0.0");

    // Negative control: pure vanilla run has zero overlay elements on every title phase.
    let vanilla_state = ModTitleState::new(dir_source.clone(), Vec::new(), None);
    assert!(!vanilla_state.needs_title_overlay());
    for phase in [
        TitlePhase::Sega,
        TitlePhase::Reveal,
        TitlePhase::PressStart,
        TitlePhase::Menu,
        TitlePhase::Slots,
        TitlePhase::EraseSlots,
        TitlePhase::EraseConfirm,
        TitlePhase::SoundTest,
    ] {
        let vis = title_hud_visibility(phase, &[], 0, &dir_source, None, false);
        assert_eq!(vis, TitleHudVisibility::default(), "phase {phase:?}");
    }

    // Negative control: PSIV_MODS=none stays strictly vanilla even if discovered_count > 0.
    let none_vis = title_hud_visibility(TitlePhase::Menu, &[], 3, &env_none, None, false);
    assert_eq!(none_vis, TitleHudVisibility::default());

    // Active mods show banner + Mods button during interactive phases, never during Sega/Reveal/SoundTest.
    let active_menu = title_hud_visibility(
        TitlePhase::Menu,
        std::slice::from_ref(&mod_a),
        1,
        &dir_source,
        None,
        false,
    );
    assert!(active_menu.show_active_mods_banner);
    assert!(active_menu.show_mods_button);
    assert!(!active_menu.show_save_notice);

    for quiet_phase in [TitlePhase::Sega, TitlePhase::Reveal, TitlePhase::SoundTest] {
        let quiet = title_hud_visibility(
            quiet_phase,
            std::slice::from_ref(&mod_a),
            1,
            &dir_source,
            None,
            false,
        );
        assert_eq!(quiet, TitleHudVisibility::default(), "{quiet_phase:?}");
    }

    // Disabled-only mods in mods/ or session bypass expose the Mods button without an active banner.
    let disabled_only = title_hud_visibility(TitlePhase::Menu, &[], 2, &dir_source, None, false);
    assert!(!disabled_only.show_active_mods_banner);
    assert!(disabled_only.show_mods_button);

    let bypassed = title_hud_visibility(TitlePhase::PressStart, &[], 0, &ignored, None, false);
    assert!(!bypassed.show_active_mods_banner);
    assert!(bypassed.show_mods_button);

    // Save-profile notice is shown until dismissed.
    let notice = SaveProfileNotice::Diff(ModProfileDiff {
        added: vec!["hard-mode".to_owned()],
        ..ModProfileDiff::default()
    });
    let with_notice =
        title_hud_visibility(TitlePhase::Slots, &[], 0, &dir_source, Some(&notice), false);
    assert!(with_notice.show_save_notice);
    let dismissed =
        title_hud_visibility(TitlePhase::Slots, &[], 0, &dir_source, Some(&notice), true);
    assert!(!dismissed.show_save_notice);
}

#[test]
fn active_mods_banner_formats_load_order_and_truncates_gracefully() {
    // Negative control: empty active list produces None.
    assert_eq!(format_active_mods_banner(&[]), None);

    let m1 = ModManifest::new("alpha", "Alpha", "1.0.0");
    let m2 = ModManifest::new("beta", "Beta", "2.1.0");
    let m3 = ModManifest::new("gamma", "Gamma", "0.9.0");
    let m4 = ModManifest::new("delta", "Delta", "3.0.0");

    assert_eq!(
        format_active_mods_banner(std::slice::from_ref(&m1)).as_deref(),
        Some("1 MOD ACTIVE: Alpha v1.0.0")
    );
    assert_eq!(
        format_active_mods_banner(&[m1.clone(), m2.clone()]).as_deref(),
        Some("2 MODS ACTIVE: Alpha v1.0.0, Beta v2.1.0")
    );
    assert_eq!(
        format_active_mods_banner(&[m1, m2, m3, m4]).as_deref(),
        Some("4 MODS ACTIVE: Alpha v1.0.0, Beta v2.1.0, Gamma v0.9.0 (+1 more)")
    );
}

#[test]
fn mod_list_and_toggle_persist_directory_mods_and_refuse_read_only_overrides() {
    let mods_dir = temp_mods_dir("list-toggle");
    let mut m1 = ModManifest::new("alpha", "Alpha", "1.0.0");
    m1.order = 10;
    let mut m2 = ModManifest::new("beta", "Beta", "1.2.0");
    m2.order = 20;
    m2.enabled = false;
    init_mod_dir(&mods_dir.join("alpha"), &m1).unwrap();
    init_mod_dir(&mods_dir.join("beta"), &m2).unwrap();

    fs::create_dir_all(mods_dir.join("broken")).unwrap();
    fs::write(mods_dir.join("broken/mod.json"), "{bad").unwrap();

    let dir_source = ModSourceMode::Directory {
        mods_dir: mods_dir.clone(),
    };
    let list = build_mod_list(&dir_source, std::slice::from_ref(&m1), false);
    assert_eq!(list.rows.len(), 3);
    assert_eq!(list.rows[0].status, ModRowStatus::Active);
    assert_eq!(list.rows[0].load_slot, Some(1));
    assert_eq!(list.rows[0].next_enabled, Some(false));
    assert_eq!(list.rows[1].status, ModRowStatus::Disabled);
    assert_eq!(list.rows[1].load_slot, None);
    assert_eq!(list.rows[1].next_enabled, Some(true));
    assert_eq!(list.rows[2].status, ModRowStatus::Invalid);
    assert_eq!(list.rows[2].toggle_dir, None);
    assert!(!list.can_reload);

    // Toggle beta on and alpha off in mod.json.
    let beta_on = toggle_mod_in_source(&dir_source, &mods_dir.join("beta"), true).unwrap();
    assert!(beta_on.enabled);
    let alpha_off = toggle_mod_in_source(&dir_source, &mods_dir.join("alpha"), false).unwrap();
    assert!(!alpha_off.enabled);

    let after = build_mod_list(&dir_source, std::slice::from_ref(&m1), true);
    assert!(after.can_reload);
    assert_eq!(after.rows[0].id, "alpha");
    assert_eq!(after.rows[0].status, ModRowStatus::DisabledPendingReload);
    assert_eq!(after.rows[1].id, "beta");
    assert_eq!(after.rows[1].status, ModRowStatus::EnabledPendingReload);

    // Negative controls: ExplicitPaths and DisabledByEnv are read-only and refuse toggling.
    let explicit = ModSourceMode::ExplicitPaths {
        paths: vec![mods_dir.join("alpha")],
    };
    let explicit_list = build_mod_list(&explicit, std::slice::from_ref(&m1), false);
    assert!(explicit_list.read_only_note.is_some());
    assert_eq!(explicit_list.rows.len(), 1);
    assert_eq!(explicit_list.rows[0].toggle_dir, None);
    assert!(toggle_mod_in_source(&explicit, &mods_dir.join("alpha"), false).is_err());

    let env_none = ModSourceMode::DisabledByEnv {
        raw: "none".to_owned(),
    };
    assert!(toggle_mod_in_source(&env_none, &mods_dir.join("alpha"), true).is_err());

    let _ = fs::remove_dir_all(&mods_dir);
}

#[test]
fn save_profile_notice_formats_diffs_and_rejects_empty_diff() {
    // Negative control: empty diff produces no notice.
    assert_eq!(
        format_save_profile_notice(&SaveProfileNotice::Diff(ModProfileDiff::default())),
        None
    );

    let diff = SaveProfileNotice::Diff(ModProfileDiff {
        added: vec!["new-mod".to_owned()],
        removed: vec!["old-mod".to_owned()],
        version_changes: vec![("hard-mode".into(), "1.0.0".into(), "1.2.0".into())],
        order_changed: true,
    });
    let view = format_save_profile_notice(&diff).expect("non-empty diff");
    assert!(
        view.title.contains("Save profile differs"),
        "{}",
        view.title
    );
    let joined = view.details.join(" | ");
    assert!(joined.contains("Added: new-mod"), "{joined}");
    assert!(joined.contains("Removed: old-mod"), "{joined}");
    assert!(joined.contains("Load order changed"), "{joined}");
    assert!(joined.contains("hard-mode (1.0.0 -> 1.2.0)"), "{joined}");

    let unreadable =
        format_save_profile_notice(&SaveProfileNotice::Unreadable("invalid JSON".to_owned()))
            .expect("unreadable notice");
    assert!(unreadable.title.contains("could not be read"));
    assert_eq!(unreadable.details, vec!["invalid JSON"]);
}
