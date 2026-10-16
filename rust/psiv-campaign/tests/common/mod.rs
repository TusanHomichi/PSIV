//! Shared test inputs: the local runtime pack, skipped with a clear message
//! when it is absent (it is a git-ignored local asset).

#![allow(dead_code)]

use std::sync::OnceLock;

use psiv_data::{BattleFiles, GameData};

/// The pack directory, relative to this crate.
pub const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

/// The real route this crate ships.
pub const MAIN_ROUTE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/routes/main.json");

pub struct Pack {
    pub data: GameData,
    pub battle: BattleFiles,
}

/// The pack `pack()` reads: `PSIV_RUNTIME_PACK` when set (a scratch pack built
/// from a revision's own extractors), else the repository's `runtime-pack`.
pub fn pack_dir() -> std::path::PathBuf {
    std::env::var_os("PSIV_RUNTIME_PACK")
        .map_or_else(|| std::path::PathBuf::from(PACK), std::path::PathBuf::from)
}

/// The loaded pack, or `None` (after saying so) when it is absent.
pub fn pack() -> Option<&'static Pack> {
    static PACK_CELL: OnceLock<Option<Pack>> = OnceLock::new();
    PACK_CELL
        .get_or_init(|| {
            let dir = pack_dir();
            if !dir.join("manifest.json").is_file() {
                eprintln!("runtime pack not present at {}; skipping", dir.display());
                return None;
            }
            Some(Pack {
                data: GameData::load(&dir).expect("pack loads"),
                battle: BattleFiles::load(&dir).expect("battle files load"),
            })
        })
        .as_ref()
}

/// A command window for fighter `actor` on the actions page, with nothing
/// listed: tests fill in what the case needs.
pub fn menu_window(actor: u8) -> psiv_runtime::CommandMenuView {
    psiv_runtime::CommandMenuView {
        strip: None,
        list: None,
        title: String::new(),
        page: psiv_runtime::MenuPage::Actions,
        rows: Vec::new(),
        cursor: 0,
        actor: Some(actor),
        character: Some(actor - 1),
        party: Vec::new(),
        enemies: vec![6],
        techniques: Vec::new(),
        skills: Vec::new(),
        targets: Vec::new(),
    }
}
