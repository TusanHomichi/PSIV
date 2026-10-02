//! Shared test inputs: the local runtime pack, skipped with a clear message
//! when it is absent (it is a git-ignored local asset).

#![allow(dead_code)]

use std::path::Path;
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

/// The loaded pack, or `None` (after saying so) when `runtime-pack` is absent.
pub fn pack() -> Option<&'static Pack> {
    static PACK_CELL: OnceLock<Option<Pack>> = OnceLock::new();
    PACK_CELL
        .get_or_init(|| {
            let dir = Path::new(PACK);
            if !dir.join("manifest.json").is_file() {
                eprintln!("runtime pack not present at {PACK}; skipping");
                return None;
            }
            Some(Pack {
                data: GameData::load(dir).expect("pack loads"),
                battle: BattleFiles::load(dir).expect("battle files load"),
            })
        })
        .as_ref()
}
