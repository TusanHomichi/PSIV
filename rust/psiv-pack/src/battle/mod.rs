//! `battle/`: the port of `psiv_tools/battle_pack.py`, which owns the reasons
//! for each file's shape (#216, slice 6). This slice writes the enemies, the
//! formations, the level tables, the abilities, the characters with their
//! STATUS portraits, the equipment, the enemy animations and the first of the
//! battle art; the rest of `battle/art/` follows.

mod abilities;
mod animations;
mod art;
mod characters;
mod enemies;
mod equipment;
mod formations;
mod levels;
mod names;
mod rules;
mod status_portraits;

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

/// `battle/enemies.json`, `battle/formations.json`, `battle/levels.json`,
/// `battle/abilities.json`, `battle/characters.json` with
/// `battle/status_portraits/`, `battle/equipment.json` and
/// `battle/enemy_animations.json`, and [`art`]'s part of `battle/art/`.
pub fn build(rom: &Rom, root: &Path) -> Result<(), PackError> {
    let data = rom.bytes();
    let decodes = Decodes::new(data);
    let display = names::display_names(data)?;
    let enemy_names = display.get("enemies").cloned().unwrap_or_default();
    crate::write_json(
        root,
        "battle/enemies.json",
        &enemies::build(data, &enemy_names)?,
    )?;
    crate::write_json(
        root,
        "battle/enemy_animations.json",
        &animations::build(data, &enemy_names)?,
    )?;
    crate::write_json(
        root,
        "battle/abilities.json",
        &abilities::build(data, &display)?,
    )?;
    crate::write_json(
        root,
        "battle/formations.json",
        &formations::build(data, &decodes)?,
    )?;
    crate::write_json(root, "battle/levels.json", &levels::build(data)?)?;

    let rules = rules::read_rules(data)?;
    let types = rules::type_rules(&rules)?;
    let items = equipment::read_items(data)?;
    let empty = names::Display::new();
    let table = |key: &str| display.get(key).unwrap_or(&empty);
    crate::write_json(
        root,
        "battle/equipment.json",
        &equipment::build(&items, &rules, &types, table("items"))?,
    )?;
    let init = rules::read_char_init(data)?;
    let seated = status_portraits::write(data, root)?
        .into_iter()
        .enumerate()
        .map(|(id, portrait)| Ok((status_portraits::age(data, id)?, portrait)))
        .collect::<Result<Vec<_>, PackError>>()?;
    let names = characters::Names {
        characters: table("characters"),
        professions: table("professions"),
        techniques: table("techniques"),
        skills: table("skills"),
        items: table("items"),
    };
    crate::write_json(
        root,
        "battle/characters.json",
        &characters::build(data, &init, &rules, &types, &items, &names, &seated)?,
    )?;
    art::build(data, root)
}
