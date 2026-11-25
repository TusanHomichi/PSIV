//! Declarative registry of ID-keyed pack arrays and dynamic JSON maps.
//!
//! Shared between [`super::patch`], [`super::grid`], and the schema/pack guard
//! tests so a field rename cannot silently drift away from the actual pack
//! schema.

/// Pattern matching one or more pack-relative JSON paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilePattern {
    /// Exact pack-relative path (for example `"battle/enemies.json"`).
    Exact(&'static str),
    /// Directory prefix and filename suffix (for example `"maps/"` + `".json"`).
    Prefix {
        /// Leading directory prefix including trailing `/`.
        prefix: &'static str,
        /// Required filename suffix.
        suffix: &'static str,
    },
}

impl FilePattern {
    pub(crate) fn matches(self, rel_file: &str) -> bool {
        match self {
            Self::Exact(exact) => rel_file == exact,
            Self::Prefix { prefix, suffix } => {
                rel_file.starts_with(prefix) && rel_file.ends_with(suffix)
            }
        }
    }
}

/// Declarative specification of one ID-keyed pack table array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyedArraySpec {
    pub file: FilePattern,
    pub path: &'static [&'static str],
    pub key_field: &'static str,
    /// Whether this table supports explicit additive record creation via `"$add"`.
    pub allow_add: bool,
}

impl KeyedArraySpec {
    const fn edit_only(
        file: FilePattern,
        path: &'static [&'static str],
        key_field: &'static str,
    ) -> Self {
        Self {
            file,
            path,
            key_field,
            allow_add: false,
        }
    }

    const fn additive(
        file: FilePattern,
        path: &'static [&'static str],
        key_field: &'static str,
    ) -> Self {
        Self {
            file,
            path,
            key_field,
            allow_add: true,
        }
    }
}

pub(crate) const MAPS_JSON: FilePattern = FilePattern::Prefix {
    prefix: "maps/",
    suffix: ".json",
};

pub(crate) const SPRITES_JSON: FilePattern = FilePattern::Prefix {
    prefix: "sprites/",
    suffix: ".json",
};

