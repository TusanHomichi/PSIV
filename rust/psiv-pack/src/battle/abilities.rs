//! `battle/abilities.json`: the port of `extract_techniques`,
//! `extract_skills`, `extract_enemy_skills` and `extract_items` in
//! `psiv_tools/core.py`, and of `ability_effect_count` and `build_abilities`
//! in `psiv_tools/battle_pack.py`, whose module docs give the reasons: every
//! record's first byte indexes `AbilityEffectsOffs`, which `TRAP #2`
//! dispatches through with no bound, so the table's length is derived from
//! the cartridge and a record past it is marked rather than dropped.
//!
//! The file is `psiv_data::AbilitiesFile`. Each record's offset and reserved
//! bytes, the targeting byte's names, the dispatch target of a record past the
//! table, the counts and the census have no typed home yet and are laid over
//! it.

use std::collections::{BTreeSet, HashMap};

use psiv_data::{AbilitiesFile, Ability, AbilityTargeting, EffectTable};

use super::names::{
    Display, SKILLS, TECHNIQUES, display, element, enemy_skill_symbols, named_byte, stat, target,
    usability,
};
use super::{FORMAT_VERSION, range, serialize};
use crate::PackError;
use crate::json::{Json, overlay};
use crate::map_symbols::ITEM_SYMBOLS;

/// `AbilityEffectsOffs`, and `AbilityEffect_None`, the `rts` right after it.
const EFFECTS: usize = 0x0061BE;
const EFFECT_NONE: usize = 0x006216;
const RTS: [u8; 2] = [0x4E, 0x75];

/// `TABLES[...]`: where each table starts and how long its records are.
const TECHNIQUES_AT: usize = 0x2A9BE8;
const SKILLS_AT: usize = 0x2A9D28;
const ENEMY_SKILLS_AT: usize = 0x28336C;
const ITEMS_AT: usize = 0x2A8E28;
const ABILITY_BYTES: usize = 8;
const ITEM_BYTES: usize = 22;

const NOTE: &str = "TRAP #2 dispatches with `add.w d0,d0 / adda.w (a0,d0.w),a0 / jsr (a0)` \
and no bound, so an effect id past the table jumps wherever the following bytes point. Reject \
an ability whose effect_out_of_range is true rather than reproducing that.";

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn word(rom: &[u8], at: usize) -> Result<u16, PackError> {
    Ok(crate::m68k::w(rom, at)?)
}

/// `ability_effect_count`: the table ends where its lowest target begins,
/// which must be `AbilityEffect_None`.
pub fn effect_count(rom: &[u8]) -> Result<u8, PackError> {
    if rom.get(EFFECT_NONE..EFFECT_NONE + 2) != Some(&RTS[..]) {
        return Err(PackError(format!(
            "0x{EFFECT_NONE:06X} is not AbilityEffect_None's `rts`; the ability effect table \
             cannot be bounded"
        )));
    }
    let count = (EFFECT_NONE - EFFECTS) / 2;
    let lowest = (0..count)
        .map(|index| word(rom, EFFECTS + index * 2).map(|w| EFFECTS + usize::from(w)))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .min()
        .unwrap_or(EFFECT_NONE);
    if lowest != EFFECT_NONE {
        return Err(PackError(format!(
            "the lowest AbilityEffectsOffs target is 0x{lowest:06X}, not the \
             0x{EFFECT_NONE:06X} that bounds the table"
        )));
    }
    Ok(count as u8)
}

/// `_out_of_range_target`: where the dispatcher would jump for `effect`.
fn dispatch_target(rom: &[u8], effect: u8) -> Result<String, PackError> {
    Ok(hex6(
        EFFECTS + usize::from(word(rom, EFFECTS + usize::from(effect) * 2)?),
    ))
}

