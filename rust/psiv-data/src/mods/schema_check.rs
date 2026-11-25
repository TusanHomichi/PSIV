//! Serde-driven unknown-field verification for patched pack JSON files.
//!
//! Instead of maintaining hand-written field allowlists that drift from
//! `psiv-data`'s Rust types, [`TrackedValue`] wraps `&serde_json::Value` as a
//! `serde::Deserializer` and records every JSON path for which serde invokes
//! `deserialize_ignored_any`. Comparing the ignored paths of the patched JSON
//! against the unmodified base JSON's baseline catches any unknown or misspelled
//! field anywhere in the deserialized schema—including newly inserted optional
//! subtrees, `null`-replaced fields, and replaced unkeyed arrays—while allowing
//! every modeled optional field automatically and tolerating retail packer
//! provenance fields that already existed in the unmodified base pack.

use serde::de::{
    self, DeserializeOwned, DeserializeSeed, Deserializer as _, EnumAccess, IntoDeserializer,
    MapAccess, SeqAccess, VariantAccess, Visitor,
};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::BTreeSet;

use super::patch::MAPS_JSON;

/// Baseline of ignored/research paths already present in the unmodified base JSON.
pub(crate) struct SchemaBaseline {
    ignored: BTreeSet<String>,
    extra_keys: BTreeSet<String>,
}

/// Computes the schema-validation baseline for `base` if `rel_file` has a
/// `psiv-data` Rust schema and `base` parses as a complete record of that schema.
///
/// Returns `None` for files whose schema lives outside `psiv-data` (e.g.,
/// Godot-only layout JSONs) or partial synthetic JSON snippets used in unit
/// tests; callers fall back to base-key comparison in that case.
pub(crate) fn prepare_baseline(rel_file: &str, base: &Value) -> Option<SchemaBaseline> {
    let ignored = collect_ignored_for_file(rel_file, base)?.ok()?;
    let extra_keys = collect_flatten_keys(rel_file, base);
    Some(SchemaBaseline {
        ignored,
        extra_keys,
    })
}

/// Verifies that `patched` introduces no unknown/ignored fields relative to
/// `baseline` according to `psiv-data`'s actual `Deserialize` schema for `rel_file`.
pub(crate) fn verify_patched(
    rel_file: &str,
    baseline: &SchemaBaseline,
    patched: &Value,
) -> Result<(), String> {
    let Some(res) = collect_ignored_for_file(rel_file, patched) else {
        return Ok(());
    };
    let patched_extra = collect_flatten_keys(rel_file, patched);
    if let Some(unknown) = patched_extra.difference(&baseline.extra_keys).next() {
        return Err(format!(
            "unknown field `{unknown}` (not in schema for `{rel_file}`)"
        ));
    }
    let patched_ignored = match res {
        Ok(set) => set,
        Err(error) => {
            // Files reloaded by GameData::load / BattleFiles::load (e.g. battle/enemies.json)
            // defer non-unknown-field value/range errors (like hp = 70000) to those loaders so
            // they report full pack file context; files outside GameData/BattleFiles fail closed
            // immediately here as well as in PackOverlay's explicit loaders.
            if error.contains("unknown field")
                || matches!(
                    rel_file,
                    crate::battle::COMMAND_UI_ART_PATH | crate::battle::DAMAGE_DIGIT_ART_PATH
                )
            {
                return Err(error);
            }
            return Ok(());
        }
    };
    if let Some(unknown) = patched_ignored.difference(&baseline.ignored).next() {
        return Err(format!(
            "unknown field `{unknown}` (not in schema for `{rel_file}`)"
        ));
    }
    Ok(())
}

