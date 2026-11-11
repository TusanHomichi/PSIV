//! `battle/`: the port of `psiv_tools/battle_pack.py`, which owns the reasons
//! for each file's shape (#216, slice 6). This slice writes the enemies, the
//! formations, the level tables, the abilities, the characters with their
//! STATUS portraits, the equipment, the enemy animations and the first of the
//! battle art; the rest of `battle/art/` follows.

mod abilities;
pub mod animations;
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
pub fn build(rom: &Rom, root: &Path) -> Result<Json, PackError> {
    let data = rom.bytes();
    let decodes = Decodes::new(data);
    let display = names::display_names(data)?;
    let enemy_names = display.get("enemies").cloned().unwrap_or_default();
    let mut written = Written::new(root);
    written.write(
        "enemies",
        "battle/enemies.json",
        enemies::build(data, &enemy_names)?,
    )?;
    let enemy_animations = animations::build(data, &enemy_names)?;
    written.write(
        "enemy_animations",
        "battle/enemy_animations.json",
        enemy_animations.clone(),
    )?;
    written.write(
        "abilities",
        "battle/abilities.json",
        abilities::build(data, &display)?,
    )?;
    written.write(
        "formations",
        "battle/formations.json",
        formations::build(data, &decodes)?,
    )?;
    written.write("levels", "battle/levels.json", levels::build(data)?)?;

    let rules = rules::read_rules(data)?;
    let types = rules::type_rules(&rules)?;
    let items = equipment::read_items(data)?;
    let empty = names::Display::new();
    let table = |key: &str| display.get(key).unwrap_or(&empty);
    written.write(
        "equipment",
        "battle/equipment.json",
        equipment::build(&items, &rules, &types, table("items"))?,
    )?;
    let init = rules::read_char_init(data)?;
    let seated = status_portraits::write(data, root)?
        .into_iter()
        .enumerate()
        .map(|(id, portrait)| Ok((status_portraits::age(data, id)?, portrait)))
        .collect::<Result<Vec<_>, PackError>>()?;
    let portrait_count = seated.len();
    let names = characters::Names {
        characters: table("characters"),
        professions: table("professions"),
        techniques: table("techniques"),
        skills: table("skills"),
        items: table("items"),
    };
    written.write(
        "characters",
        "battle/characters.json",
        characters::build(data, &init, &rules, &types, &items, &names, &seated)?,
    )?;
    let art = art::build(data, root, &enemy_animations)?;
    let mut fragment = written.fragment(portrait_count)?;
    fragment.insert("art", art);
    Ok(Json::object().with("battle", fragment))
}

/// The record files as written, for `emit_battle`'s manifest fragment.
struct Written<'a> {
    root: &'a Path,
    files: Vec<(&'static str, &'static str, Json, String)>,
}

impl<'a> Written<'a> {
    fn new(root: &'a Path) -> Self {
        Written {
            root,
            files: Vec::new(),
        }
    }

    fn write(
        &mut self,
        key: &'static str,
        path: &'static str,
        value: Json,
    ) -> Result<(), PackError> {
        let sha = crate::write_json(self.root, path, &value)?;
        self.files.push((key, path, value, sha));
        Ok(())
    }

    fn get(&self, key: &str) -> Result<(&'static str, &Json, &str), PackError> {
        self.files
            .iter()
            .find(|(k, ..)| *k == key)
            .map(|(_, path, value, sha)| (*path, value, sha.as_str()))
            .ok_or_else(|| PackError(format!("battle file {key} was not written")))
    }

    /// The file's name and digest, with the named values beside them.
    fn entry(&self, key: &str, fields: &[(&str, &[&str])]) -> Result<Json, PackError> {
        let (path, value, sha) = self.get(key)?;
        let mut entry = Json::object().with("file", path).with("sha256", sha);
        for (name, at) in fields {
            entry.insert(*name, value.at(at)?.clone());
        }
        Ok(entry)
    }

    /// `emit_battle`'s return, less the art subtree.
    fn fragment(&self, portrait_count: usize) -> Result<Json, PackError> {
        let (_, formations, _) = self.get("formations")?;
        let (_, abilities, _) = self.get("abilities")?;
        let (_, equipment, _) = self.get("equipment")?;
        let mut ability_entry = self.entry("abilities", &[])?;
        crate::json::overlay(&mut ability_entry, abilities.at(&["counts"])?);
        let rules = equipment.at(&["rules"])?;
        let mut routines = Json::object();
        for name in ["equip", "attack", "derived_stats", "elements"] {
            routines.insert(name, rules.at(&[name, "routine"])?.clone());
        }
        let derived = rules
            .at(&["derived_stats", "passes"])?
            .items()?
            .iter()
            .map(|pass| pass.at(&["stat"]).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        let files = Json::object()
            .with("enemies", self.entry("enemies", &[("count", &["count"])])?)
            .with(
                "formations",
                self.entry(
                    "formations",
                    &[
                        ("count", &["formation_count"]),
                        ("boss_count", &["boss_formation_count"]),
                        ("encounter_groups", &["encounter_groups", "group_count"]),
                    ],
                )?
                .with(
                    "inline_count",
                    formations.at(&["inline_formations"])?.items()?.len(),
                ),
            )
            .with(
                "levels",
                self.entry(
                    "levels",
                    &[
                        ("characters", &["character_count"]),
                        ("records", &["total_records"]),
                    ],
                )?,
            )
            .with("abilities", ability_entry)
            .with(
                "characters",
                self.entry("characters", &[("count", &["count"])])?
                    .with("portrait_png_count", portrait_count),
            )
            .with(
                "equipment",
                self.entry(
                    "equipment",
                    &[
                        ("count", &["count"]),
                        ("equippable", &["census", "equippable"]),
                    ],
                )?,
            )
            .with(
                "enemy_animations",
                self.entry(
                    "enemy_animations",
                    &[
                        ("count", &["count"]),
                        ("exact_sfx", &["census", "exact_sfx"]),
                        ("generic_sfx", &["census", "generic_sfx"]),
                    ],
                )?,
            );
        let mut census = Json::object();
        for key in [
            "enemies",
            "formations",
            "levels",
            "abilities",
            "characters",
            "equipment",
            "enemy_animations",
        ] {
            census.insert(key, self.get(key)?.1.at(&["census"])?.clone());
        }
        Ok(Json::object()
            .with("directory", "battle")
            .with("files", files)
            .with("ability_effects", abilities.at(&["effects"])?.clone())
            .with(
                "equipment_rules",
                Json::object()
                    .with("routines", routines)
                    .with(
                        "max_equippable_type",
                        rules.at(&["equip", "max_equippable_type"])?.clone(),
                    )
                    .with(
                        "weapon_types",
                        rules.at(&["attack", "weapon_types"])?.clone(),
                    )
                    .with(
                        "multi_target_types",
                        rules.at(&["attack", "multi_target_types"])?.clone(),
                    )
                    .with("derived_stats", Json::Array(derived)),
            )
            .with("census", census))
    }
}