/// A record with no fields set yet.
fn blank(id: u16, kind: &str, effect_id: u8) -> Ability {
    Ability {
        id,
        kind: kind.to_owned(),
        symbol: None,
        name: None,
        display_name: None,
        effect_id,
        tp_cost: None,
        targeting: None,
        target_id: None,
        parameter_2: None,
        battle_object_or_graphic_id: None,
        relevant_stat: None,
        requires_weapon: None,
        power_or_hit_chance: None,
        targeting_or_parameter_3: None,
        resistance_stat: None,
        element: None,
        effect_out_of_range: false,
    }
}

/// Bytes 3 to 5, which every kind reads the same way.
fn common(record: &mut Ability, raw: &[u8]) {
    record.power_or_hit_chance = Some(u16::from(raw[3]));
    record.resistance_stat = Some(named_byte(raw[4], stat));
    record.element = Some(named_byte(raw[5], element));
}

/// `_decode_targeting`'s names for byte 2.
fn targeting(value: u8) -> Json {
    let range = value & 0x0F;
    let usable = value >> 4;
    Json::object()
        .with("target_id", range)
        .with("target", target(range))
        .with("usability_id", usable)
        .with("usability", usability(usable))
}

/// A technique or skill: `_ability_common` with byte 1 read by `kind`.
fn technique_or_skill(id: u16, kind: &str, name: &str, raw: &[u8]) -> Ability {
    let mut record = blank(id, kind, raw[0]);
    record.name = Some(name.to_owned());
    if kind == "techniques" {
        record.tp_cost = Some(raw[1]);
    } else {
        record.relevant_stat = Some(named_byte(raw[1] & 0x7F, stat));
        record.requires_weapon = Some(raw[1] & 0x80 != 0);
    }
    record.targeting = Some(AbilityTargeting { raw: raw[2] });
    common(&mut record, raw);
    record
}

fn enemy_skill(id: u16, symbol: &str, raw: &[u8]) -> Ability {
    let mut record = blank(id, "enemy_skills", raw[0]);
    record.symbol = Some(symbol.to_owned());
    record.relevant_stat = Some(named_byte(raw[1], stat));
    record.target_id = Some(raw[2]);
    common(&mut record, raw);
    record
}

/// The eight-byte battle record embedded at the head of an inventory entry.
fn item_effect(id: u16, symbol: &str, raw: &[u8]) -> Ability {
    let mut record = blank(id, "item_effects", raw[0]);
    record.symbol = Some(symbol.to_owned());
    record.parameter_2 = Some(raw[1]);
    record.targeting_or_parameter_3 = Some(raw[2]);
    record.battle_object_or_graphic_id = Some(raw[7]);
    common(&mut record, raw);
    record
}

/// One kind's table: its records and what each carries beside the schema.
struct Table {
    kind: &'static str,
    records: Vec<Ability>,
    extra: Vec<Json>,
}

fn table(
    rom: &[u8],
    kind: &'static str,
    start: usize,
    stride: usize,
    labels: &[&str],
    read: impl Fn(u16, &str, &[u8]) -> Ability,
) -> Result<Table, PackError> {
    let mut records = Vec::new();
    let mut extra = Vec::new();
    for (index, label) in labels.iter().enumerate() {
        let offset = start + index * stride;
        let id = index as u16 + 1;
        let raw = rom
            .get(offset..offset + stride)
            .ok_or_else(|| PackError(format!("{kind} record {id} runs past end of ROM")))?;
        let record = read(id, label, raw);
        let mut more = Json::object().with("rom_offset", hex6(offset));
        if kind == "item_effects" {
            more.insert("reserved_7", raw[6]);
        } else {
            more.insert("reserved", vec![raw[6], raw[7]]);
        }
        if let Some(byte) = &record.targeting {
            more.insert("targeting", targeting(byte.raw));
        }
        records.push(record);
        extra.push(more);
    }
    Ok(Table {
        kind,
        records,
        extra,
    })
}