fn collect_ignored_for_file(
    rel_file: &str,
    value: &Value,
) -> Option<Result<BTreeSet<String>, String>> {
    match rel_file {
        "battle/enemies.json" => Some(ignored_paths::<crate::battle::EnemiesFile>(value)),
        "battle/equipment.json" => Some(ignored_paths::<crate::battle::EquipmentFile>(value)),
        "battle/characters.json" => Some(ignored_paths::<crate::battle::CharactersFile>(value)),
        "battle/abilities.json" => Some(ignored_paths::<crate::battle::AbilitiesFile>(value)),
        "battle/formations.json" => Some(ignored_paths::<crate::battle::FormationsFile>(value)),
        "battle/levels.json" => Some(ignored_paths::<crate::battle::LevelsFile>(value)),
        "battle/enemy_animations.json" => {
            Some(ignored_paths::<crate::battle::EnemyAnimationsFile>(value))
        }
        "battle/art/backgrounds.json" => {
            Some(ignored_paths::<crate::battle::backgrounds::File>(value))
        }
        "battle/art/command_ui.json" => Some(ignored_paths::<crate::battle::CommandUiArt>(value)),
        "battle/art/damage_digits.json" => {
            Some(ignored_paths::<crate::battle::DamageDigitArt>(value))
        }
        "battle/art/fusion.json" => Some(ignored_paths::<crate::battle::FusionArt>(value)),
        "dialogue/trees.json" => Some(ignored_paths::<crate::dialogue::TreeFile>(value)),
        "dialogue/font.json" => Some(ignored_paths::<crate::dialogue::FontFile>(value)),
        "dialogue/portraits.json" => Some(ignored_paths::<crate::dialogue::PortraitFile>(value)),
        "dialogue/window.json" => Some(ignored_paths::<crate::dialogue::WindowFile>(value)),
        "manifest.json" => Some(ignored_paths::<crate::manifest::Manifest>(value)),
        "shops.json" => Some(ignored_paths::<crate::shops::ShopData>(value)),
        "sound/index.json" => Some(ignored_paths::<crate::sound::schema::IndexFile>(value)),
        "sound/driver.json" => Some(ignored_paths::<crate::sound::schema::DriverFile>(value)),
        "sound/music.json" => Some(ignored_paths::<crate::sound::schema::MusicFile>(value)),
        "sound/sfx.json" => Some(ignored_paths::<crate::sound::schema::SfxFile>(value)),
        "game_start.json" => Some(ignored_paths::<crate::new_game::File>(value)),
        "guild/guild.json" => Some(ignored_paths::<crate::guild::GuildJson>(value)),
        "fifth/fifth.json" => Some(ignored_paths::<crate::fifth::FifthJson>(value)),
        "travel.json" => Some(ignored_paths::<crate::travel::TravelData>(value)),
        "ship_menu/ship_menu.json" => Some(ignored_paths::<crate::ship_menu::ShipMenuJson>(value)),
        "title/sound_test.json" => Some(ignored_paths::<crate::sound_test::SoundTestFile>(value)),
        "presentation/panels.json" => Some(ignored_paths::<crate::presentation::File>(value)),
        "presentation/credits.json" => Some(ignored_paths::<crate::EndingCreditsAssets>(value)),
        file if MAPS_JSON.matches(file) => Some(ignored_paths::<crate::map::MapRecord>(value)),
        file if file.starts_with("sprites/") && file.ends_with(".json") => {
            Some(ignored_paths::<crate::sprites::SheetFile>(value))
        }
        _ => None,
    }
}