/// Every pack table array that supports sparse ID-keyed patching and diffing.
pub(crate) const KEYED_ARRAYS: &[KeyedArraySpec] = &[
    // Battle tables
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/enemies.json"),
        &["enemies"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/equipment.json"),
        &["items"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/equipment.json"),
        &["types"],
        "name",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/characters.json"),
        &["characters"],
        "character_id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/abilities.json"),
        &["techniques"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/abilities.json"),
        &["skills"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/abilities.json"),
        &["enemy_skills"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/abilities.json"),
        &["item_effects"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/abilities.json"),
        &["combos"],
        "id",
    ),
    KeyedArraySpec::additive(
        FilePattern::Exact("battle/formations.json"),
        &["formations"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/formations.json"),
        &["formations", "*", "enemies"],
        "slot",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/formations.json"),
        &["boss_formations"],
        "event_battle_index",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/formations.json"),
        &["boss_formations", "*", "enemies"],
        "slot",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/formations.json"),
        &["inline_formations"],
        "label",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/formations.json"),
        &["inline_formations", "*", "enemies"],
        "slot",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/formations.json"),
        &["encounter_groups", "groups"],
        "group",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/formations.json"),
        &["map_bindings"],
        "map_id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/formations.json"),
        &["position_grids"],
        "name",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/levels.json"),
        &["characters"],
        "character_id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/levels.json"),
        &["characters", "*", "levels"],
        "level",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/enemy_animations.json"),
        &["animations"],
        "enemy_id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/art/backgrounds.json"),
        &["backgrounds"],
        "index",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/art/command_ui.json"),
        &["tiles"],
        "vram_tile",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/art/damage_digits.json"),
        &["tiles"],
        "vram_tile",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("battle/art/fusion.json"),
        &["frames"],
        "index",
    ),
    // Shops & inns
    KeyedArraySpec::edit_only(FilePattern::Exact("shops.json"), &["inventories"], "index"),
    KeyedArraySpec::edit_only(FilePattern::Exact("shops.json"), &["inns"], "index"),
    KeyedArraySpec::edit_only(FilePattern::Exact("shops.json"), &["counters"], "id"),
    KeyedArraySpec::edit_only(FilePattern::Exact("shops.json"), &["portraits"], "png"),
    // Dialogue & windows
    KeyedArraySpec::edit_only(
        FilePattern::Exact("dialogue/trees.json"),
        &["trees"],
        "tree",
    ),
    KeyedArraySpec::additive(
        FilePattern::Exact("dialogue/trees.json"),
        &["trees", "*", "entries"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("dialogue/trees.json"),
        &["talk_options", "entries"],
        "flag",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("dialogue/trees.json"),
        &["system_messages", "messages"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("dialogue/portraits.json"),
        &["portraits"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("dialogue/window.json"),
        &["windows"],
        "index",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("dialogue/window.json"),
        &["menu_windows"],
        "index",
    ),
    // Field maps
    KeyedArraySpec::additive(MAPS_JSON, &["npcs"], "index"),
    KeyedArraySpec::additive(MAPS_JSON, &["warps"], "index"),
    KeyedArraySpec::additive(MAPS_JSON, &["treasure_chests"], "index"),
    KeyedArraySpec::edit_only(MAPS_JSON, &["interaction_areas"], "index"),
    KeyedArraySpec::edit_only(MAPS_JSON, &["map_updates"], "index"),
    KeyedArraySpec::edit_only(MAPS_JSON, &["layout_variants"], "png"),
    KeyedArraySpec::edit_only(MAPS_JSON, &["patch_tiles", "tiles"], "index"),
    KeyedArraySpec::edit_only(MAPS_JSON, &["overworld_patches"], "event_flag"),
    // Travel, title, ship menu, presentation
    KeyedArraySpec::edit_only(FilePattern::Exact("travel.json"), &["towns"], "town_flag"),
    KeyedArraySpec::edit_only(FilePattern::Exact("travel.json"), &["dungeons"], "index"),
    KeyedArraySpec::edit_only(FilePattern::Exact("travel.json"), &["world_maps"], "name"),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("title/sound_test.json"),
        &["tracks"],
        "index",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("ship_menu/ship_menu.json"),
        &["windows"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("ship_menu/ship_menu.json"),
        &["place_windows"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("presentation/panels.json"),
        &["panels"],
        "id",
    ),
    // Sound tables
    KeyedArraySpec::edit_only(
        FilePattern::Exact("sound/music.json"),
        &["records", "records"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("sound/music.json"),
        &["records", "records", "*", "tracks"],
        "track_id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("sound/music.json"),
        &["records", "records", "*", "voices"],
        "index",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("sound/sfx.json"),
        &["records", "regular", "records"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("sound/sfx.json"),
        &["records", "regular", "records", "*", "tracks"],
        "track_id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("sound/sfx.json"),
        &["records", "regular", "records", "*", "voices"],
        "index",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("sound/sfx.json"),
        &["records", "special", "records"],
        "id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("sound/sfx.json"),
        &["records", "special", "records", "*", "tracks"],
        "track_id",
    ),
    KeyedArraySpec::edit_only(
        FilePattern::Exact("sound/sfx.json"),
        &["records", "special", "records", "*", "voices"],
        "index",
    ),
    // Sprite sheets
    KeyedArraySpec::edit_only(SPRITES_JSON, &["sheets"], "id"),
];

/// Returns the specification when `path` inside `rel_file` addresses a known
/// ID-keyed record array.
pub(crate) fn keyed_array_spec(rel_file: &str, path: &[&str]) -> Option<&'static KeyedArraySpec> {
    KEYED_ARRAYS
        .iter()
        .find(|spec| spec.file.matches(rel_file) && spec.path == path)
}

/// Returns the stable ID field name when `path` inside `rel_file` addresses a
/// known ID-keyed record array.
pub(crate) fn array_key_field(rel_file: &str, path: &[&str]) -> Option<&'static str> {
    keyed_array_spec(rel_file, path).map(|spec| spec.key_field)
}

/// Returns `true` when `(rel_file, schema_path)` addresses a JSON object that
/// maps arbitrary user/asset keys rather than fixed struct fields.
pub(crate) fn is_dynamic_map(rel_file: &str, schema_path: &[&str]) -> bool {
    match (rel_file, schema_path) {
        (file, ["map_update_images" | "art_cycle_images"]) if MAPS_JSON.matches(file) => true,
        ("dialogue/font.json", ["by_char"]) => true,
        ("dialogue/window.json", ["roles"]) => true,
        ("title/palette_cycle.json", ["frames"]) => true,
        ("battle/art/enemy_attacks.json", ["enemies", "*", "palette_overrides"]) => true,
        (
            "battle/art/enemies.json",
            ["palette_layout", "fixed_colors" | "index_15_by_cram_line"],
        ) => true,
        _ => false,
    }
}