/// Names, the range check and the dispatch target, and the census entry of
/// a record past the table.
fn finish(
    rom: &[u8],
    table: &mut Table,
    names: &Display,
    count: u8,
    out_of_range: &mut Vec<Json>,
) -> Result<(), PackError> {
    for (record, extra) in table.records.iter_mut().zip(&mut table.extra) {
        record.display_name = display(names, record.id);
        record.effect_out_of_range = record.effect_id >= count;
        if !record.effect_out_of_range {
            continue;
        }
        let target = dispatch_target(rom, record.effect_id)?;
        extra.insert("effect_dispatch_target", target.as_str());
        out_of_range.push(
            Json::object()
                .with("kind", table.kind)
                .with("id", record.id)
                .with("symbol", record.symbol.clone().or(record.name.clone()))
                .with("display_name", record.display_name.clone())
                .with("effect_id", record.effect_id)
                .with("effect_id_hex", format!("0x{:02X}", record.effect_id))
                .with("dispatch_target", target),
        );
    }
    Ok(())
}

fn census(file: &AbilitiesFile, count: u8, out_of_range: Vec<Json>) -> Json {
    let used: BTreeSet<u8> = file.all().map(|r| r.effect_id).collect();
    let elements: BTreeSet<u16> = file
        .all()
        .filter_map(|r| r.element.as_ref().map(|e| e.id))
        .collect();
    Json::object()
        .with("effect_ids_used", used.iter().copied().collect::<Vec<_>>())
        .with(
            "effect_ids_in_table_unused",
            (0..count)
                .filter(|id| !used.contains(id))
                .collect::<Vec<_>>(),
        )
        .with("effect_out_of_range", out_of_range)
        .with("elements_used", elements.into_iter().collect::<Vec<_>>())
        .with(
            "tp_costs",
            range(
                file.techniques
                    .iter()
                    .filter_map(|r| r.tp_cost.map(u64::from)),
            ),
        )
        .with(
            "skills_requiring_a_weapon",
            file.skills
                .iter()
                .filter(|r| r.requires_weapon == Some(true))
                .count(),
        )
}