/// Captures keys absorbed by `#[serde(flatten)]` structs (`SheetFile`/`Sheet`
/// `research` maps and `InlineFormation`) that bypass `deserialize_ignored_any`.
fn collect_flatten_keys(rel_file: &str, value: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if rel_file.starts_with("sprites/")
        && rel_file.ends_with(".json")
        && let Ok(file) = serde_json::from_value::<crate::sprites::SheetFile>(value.clone())
    {
        for key in file.research.keys() {
            out.insert(key.clone());
        }
        for (i, sheet) in file.sheets.iter().enumerate() {
            for key in sheet.research.keys() {
                out.insert(format!("sheets[{i}].{key}"));
            }
        }
        for (pi, pv) in file.palette_variants.iter().enumerate() {
            for (si, sheet) in pv.sheets.iter().enumerate() {
                for key in sheet.research.keys() {
                    out.insert(format!("palette_variants[{pi}].sheets[{si}].{key}"));
                }
            }
        }
    } else if rel_file == "battle/formations.json"
        && let Some(arr) = value.get("inline_formations").and_then(Value::as_array)
    {
        const INLINE_FORMATION_KEYS: &[&str] = &[
            "label",
            "rom_offset",
            "loaded_by",
            "id",
            "event_battle_index",
            "ambush_chance",
            "run_chance",
            "can_run",
            "drop_rate",
            "drop_item",
            "count",
            "count_matches_entries",
            "enemies",
        ];
        for (i, item) in arr.iter().enumerate() {
            if let Some(obj) = item.as_object() {
                for key in obj.keys() {
                    if !INLINE_FORMATION_KEYS.contains(&key.as_str()) {
                        out.insert(format!("inline_formations[{i}].{key}"));
                    }
                }
            }
        }
    }
    out
}

fn ignored_paths<T: DeserializeOwned>(value: &Value) -> Result<BTreeSet<String>, String> {
    let ignored = RefCell::new(BTreeSet::new());
    let mut path = Vec::new();
    let de = TrackedValue {
        value,
        path: &mut path,
        ignored: &ignored,
    };
    T::deserialize(de).map_err(|error| error.to_string())?;
    Ok(ignored.into_inner())
}

fn format_tracked_path(segments: &[String]) -> String {
    let mut out = String::new();
    for seg in segments {
        if seg.starts_with('[') {
            out.push_str(seg);
        } else {
            if !out.is_empty() {
                out.push('.');
            }
            out.push_str(seg);
        }
    }
    out
}

struct TrackedValue<'a, 'b> {
    value: &'a Value,
    path: &'b mut Vec<String>,
    ignored: &'b RefCell<BTreeSet<String>>,
}

impl<'a, 'b> TrackedValue<'a, 'b> {
    fn child<'c>(&'c mut self, value: &'a Value) -> TrackedValue<'a, 'c> {
        TrackedValue {
            value,
            path: self.path,
            ignored: self.ignored,
        }
    }
}

impl<'de, 'b> de::Deserializer<'de> for TrackedValue<'de, 'b> {
    type Error = serde_json::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::Null => visitor.visit_unit(),
            Value::Bool(v) => visitor.visit_bool(*v),
            Value::Number(n) => n.deserialize_any(visitor),
            Value::String(s) => visitor.visit_borrowed_str(s),
            Value::Array(_) => self.deserialize_seq(visitor),
            Value::Object(_) => self.deserialize_map(visitor),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_bool(visitor)
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_i8(visitor)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_i16(visitor)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_i32(visitor)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_i64(visitor)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_u8(visitor)
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_u16(visitor)
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_u32(visitor)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_u64(visitor)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_f32(visitor)
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_f64(visitor)
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_char(visitor)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_str(visitor)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_string(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_bytes(visitor)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_byte_buf(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_unit(visitor)
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.value.deserialize_unit_struct(name, visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::Array(items) => visitor.visit_seq(TrackedSeqAccess {
                iter: items.iter().enumerate(),
                path: self.path,
                ignored: self.ignored,
            }),
            other => other.deserialize_seq(visitor),
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::Object(map) => visitor.visit_map(TrackedMapAccess {
                iter: map.iter(),
                pending: None,
                path: self.path,
                ignored: self.ignored,
            }),
            other => other.deserialize_map(visitor),
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::Array(_) => self.deserialize_seq(visitor),
            Value::Object(_) => self.deserialize_map(visitor),
            other => other.deserialize_map(visitor),
        }
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::String(s) => visitor.visit_enum(s.as_str().into_deserializer()),
            Value::Object(map) => {
                let mut iter = map.iter();
                let Some((variant, payload)) = iter.next() else {
                    return self.value.deserialize_enum(name, variants, visitor);
                };
                if iter.next().is_some() {
                    return self.value.deserialize_enum(name, variants, visitor);
                }
                visitor.visit_enum(TrackedEnumAccess {
                    variant,
                    payload,
                    path: self.path,
                    ignored: self.ignored,
                })
            }
            other => other.deserialize_enum(name, variants, visitor),
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.value.deserialize_identifier(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.ignored
            .borrow_mut()
            .insert(format_tracked_path(self.path));
        visitor.visit_unit()
    }
}

struct TrackedSeqAccess<'a, 'b> {
    iter: std::iter::Enumerate<std::slice::Iter<'a, Value>>,
    path: &'b mut Vec<String>,
    ignored: &'b RefCell<BTreeSet<String>>,
}

impl<'de, 'b> SeqAccess<'de> for TrackedSeqAccess<'de, 'b> {
    type Error = serde_json::Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Self::Error> {
        let Some((idx, item)) = self.iter.next() else {
            return Ok(None);
        };
        self.path.push(format!("[{idx}]"));
        let res = seed.deserialize(TrackedValue {
            value: item,
            path: self.path,
            ignored: self.ignored,
        });
        self.path.pop();
        res.map(Some)
    }
}

