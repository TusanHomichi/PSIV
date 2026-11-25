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
    KeyedArraySpec {
        file: FilePattern::Exact("battle/enemies.json"),
        path: &["enemies"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/equipment.json"),
        path: &["items"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/equipment.json"),
        path: &["types"],
        key_field: "name",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/characters.json"),
        path: &["characters"],
        key_field: "character_id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/abilities.json"),
        path: &["techniques"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/abilities.json"),
        path: &["skills"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/abilities.json"),
        path: &["enemy_skills"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/abilities.json"),
        path: &["item_effects"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/abilities.json"),
        path: &["combos"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["formations"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["formations", "*", "enemies"],
        key_field: "slot",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["boss_formations"],
        key_field: "event_battle_index",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["boss_formations", "*", "enemies"],
        key_field: "slot",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["inline_formations"],
        key_field: "label",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["inline_formations", "*", "enemies"],
        key_field: "slot",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["encounter_groups", "groups"],
        key_field: "group",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["map_bindings"],
        key_field: "map_id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["position_grids"],
        key_field: "name",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/levels.json"),
        path: &["characters"],
        key_field: "character_id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/levels.json"),
        path: &["characters", "*", "levels"],
        key_field: "level",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/enemy_animations.json"),
        path: &["animations"],
        key_field: "enemy_id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/art/backgrounds.json"),
        path: &["backgrounds"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/art/command_ui.json"),
        path: &["tiles"],
        key_field: "vram_tile",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/art/damage_digits.json"),
        path: &["tiles"],
        key_field: "vram_tile",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/art/fusion.json"),
        path: &["frames"],
        key_field: "index",
    },
    // Shops & inns
    KeyedArraySpec {
        file: FilePattern::Exact("shops.json"),
        path: &["inventories"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("shops.json"),
        path: &["inns"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("shops.json"),
        path: &["counters"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("shops.json"),
        path: &["portraits"],
        key_field: "png",
    },
    // Dialogue & windows
    KeyedArraySpec {
        file: FilePattern::Exact("dialogue/trees.json"),
        path: &["trees"],
        key_field: "tree",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("dialogue/trees.json"),
        path: &["trees", "*", "entries"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("dialogue/trees.json"),
        path: &["talk_options", "entries"],
        key_field: "entry",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("dialogue/trees.json"),
        path: &["system_messages", "messages"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("dialogue/portraits.json"),
        path: &["portraits"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("dialogue/window.json"),
        path: &["windows"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("dialogue/window.json"),
        path: &["menu_windows"],
        key_field: "index",
    },
    // Field maps
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["npcs"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["warps"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["treasure_chests"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["interaction_areas"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["map_updates"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["layout_variants"],
        key_field: "png",
    },
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["patch_tiles", "tiles"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["overworld_patches"],
        key_field: "event_flag",
    },
    // Travel, title, ship menu, presentation
    KeyedArraySpec {
        file: FilePattern::Exact("travel.json"),
        path: &["towns"],
        key_field: "town_flag",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("travel.json"),
        path: &["dungeons"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("travel.json"),
        path: &["world_maps"],
        key_field: "name",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("title/sound_test.json"),
        path: &["tracks"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("ship_menu/ship_menu.json"),
        path: &["windows"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("ship_menu/ship_menu.json"),
        path: &["place_windows"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("presentation/panels.json"),
        path: &["panels"],
        key_field: "id",
    },
    // Sound tables
    KeyedArraySpec {
        file: FilePattern::Exact("sound/music.json"),
        path: &["records", "records"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("sound/music.json"),
        path: &["records", "records", "*", "tracks"],
        key_field: "track_id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("sound/music.json"),
        path: &["records", "records", "*", "voices"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("sound/sfx.json"),
        path: &["records", "regular", "records"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("sound/sfx.json"),
        path: &["records", "regular", "records", "*", "tracks"],
        key_field: "track_id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("sound/sfx.json"),
        path: &["records", "regular", "records", "*", "voices"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("sound/sfx.json"),
        path: &["records", "special", "records"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("sound/sfx.json"),
        path: &["records", "special", "records", "*", "tracks"],
        key_field: "track_id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("sound/sfx.json"),
        path: &["records", "special", "records", "*", "voices"],
        key_field: "index",
    },
    // Sprite sheets
    KeyedArraySpec {
        file: SPRITES_JSON,
        path: &["sheets"],
        key_field: "id",
    },
];

/// Returns the stable ID field name when `path` inside `rel_file` addresses a
/// known ID-keyed record array.
pub(crate) fn array_key_field(rel_file: &str, path: &[&str]) -> Option<&'static str> {
    KEYED_ARRAYS
        .iter()
        .find(|spec| spec.file.matches(rel_file) && spec.path == path)
        .map(|spec| spec.key_field)
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
