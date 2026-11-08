//! `battle/characters.json`: the port of `extract_characters` in
//! `psiv_tools/core.py` and `build_characters` in
//! `psiv_tools/battle_records.py`, whose docs give the reasons: the eleven
//! `InitialCharStats` records, and the RAM `InitializeCharStats` leaves once
//! it has applied one (the `initialized` block, a conformance vector derived
//! with the rules [`super::rules`] decodes). `battle_pack.py` adds each
//! character's age and STATUS portrait, and refuses a pack whose initial
//! equipment the equip filter would refuse.
//!
//! The file is `psiv_data::CharactersFile`. Each record's offset and raw
//! bytes, the profession's symbol, the property meanings, the known ability
//! lists, the portrait's research, the source and the census have no typed
//! home yet and are laid over it.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use psiv_data::{
    Character, CharactersFile, DerivedStat, Equipped, Initialized, LevelStats, Loadout, NamedId,
    Property, SkillSlots, StatusPortrait, TechniqueSlots, WeaponElements,
};

use super::equipment::Item;
use super::names::{
    CHARACTERS, Display, PROFESSIONS, PROPERTIES, SKILLS, TECHNIQUES, display, element, named_byte,
    property_level,
};
use super::rules::{
    CHARACTER_BYTES, CHARACTER_TABLE, CharInit, ElementRules, Rules, SLOTS, TypeRule,
};
use super::{FORMAT_VERSION, range, serialize};
use crate::PackError;
use crate::json::{Json, overlay};
use crate::png::hex;

const APPLIED_NOTE: &str = "runs once for all eleven characters whether or not they are in \
the party, and finishes each with UpdateCharModStats and UpdateCharElems";

const STATS: [&str; 4] = ["strength", "mental", "agility", "dexterity"];

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

/// The display-name tables this file reads.
pub struct Names<'a> {
    pub characters: &'a Display,
    pub professions: &'a Display,
    pub techniques: &'a Display,
    pub skills: &'a Display,
    pub items: &'a Display,
}

/// One 66-byte `InitialCharStats` record.
struct Record<'a> {
    id: usize,
    raw: &'a [u8],
}

impl Record<'_> {
    fn word(&self, at: usize) -> u16 {
        u16::from_be_bytes([self.raw[at], self.raw[at + 1]])
    }

    fn stat(&self, name: &str) -> Option<u8> {
        STATS
            .iter()
            .position(|s| *s == name)
            .map(|index| self.raw[12 + index])
    }

    fn property(&self, index: usize) -> u8 {
        self.raw[16 + index]
    }
}

/// `_slots`: a positional ability array, with the stored ids resolved.
fn known(raw: &[u8], names: &[&str], display_names: &Display) -> Vec<Json> {
    raw.iter()
        .enumerate()
        .filter(|(_, value)| **value != 0)
        .map(|(slot, &value)| {
            Json::object()
                .with("slot", slot)
                .with("id", value)
                .with("symbol", names.get(usize::from(value) - 1).copied())
                .with("display_name", display(display_names, u16::from(value)))
        })
        .collect()
}

/// One derived-stat pass: the base stat plus every equipped item's signed
/// bonuses. `add.b` into a byte register wraps; the word passes keep 16 bits.
fn derive(base: u8, bonuses: &[&str], equipped: &[(&str, &Item)], result_bytes: u8) -> i32 {
    let mut total = i32::from(base);
    for (_, item) in equipped {
        for field in bonuses {
            total += i32::from(item.bonus(field));
        }
    }
    total & if result_bytes == 1 { 0xFF } else { 0xFFFF }
}