struct TrackedMapAccess<'a, 'b> {
    iter: serde_json::map::Iter<'a>,
    pending: Option<(&'a str, &'a Value)>,
    path: &'b mut Vec<String>,
    ignored: &'b RefCell<BTreeSet<String>>,
}

impl<'de, 'b> MapAccess<'de> for TrackedMapAccess<'de, 'b> {
    type Error = serde_json::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        let Some((k, v)) = self.iter.next() else {
            return Ok(None);
        };
        self.pending = Some((k.as_str(), v));
        seed.deserialize(k.as_str().into_deserializer()).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        let (k, v) = self
            .pending
            .take()
            .expect("next_value_seed called before next_key_seed");
        self.path.push(k.to_owned());
        let res = seed.deserialize(TrackedValue {
            value: v,
            path: self.path,
            ignored: self.ignored,
        });
        self.path.pop();
        res
    }
}

struct TrackedEnumAccess<'a, 'b> {
    variant: &'a str,
    payload: &'a Value,
    path: &'b mut Vec<String>,
    ignored: &'b RefCell<BTreeSet<String>>,
}

impl<'de, 'b> EnumAccess<'de> for TrackedEnumAccess<'de, 'b> {
    type Error = serde_json::Error;
    type Variant = TrackedVariantAccess<'de, 'b>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Self::Error> {
        let val = seed.deserialize(self.variant.into_deserializer())?;
        Ok((
            val,
            TrackedVariantAccess {
                variant: self.variant,
                payload: self.payload,
                path: self.path,
                ignored: self.ignored,
            },
        ))
    }
}

struct TrackedVariantAccess<'a, 'b> {
    variant: &'a str,
    payload: &'a Value,
    path: &'b mut Vec<String>,
    ignored: &'b RefCell<BTreeSet<String>>,
}

impl<'de, 'b> VariantAccess<'de> for TrackedVariantAccess<'de, 'b> {
    type Error = serde_json::Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        de::Deserialize::deserialize(self.payload)
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, Self::Error> {
        self.path.push(self.variant.to_owned());
        let mut tracked = TrackedValue {
            value: self.payload,
            path: self.path,
            ignored: self.ignored,
        };
        let res = seed.deserialize(tracked.child(self.payload));
        self.path.pop();
        res
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.path.push(self.variant.to_owned());
        let res = TrackedValue {
            value: self.payload,
            path: self.path,
            ignored: self.ignored,
        }
        .deserialize_seq(visitor);
        self.path.pop();
        res
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.path.push(self.variant.to_owned());
        let res = TrackedValue {
            value: self.payload,
            path: self.path,
            ignored: self.ignored,
        }
        .deserialize_map(visitor);
        self.path.pop();
        res
    }
}