/// The whole file.
pub fn build(rom: &[u8], names: &HashMap<&'static str, Display>) -> Result<Json, PackError> {
    let count = effect_count(rom)?;
    let enemy_skills = enemy_skill_symbols();
    let mut tables = [
        table(
            rom,
            "techniques",
            TECHNIQUES_AT,
            ABILITY_BYTES,
            &TECHNIQUES,
            |id, name, raw| technique_or_skill(id, "techniques", name, raw),
        )?,
        table(
            rom,
            "skills",
            SKILLS_AT,
            ABILITY_BYTES,
            &SKILLS,
            |id, name, raw| technique_or_skill(id, "skills", name, raw),
        )?,
        table(
            rom,
            "enemy_skills",
            ENEMY_SKILLS_AT,
            ABILITY_BYTES,
            &enemy_skills,
            enemy_skill,
        )?,
        table(
            rom,
            "item_effects",
            ITEMS_AT,
            ITEM_BYTES,
            &ITEM_SYMBOLS,
            item_effect,
        )?,
    ];
    let mut out_of_range = Vec::new();
    let empty = Display::new();
    for table in &mut tables {
        let kind_names = if table.kind == "item_effects" {
            names.get("items")
        } else {
            names.get(table.kind)
        };
        finish(
            rom,
            table,
            kind_names.unwrap_or(&empty),
            count,
            &mut out_of_range,
        )?;
    }
    let [techniques, skills, enemy_skills, item_effects] = tables;
    let mut over = Json::object()
        .with("kind", "battle_abilities")
        .with("record_bytes", ABILITY_BYTES)
        .with(
            "effects",
            Json::object()
                .with("label", "AbilityEffectsOffs")
                .with("note", NOTE),
        );
    let mut counts = Json::object();
    for table in [&techniques, &skills, &enemy_skills, &item_effects] {
        counts.insert(table.kind, table.records.len());
        over.insert(table.kind, table.extra.clone());
    }
    let file = AbilitiesFile {
        format_version: FORMAT_VERSION,
        effects: EffectTable {
            table: hex6(EFFECTS),
            count: u32::from(count),
            bounds_checked: false,
        },
        techniques: techniques.records,
        skills: skills.records,
        enemy_skills: enemy_skills.records,
        item_effects: item_effects.records,
    };
    over.insert("counts", counts);
    over.insert("census", census(&file, count, out_of_range));
    let mut out = serialize(&file, "the abilities")?;
    overlay(&mut out, &over);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ROM whose effect table has `count` entries, the lowest pointing at
    /// the `rts` right after it.
    fn rom_with_table(lowest: usize) -> Vec<u8> {
        let mut rom = vec![0u8; EFFECT_NONE + 0x100];
        for index in 0..(EFFECT_NONE - EFFECTS) / 2 {
            let target = (EFFECT_NONE - EFFECTS + 2 * index) as u16;
            rom[EFFECTS + index * 2..][..2].copy_from_slice(&target.to_be_bytes());
        }
        rom[EFFECTS + 6..][..2].copy_from_slice(&((lowest - EFFECTS) as u16).to_be_bytes());
        rom[EFFECT_NONE..][..2].copy_from_slice(&RTS);
        rom
    }

    #[test]
    fn the_table_ends_at_its_lowest_target() {
        assert_eq!(effect_count(&rom_with_table(EFFECT_NONE)).unwrap(), 44);
    }

    #[test]
    fn a_target_inside_the_table_or_a_missing_rts_refuses() {
        let error = effect_count(&rom_with_table(EFFECT_NONE - 2)).unwrap_err();
        assert!(
            error.0.contains("lowest AbilityEffectsOffs target"),
            "{}",
            error.0
        );
        let mut rom = rom_with_table(EFFECT_NONE);
        rom[EFFECT_NONE] = 0;
        assert!(effect_count(&rom).unwrap_err().0.contains("rts"));
    }

    #[test]
    fn the_dispatch_target_reads_one_word_past_the_table() {
        let mut rom = rom_with_table(EFFECT_NONE);
        rom[EFFECTS + 44 * 2..][..2].copy_from_slice(&RTS);
        assert_eq!(dispatch_target(&rom, 44).unwrap(), hex6(EFFECTS + 0x4E75));
    }

    #[test]
    fn byte_one_is_a_cost_for_a_technique_and_a_stat_for_a_skill() {
        let raw = [5, 0x83, 0x31, 40, 2, 3, 0, 0];
        let technique = technique_or_skill(1, "techniques", "Foi", &raw);
        assert_eq!(technique.tp_cost, Some(0x83));
        assert!(technique.relevant_stat.is_none());
        let skill = technique_or_skill(1, "skills", "Crosscut", &raw);
        assert_eq!(skill.relevant_stat.as_ref().unwrap().id, 3);
        assert_eq!(skill.requires_weapon, Some(true));
        assert_eq!(
            skill.element.as_ref().unwrap().name.as_deref(),
            Some("fire")
        );
        let names = serde_json::Value::from(&targeting(0x31));
        assert_eq!(names["target"], "single_enemy");
        assert_eq!(names["usability"], "battle_and_field");
    }

    #[test]
    fn an_enemy_skill_keeps_its_whole_stat_byte_and_an_item_its_object() {
        let raw = [1, 0x85, 9, 7, 0, 15, 0, 0];
        let skill = enemy_skill(2, "S", &raw);
        assert_eq!(skill.relevant_stat.as_ref().unwrap().id, 0x85);
        assert!(skill.relevant_stat.as_ref().unwrap().name.is_none());
        assert_eq!(skill.target_id, Some(9));
        assert!(skill.element.as_ref().unwrap().name.is_none());
        let item = item_effect(3, "I", &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            (
                item.parameter_2,
                item.targeting_or_parameter_3,
                item.battle_object_or_graphic_id
            ),
            (Some(2), Some(3), Some(8))
        );
    }
}