/// `UpdateCharElems` over one loadout: a hand holding anything but a shield
/// sets that hand's attack element; a shield or armour sets its element's
/// property to the resistance value; a property nothing touched keeps the
/// record's own byte (`own`). Returns the properties and the two hands'
/// elements.
fn element_bytes(
    own: &[u8],
    equipped: &[(&str, &Item)],
    rules: &ElementRules,
) -> Result<(Vec<u8>, [u8; 2]), String> {
    let mut granted = vec![0u16; own.len()];
    let mut weapons = [0u8; 2];
    for (slot, item) in equipped {
        let id = item.element();
        if let Some(hand) = SLOTS[..2].iter().position(|s| s == slot) {
            if u16::from(item.kind()) != rules.shield_type {
                weapons[hand] = id;
                continue;
            }
            weapons[hand] = 0;
        }
        if id != 0 {
            let slot = granted
                .get_mut(usize::from(id) - 1)
                .ok_or_else(|| format!("wears element {id}, past the property slots"))?;
            *slot = rules.resistance_value;
        }
    }
    let props = own
        .iter()
        .zip(&granted)
        .map(|(&own, &granted)| {
            if granted == 0 {
                Ok(own)
            } else {
                u8::try_from(granted)
                    .map_err(|_| format!("resistance value {granted} does not fit a property byte"))
            }
        })
        .collect::<Result<_, _>>()?;
    Ok((props, weapons))
}

/// `_initialize`: `UpdateCharModStats` and `UpdateCharElems` run over one
/// record with the decoded rules.
fn initialize(
    record: &Record,
    equipped: &[(&str, &Item)],
    rules: &Rules,
) -> Result<(Initialized, Json), PackError> {
    let mut derived: BTreeMap<String, i32> = BTreeMap::new();
    for pass in &rules.derived.passes {
        let base = record.stat(&pass.base_stat).ok_or_else(|| {
            PackError(format!(
                "a derived-stat pass starts from {}, which is not a record stat",
                pass.base_stat
            ))
        })?;
        let total = derive(base, &pass.bonuses, equipped, pass.result_bytes);
        derived.insert(pass.stat.clone(), total);
    }
    let get = |name: &str| {
        derived
            .get(name)
            .copied()
            .ok_or_else(|| PackError(format!("no derived-stat pass writes {name}")))
    };
    let mut stats = BTreeMap::new();
    for name in STATS {
        stats.insert(
            name.to_owned(),
            DerivedStat {
                base: record.stat(name).expect("a record stat"),
                modified: get(&format!("{name}_mod"))? as u8,
            },
        );
    }

    let own: Vec<u8> = (0..PROPERTIES.len()).map(|i| record.property(i)).collect();
    let (props, weapons) = element_bytes(&own, equipped, &rules.elements)
        .map_err(|error| PackError(format!("character {}: {error}", record.id)))?;
    let mut element_props = BTreeMap::new();
    let mut meanings = Json::object();
    for (name, &value) in PROPERTIES.iter().zip(&props) {
        element_props.insert((*name).to_owned(), Property { value });
        meanings.insert(*name, Json::object().with("meaning", property_level(value)));
    }
    let mut extra = Json::object().with("element_props", meanings);
    for (name, value) in &derived {
        if !name.ends_with("_mod") {
            extra.insert(name.as_str(), *value);
        }
    }
    let initialized = Initialized {
        stats,
        atk_pow: get("atk_pow")? as u16,
        dfs_pow: get("dfs_pow")? as u16,
        magic_dfs: get("magic_dfs")? as u16,
        element_props,
        weapon_elements: WeaponElements {
            right_hand: named_byte(weapons[0], element),
            left_hand: named_byte(weapons[1], element),
        },
    };
    Ok((initialized, extra))
}

fn equipped_in(loadout: &Loadout, slot: &str) -> Option<Equipped> {
    loadout
        .slots()
        .iter()
        .find(|(name, _)| *name == slot)
        .and_then(|(_, item)| item.cloned())
}

