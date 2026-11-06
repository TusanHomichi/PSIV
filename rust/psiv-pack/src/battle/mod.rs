//! `battle/`: the port of `psiv_tools/battle_pack.py`, which owns the reasons
//! for each file's shape (#216, slice 6). This slice writes the formations
//! and the level tables; the rest of the directory follows.

mod formations;
mod levels;

use std::path::Path;

use crate::decode::Decodes;
use crate::json::Json;
use crate::{PackError, Rom};

/// `PACK_FORMAT_VERSION`: one pack is one version.
const FORMAT_VERSION: u32 = 2;

/// `_range`: `{"min": .., "max": ..}`, null for no values.
fn range(values: impl Iterator<Item = u64> + Clone) -> Json {
    Json::object()
        .with("min", values.clone().min())
        .with("max", values.max())
}

fn serialize<T: serde::Serialize>(value: &T, what: &str) -> Result<Json, PackError> {
    serde_json::to_value(value)
        .map(|value| Json::from(&value))
        .map_err(|error| PackError(format!("cannot serialize {what}: {error}")))
}

/// `battle/formations.json` and `battle/levels.json`.
pub fn build(rom: &Rom, root: &Path) -> Result<(), PackError> {
    let data = rom.bytes();
    let decodes = Decodes::new(data);
    crate::write_json(
        root,
        "battle/formations.json",
        &formations::build(data, &decodes)?,
    )?;
    crate::write_json(root, "battle/levels.json", &levels::build(data)?).map(drop)
}