/// `_character_census`, and the initial equipment the equip filter refuses.
fn census(
    characters: &[Character],
    known: &[(usize, usize)],
    types: &BTreeMap<u8, TypeRule>,
    items: &HashMap<u8, &Item>,
) -> Result<(Json, usize), PackError> {
    let rule = |kind: u8| {
        types.get(&kind).ok_or_else(|| {
            PackError(format!(
                "an initial item has type {kind}, which no rule covers"
            ))
        })
    };
    let mut refused = Vec::new();
    let mut downgraded = Vec::new();
    let mut left_hand_weapons = Vec::new();
    let mut two_handed_conflicts = Vec::new();
    for character in characters {
        let id = usize::from(character.character_id);
        for (slot, entry) in character.equipment.slots() {
            let Some(entry) = entry else { continue };
            let item = items[&entry.item_id];
            let kind = rule(entry.kind)?;
            let reason = if !kind.equippable {
                "type is not equippable".to_owned()
            } else if item.used_by_mask() & (1 << id) == 0 {
                "usable_by does not include this character".to_owned()
            } else if kind.slot != Some(slot) && !(slot == "left_hand" && kind.is_weapon) {
                format!("the equip menu never places this type in {slot}")
            } else {
                continue;
            };
            refused.push(
                Json::object()
                    .with("character_id", id)
                    .with("slot", slot)
                    .with("item_id", entry.item_id)
                    .with("reason", reason),
            );
        }
        for name in PROPERTIES {
            let before = character.properties[name].value;
            let after = character.initialized.element_props[name].value;
            if after > before {
                downgraded.push(
                    Json::object()
                        .with("character_id", id)
                        .with("element", name)
                        .with("record_value", before)
                        .with("initialized_value", after),
                );
            }
        }
        let left = equipped_in(&character.equipment, "left_hand");
        if let Some(left) = &left
            && rule(left.kind)?.is_weapon
        {
            left_hand_weapons.push(id);
        }
        if let Some(right) = equipped_in(&character.equipment, "right_hand")
            && rule(right.kind)?.two_handed
            && left.is_some()
        {
            two_handed_conflicts.push(id);
        }
    }
    let filled = |c: &Character| {
        c.equipment
            .slots()
            .iter()
            .filter(|(_, e)| e.is_some())
            .count()
    };
    let set = |values: Vec<u64>| {
        values
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    };
    let refused_count = refused.len();
    let json = Json::object()
        .with(
            "professions_used",
            set(characters
                .iter()
                .map(|c| u64::from(c.profession.id))
                .collect()),
        )
        .with(
            "starting_levels",
            set(characters.iter().map(|c| u64::from(c.level)).collect()),
        )
        .with("hp", range(characters.iter().map(|c| u64::from(c.hp))))
        .with("tp", range(characters.iter().map(|c| u64::from(c.tp))))
        .with(
            "with_techniques",
            known.iter().filter(|(t, _)| *t > 0).count(),
        )
        .with("with_skills", known.iter().filter(|(_, s)| *s > 0).count())
        .with(
            "techniques_known",
            range(known.iter().map(|(t, _)| *t as u64)),
        )
        .with("skills_known", range(known.iter().map(|(_, s)| *s as u64)))
        .with(
            "equipment_slots_filled",
            range(characters.iter().map(|c| filled(c) as u64)),
        )
        .with(
            "starting_with_experience",
            characters
                .iter()
                .filter(|c| c.experience != 0)
                .map(|c| c.character_id)
                .collect::<Vec<_>>(),
        )
        .with("equipment_the_owner_cannot_equip", refused)
        .with("element_props_weakened_by_equipment", downgraded)
        .with("weapon_in_left_hand", left_hand_weapons)
        .with("two_handed_with_left_hand_occupied", two_handed_conflicts);
    Ok((json, refused_count))
}

/// The whole file. `seated` holds each character's age and
/// `status_portraits::write` record, by character id.
pub fn build(
    rom: &[u8],
    init: &CharInit,
    rules: &Rules,
    types: &BTreeMap<u8, TypeRule>,
    items: &[Item],
    names: &Names,
    seated: &[(u16, Json)],
) -> Result<Json, PackError> {
    let by_id: HashMap<u8, &Item> = items.iter().map(|item| (item.id, item)).collect();
    let mut characters = Vec::new();
    let mut extra = Vec::new();
    let mut known_counts = Vec::new();
    for (id, symbol) in CHARACTERS.iter().enumerate() {
        let offset = CHARACTER_TABLE + id * CHARACTER_BYTES;
        let raw = rom
            .get(offset..offset + CHARACTER_BYTES)
            .ok_or_else(|| PackError(format!("Character record {id} runs past end of ROM")))?;
        let record = Record { id, raw };

        let mut loadout = Loadout {
            right_hand: None,
            left_hand: None,
            head: None,
            body: None,
        };
        let mut equipped = Vec::new();
        for (index, slot) in SLOTS.iter().enumerate() {
            let item_id = raw[30 + index];
            if item_id == 0 {
                continue;
            }
            let item = by_id.get(&item_id).ok_or_else(|| {
                PackError(format!(
                    "character {id} equips item {item_id}, which has no record"
                ))
            })?;
            let entry = Equipped {
                item_id,
                symbol: item.symbol.to_owned(),
                display_name: display(names.items, u16::from(item_id)),
                kind: item.kind(),
            };
            match index {
                0 => loadout.right_hand = Some(entry),
                1 => loadout.left_hand = Some(entry),
                2 => loadout.head = Some(entry),
                _ => loadout.body = Some(entry),
            }
            equipped.push((*slot, *item));
        }
        let (initialized, initialized_extra) = initialize(&record, &equipped, rules)?;

        let profession = record.word(0);
        let profession_symbol = PROFESSIONS
            .get(usize::from(profession))
            .ok_or_else(|| PackError(format!("character {id} has profession {profession}")))?;
        let techniques = known(&raw[34..50], &TECHNIQUES, names.techniques);
        let mut skills = known(&raw[50..58], &SKILLS, names.skills);
        for skill in &mut skills {
            let Some(Json::Int(slot)) = skill.get("slot") else {
                unreachable!("known() writes a slot")
            };
            let uses = raw[58 + *slot as usize];
            skill.insert("uses", uses);
            skill.insert("max_uses", uses);
        }
        known_counts.push((techniques.len(), skills.len()));
        let (age, portrait) = seated
            .get(id)
            .ok_or_else(|| PackError(format!("no STATUS portrait for character {id}")))?;
        let png = match portrait.get("png") {
            Some(Json::Str(png)) => png.clone(),
            _ => return Err(PackError(format!("portrait {id} has no png"))),
        };
        let mut properties = BTreeMap::new();
        let mut meanings = Json::object();
        for (index, name) in PROPERTIES.iter().enumerate() {
            let value = record.property(index);
            properties.insert((*name).to_owned(), Property { value });
            meanings.insert(*name, Json::object().with("meaning", property_level(value)));
        }
        characters.push(Character {
            character_id: id as u8,
            symbol: (*symbol).to_owned(),
            display_name: display(names.characters, id as u16),
            profession: NamedId {
                id: profession,
                name: None,
            },
            age: *age,
            status_portrait: Some(StatusPortrait { png }),
            level: record.word(2),
            experience: u32::from_be_bytes(raw[4..8].try_into().expect("4 bytes")),
            hp: record.word(8),
            max_hp: record.word(8),
            tp: record.word(10),
            max_tp: record.word(10),
            stats: LevelStats {
                strength: raw[12],
                mental: raw[13],
                agility: raw[14],
                dexterity: raw[15],
            },
            properties,
            equipment: loadout,
            techniques: TechniqueSlots {
                slots: raw[34..50].to_vec(),
            },
            skills: SkillSlots {
                slots: raw[50..58].to_vec(),
                uses: raw[58..66].to_vec(),
            },
            initialized,
        });
        extra.push(
            Json::object()
                .with("rom_offset", hex6(offset))
                .with(
                    "profession",
                    Json::object()
                        .with("symbol", *profession_symbol)
                        .with("display_name", display(names.professions, profession)),
                )
                .with("properties", meanings)
                .with("techniques", Json::object().with("known", techniques))
                .with("skills", Json::object().with("known", skills))
                .with("initialized", initialized_extra)
                .with("status_portrait", portrait.clone())
                .with("raw_hex", hex(raw)),
        );
    }
    let (census, refused) = census(&characters, &known_counts, types, &by_id)?;
    if refused != 0 {
        return Err(PackError(format!(
            "initial equipment the equip filter refuses: {:?}",
            census.get("equipment_the_owner_cannot_equip")
        )));
    }
    let file = CharactersFile {
        format_version: FORMAT_VERSION,
        count: characters.len() as u32,
        characters,
    };
    let mut out = serialize(&file, "the characters")?;
    // The profession is `{id, symbol, display_name}`, not a `NamedId`'s
    // `{id, name}`: drop the schema's name before the research goes on.
    if let Json::Object(pairs) = &mut out
        && let Some((_, Json::Array(list))) = pairs.iter_mut().find(|(k, _)| k == "characters")
    {
        for character in list {
            if let Json::Object(fields) = character
                && let Some((_, Json::Object(profession))) =
                    fields.iter_mut().find(|(k, _)| k == "profession")
            {
                profession.retain(|(k, _)| k != "name");
            }
        }
    }
    let applied_by = Json::object()
        .with("label", "InitializeCharStats")
        .with("rom_offset", hex6(init.routine))
        .with(
            "character_stats_ram",
            format!("0x{:08X}", init.character_stats_ram),
        )
        .with("struct_bytes", init.struct_bytes)
        .with("note", APPLIED_NOTE);
    let over = Json::object()
        .with("kind", "battle_characters")
        .with(
            "source",
            Json::object()
                .with("label", "InitialCharStats")
                .with("rom_offset", hex6(init.table))
                .with("record_bytes", init.record_bytes)
                .with("applied_by", applied_by)
                .with("record_layout", init.fields_json()),
        )
        .with("characters", extra)
        .with("census", census);
    overlay(&mut out, &over);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_slots_skip_zero_and_name_past_the_list_as_null() {
        let names = Display::new();
        let list = known(&[0, 2, 0, 41], &TECHNIQUES, &names);
        let list: Vec<serde_json::Value> = list.iter().map(serde_json::Value::from).collect();
        assert_eq!(list.len(), 2);
        assert_eq!(
            (list[0]["slot"].as_u64(), list[0]["symbol"].as_str()),
            (Some(1), Some("Gifoi"))
        );
        assert_eq!(list[1]["id"], 41);
        assert!(list[1]["symbol"].is_null());
    }

    #[test]
    fn a_record_reads_its_stats_and_property_bytes() {
        let mut raw = [0u8; CHARACTER_BYTES];
        raw[12..16].copy_from_slice(&[10, 11, 12, 13]);
        raw[16] = 4;
        raw[29] = 1;
        let record = Record { id: 0, raw: &raw };
        assert_eq!(record.stat("agility"), Some(12));
        assert_eq!(record.stat("atk_pow"), None);
        assert_eq!((record.property(0), record.property(13)), (4, 1));
    }

    fn item(kind: u8, element: u8, strength: i8) -> Item {
        let mut raw = [0u8; super::super::rules::ITEM_BYTES];
        raw[10] = kind;
        raw[11] = strength as u8;
        raw[18] = element;
        Item {
            id: 1,
            symbol: "Test",
            offset: 0,
            raw,
        }
    }

    #[test]
    fn a_pass_adds_signed_bonuses_and_wraps_to_its_width() {
        let dagger = item(1, 0, -3);
        let ring = item(6, 0, 10);
        let worn = [("right_hand", &dagger), ("head", &ring)];
        assert_eq!(derive(10, &["strength"], &worn, 1), 17);
        assert_eq!(derive(250, &["strength"], &worn, 1), 1);
        let cursed = [("right_hand", &dagger)];
        assert_eq!(derive(1, &["strength"], &cursed, 2), 0xFFFE);
        assert_eq!(derive(1, &["strength", "strength"], &cursed, 2), 0xFFFB);
    }

    fn element_rules() -> ElementRules {
        ElementRules {
            routine: 0,
            element_offset: 0x12,
            type_offset: 0x0A,
            shield_type: 5,
            elements: 14,
            props: 0x30,
            resistance_value: 1,
            right: 0x50,
            left: 0x51,
        }
    }

    #[test]
    fn armour_and_shields_grant_resistance_and_weapons_set_their_hand() {
        let sword = item(1, 7, 0);
        let shield = item(5, 5, 0);
        let mail = item(7, 3, 0);
        let worn = [
            ("right_hand", &sword),
            ("left_hand", &shield),
            ("body", &mail),
        ];
        let own = [2u8; 14];
        let mut immune = own;
        immune[2] = 0;
        let (props, weapons) = element_bytes(&immune, &worn, &element_rules()).unwrap();
        assert_eq!(weapons, [7, 0]);
        assert_eq!((props[2], props[4]), (1, 1), "fire and water granted");
        assert_eq!(props[6], 2, "a weapon's element grants nothing");
        let (bare, none) = element_bytes(&own, &[], &element_rules()).unwrap();
        assert_eq!((bare, none), (own.to_vec(), [0, 0]));
        let past = item(7, 15, 0);
        assert!(element_bytes(&own, &[("body", &past)], &element_rules()).is_err());
    }
}
