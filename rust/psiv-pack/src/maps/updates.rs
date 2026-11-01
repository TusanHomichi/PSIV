//! Retail `RunMapUpdates` programs: the port of `psiv_tools/map_updates.py`,
//! which owns the reasons (#216, slice 3; `docs/field/MAP_UPDATES.md` is the
//! census). The dispatcher names its own 64-entry `bra.w` table, and code picks
//! the clocks, CRAM destinations, gates and ROM tables: nothing here embeds a
//! palette. No claimed section writes these yet, so the gate holds them to
//! Python as the `map_updates` probe.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::records::{MapRecord, hex6};
use crate::PackError;
use crate::json::Json;
use crate::m68k::{bra_target, l, self_bounded_bra_table, sw, w};
use crate::png::hex;

const NAMES: [&str; 64] = [
    "MapUpdate_NoUpdate",
    "MapUpdate_MotaWaterPal",
    "MapUpdate_MotaTownsWaterPal",
    "MapUpdate_PiataBasementB2_MonstersPal",
    "MapUpdate_MotaQuicksandPal",
    "MapUpdate_ZioFortMoveUpdateRate",
    "MapUpdate_ZioFortTunnels",
    "MapUpdate_ZioFortMoveUpdateRate2",
    "MapUpdate_CaveWaterPal",
    "MapUpdate_SoldiersTempleWaterPal",
    "MapUpdate_ValleyMazeWaterPal",
    "MapUpdate_StripClubLightsPal",
    "MapUpdate_DezoSnowstormPal",
    "MapUpdate_BioPlantMoveUpdateRate",
    "MapUpdate_BioPlantPal",
    "MapUpdate_MachCntrMoveUpdateRate",
    "MapUpdate_PlateSysMoveUpdateRate",
    "MapUpdate_VahFortConveyorBelts",
    "MapUpdate_WpnPlntConveyorBelts",
    "MapUpdate_WreckagePal",
    "MapUpdate_WreckagePal2",
    "MapUpdate_MachCenterPal",
    "loc_55320",
    "MapUpdate_PlateSysF1Pal",
    "MapUpdate_PlateSysF1Pal2",
    "loc_55484",
    "MapUpdate_ClimCenterPal",
    "MapUpdate_ZelanF1Pal",
    "MapUpdate_NurvusB1Pal",
    "MapUpdate_NurvusB1Pal2",
    "MapUpdate_AirCastlePal",
    "MapUpdate_AirCastlePal2",
    "MapUpdate_WreckageEnginePal",
    "MapUpdate_KuranF2Pal",
    "MapUpdate_GaruberkTowerPart4",
    "MapUpdate_GaruberkTowerPal",
    "MapUpdate_GaruberkTowerPal2",
    "loc_55972",
    "MapUpdate_GaruberkTower",
    "MapUpdate_PlateSystemPal",
    "MapUpdate_NurvusPal",
    "MapUpdate_NurvusPal2",
    "MapUpdate_RykCrystalsPal",
    "MapUpdate_ZelanKuranSpacePal",
    "MapUpdate_LeRoofRoomPal",
    "MapUpdate_ZelanCanceller",
    "MapUpdate_TheEdgePalLine1",
    "MapUpdate_TheEdgePalLine2",
    "MapUpdate_TheEdgePart2_PalLine2",
    "loc_55D1E",
    "loc_55D78",
    "MapUpdate_StrengthTwChests",
    "MapUpdate_CourageTwChests",
    "MapUpdate_VahFortPal",
    "MapUpdate_VahFortPal2",
    "MapUpdate_LadeaTwAirCstlMusic",
    "MapUpdate_ClrChestFlag",
    "MapUpdate_RandomBattlesFlag",
    "MapUpdate_NoUpdate2",
    "MapUpdate_NoUpdate3",
    "MapUpdate_NoUpdate4",
    "MapUpdate_NoUpdate5",
    "MapUpdate_NoUpdate6",
    "MapUpdate_NoUpdate7",
];
const NO_OP: [u8; 4] = [0x70, 0x00, 0x4E, 0x75];

fn refuse<T>(message: impl Into<String>) -> Result<T, PackError> {
    Err(PackError(message.into()))
}

/// `dispatch_table`: from `RunMapUpdates`, not the fork's build addresses.
fn dispatch_table(rom: &[u8]) -> Result<usize, PackError> {
    let hits: Vec<usize> = (0..rom.len().saturating_sub(12))
        .filter(|&at| {
            rom[at..at + 2] == [0x43, 0xF9]
                && rom[at + 6..at + 10] == [0x4E, 0xB1, 0x00, 0x00]
                && rom[at + 10] == 0x66
                && rom[at + 12..at + 14] == [0x60, 0x00]
        })
        .collect();
    let [hit] = hits.as_slice() else {
        return refuse(format!(
            "RunMapUpdates dispatcher occurs {} times, not once",
            hits.len()
        ));
    };
    let table = l(rom, hit + 2)? as usize;
    if self_bounded_bra_table(rom, table, "MapUpdateJmpTbl", 64)? != NAMES.len() {
        return refuse("MapUpdateJmpTbl does not have 64 entries");
    }
    Ok(table)
}

/// Every even offset of `code` holding `opcode`.
fn find(code: &[u8], opcode: u16) -> Vec<usize> {
    (0..code.len().saturating_sub(1))
        .step_by(2)
        .filter(|&at| w(code, at).ok() == Some(opcode))
        .collect()
}

fn one(code: &[u8], opcode: u16) -> Result<usize, PackError> {
    let hits = find(code, opcode);
    let [hit] = hits.as_slice() else {
        return refuse(format!(
            "expected one opcode {opcode:#06x}, found {}",
            hits.len()
        ));
    };
    Ok(*hit)
}

fn write(slot: usize, word: u16) -> Json {
    Json::object().with("slot", slot).with("word", word)
}

/// `_writes`: one frame of `(slot, word)` per step of a ROM table.
fn frames(
    rom: &[u8],
    table: usize,
    reads: &[(usize, usize)],
    length: usize,
    stride: usize,
) -> Result<Vec<Json>, PackError> {
    let Some(furthest) = reads.iter().map(|(source, _)| *source).max() else {
        return refuse("palette reads are empty");
    };
    let end = table + (length - 1) * stride + furthest + 2;
    if end > rom.len() {
        return refuse("palette reads run outside the ROM");
    }
    (0..length)
        .map(|step| {
            let row = reads
                .iter()
                .map(|&(source, slot)| Ok(write(slot, w(rom, table + step * stride + source)?)))
                .collect::<Result<Vec<Json>, PackError>>()?;
            Ok(Json::Array(row))
        })
        .collect()
}

/// `_gates`: an immediate flag id then a call into a flag-test door; the next
/// branch says whether set or clear proceeds. The `$F120` door is extended
/// events and chests: the fork's `ChestFlags` label is not its `$F140` door.
fn gates(code: &[u8], address: usize) -> Result<(Vec<Json>, Option<u16>), PackError> {
    let mut gates = Vec::new();
    let mut bypass = None;
    for at in (0..code.len().saturating_sub(11)).step_by(2) {
        if !matches!(w(code, at)?, 0x303C | 0x103C) {
            continue;
        }
        let (target, branch) = match w(code, at + 4)? {
            0x4EB9 => (i64::from(l(code, at + 6)?), at + 10),
            0x6100 => (
                address as i64 + at as i64 + 6 + i64::from(sw(code, at + 6)?),
                at + 8,
            ),
            _ => continue,
        };
        let bank = match target {
            0x57624 => "event",
            0x5762E => "chest",
            0x57638 => "temp",
            _ => continue,
        };
        let flag = w(code, at + 2)?;
        if bank == "event" && at == 0 && code[branch] == 0x66 && flag == 0xDA {
            // Reunion branches past the shutdown gates.
            bypass = Some(flag);
        } else {
            gates.push(
                Json::object()
                    .with("bank", bank)
                    .with("flag", flag)
                    .with("set", code[branch] == 0x67),
            );
        }
    }
    Ok((gates, bypass))
}

fn palette(rom: &[u8], code: &[u8], address: usize, index: usize) -> Result<Json, PackError> {
    let cram = match find(code, 0x43F8).first() {
        Some(&at) => w(code, at + 2)?,
        None => 0,
    };
    if !matches!(cram, 0xFB00 | 0xFB20) {
        return refuse(format!("update {index:#x} has no CRAM base"));
    }
    let base = usize::from(cram - 0xFB00) / 2;
    let slot_at =
        |at: usize| -> Result<usize, PackError> { Ok(base + usize::from(w(code, at)?) / 2) };
    let mut masks = Vec::new();
    for at in (0..code.len().saturating_sub(3)).step_by(2) {
        if matches!(w(code, at)?, 0x0240 | 0x0241) {
            masks.push((w(code, at + 2)?, at));
        }
    }
    let Some(&(frame_mask, _)) = masks.first() else {
        return refuse("palette clock has no mask");
    };
    let (gates, bypass) = gates(code, address)?;
    let mut stopped = Vec::new();
    for at in find(code, 0x337C) {
        stopped.push(write(slot_at(at + 4)?, w(code, at + 2)?));
    }
    let mut cycles = Vec::new();
    let compares = find(code, 0x0C38);
    for &compare in &compares {
        let length = usize::from(w(code, compare + 2)?);
        let counter = i32::from(w(code, compare + 4)?) - 0xECF2;
        if !matches!(counter, 0 | 1) || !(1..=32).contains(&length) {
            return refuse("unknown palette counter");
        }
        let register: u16 = if index == 0x0E && counter == 1 { 3 } else { 2 };
        let table = l(code, one(code, 0x41F9 + register * 0x200)? + 2)? as usize;
        let mut reads = Vec::new();
        for at in find(code, 0x3370 + register) {
            reads.push((usize::from(w(code, at + 2)? & 0xFF), slot_at(at + 4)?));
        }
        let mut stride = 2;
        if index == 0x0B {
            reads = vec![(0, slot_at(one(code, 0x3352)? + 2)?)];
        }
        if index == 0x2C {
            reads = vec![(0, slot_at(one(code, 0x3341)? + 2)?)];
            stride = 1;
        }
        if reads.is_empty() {
            return refuse("palette cycle has no writes");
        }
        let mut rows = frames(rom, table, &reads, length, stride)?;
        if stride == 1 {
            rows = (0..length)
                .map(|step| Json::Array(vec![write(reads[0].1, u16::from(rom[table + step]))]))
                .collect();
        }
        cycles.push(
            Json::object()
                .with("frame_mask", frame_mask)
                .with("timer", index == 0x0B)
                .with(
                    "phase",
                    Json::object().with("kind", "counter").with("slot", counter),
                )
                .with("frames", rows),
        );
    }
    if compares.is_empty() {
        let table = l(code, one(code, 0x45F9)? + 2)? as usize;
        let [_, (phase_mask, at)] = masks.as_slice() else {
            return refuse("frame-indexed palette has no phase mask");
        };
        let shift_word = w(code, at + 4)?;
        if shift_word & 0xF1FF != 0xE048 {
            return refuse("palette phase is not lsr.w immediate");
        }
        let shift = (shift_word >> 9) & 7;
        let mut reads = Vec::new();
        for at in find(code, 0x3352) {
            reads.push((0, slot_at(at + 2)?));
        }
        for at in find(code, 0x336A) {
            reads.push((usize::from(w(code, at + 2)?), slot_at(at + 4)?));
        }
        for at in find(code, 0x3372) {
            reads.push((usize::from(w(code, at + 2)? & 0xFF), slot_at(at + 4)?));
        }
        // Air Castle $1F masks the already-zero d0; retail never reloads the
        // frame count. The fork's optional fix is deliberately not applied.
        let air_castle = index == 0x1F;
        let length = if air_castle {
            1
        } else {
            usize::from(phase_mask >> shift) / 2 + 1
        };
        cycles.push(
            Json::object()
                .with("frame_mask", frame_mask)
                .with("timer", false)
                .with(
                    "phase",
                    Json::object()
                        .with("kind", "frame")
                        .with("mask", if air_castle { 0 } else { *phase_mask })
                        .with("shift", shift + 1),
                )
                .with("frames", frames(rom, table, &reads, length, 2)?),
        );
    }
    Ok(Json::object()
        .with("kind", "palette")
        .with("gates", gates)
        .with("bypass", bypass)
        .with("cycles", cycles)
        .with("stopped", stopped))
}

fn words_at(rom: &[u8], table: usize, count: usize) -> Result<Vec<u16>, PackError> {
    (0..count).map(|i| Ok(w(rom, table + i * 2)?)).collect()
}

/// `_program`: what one routine does, read from its code.
fn program(rom: &[u8], code: &[u8], address: usize, index: usize) -> Result<Json, PackError> {
    let kind = |kind: &str| Json::object().with("kind", kind);
    if index == 0 || index >= 0x3A {
        if code != NO_OP {
            return refuse("no-update routine is no longer a no-op");
        }
        return Ok(kind("noop"));
    }
    match index {
        0x11 | 0x12 => Ok(kind("unsupported").with(
            "missing",
            "live Map_Layout_FG byte buffer, conveyor chunk definitions, plane refresh/DMA",
        )),
        0x22 => Ok(kind("unsupported").with(
            "missing",
            "shared RAM_Start pattern staging buffer and tile DMA",
        )),
        5 | 7 | 0x0D | 0x0F | 0x10 | 0x25 => {
            let mut shifts = Vec::new();
            for at in (0..code.len().saturating_sub(1)).step_by(2) {
                let word = w(code, at)?;
                if word & 0xF1FF == 0xE080 {
                    shifts.push((word >> 9) & 7);
                }
            }
            let [x_shift, y_shift] = shifts.as_slice() else {
                return refuse("camera rate routine has no two arithmetic shifts");
            };
            Ok(kind("camera")
                .with("foreground", index == 0x25)
                .with("x_shift", *x_shift)
                .with("y_shift", *y_shift))
        }
        6 => Ok(kind("tunnels")),
        0x26 | 0x31 | 0x32 => {
            const SINE: u32 = 0x423B4;
            if !find(code, 0x4EB9)
                .into_iter()
                .any(|at| l(code, at + 2).ok() == Some(SINE))
            {
                return refuse("scroll routine does not call the sine helper");
            }
            let routine = SINE as usize;
            // move.b (d8,pc,d0.w),d0 at the positive sine door.
            if w(rom, routine + 4)? != 0x103B {
                return refuse("sine helper has changed");
            }
            let table = routine + 6 + usize::from(w(rom, routine + 6)? & 0xFF);
            let Some(positive) = rom.get(table..table + 128) else {
                return refuse("sine table runs outside the ROM");
            };
            let mut samples: Vec<u8> = positive.to_vec();
            samples.extend((0..128).rev().map(|i| positive[i].wrapping_neg()));
            let mode = match index {
                0x26 => "unsigned",
                0x31 => "signed",
                _ => "vertical",
            };
            Ok(kind("scroll").with("mode", mode).with("samples", samples))
        }
        0x2A => {
            let table = l(code, one(code, 0x45F9)? + 2)? as usize;
            let Some(&base_write) = find(code, 0x31FC).first() else {
                return refuse("crystals routine has no resting colour");
            };
            Ok(kind("crystals")
                .with("colors", words_at(rom, table, 4)?)
                .with("rest", w(code, base_write + 2)?))
        }
        0x2D | 0x33 | 0x34 | 0x38 => {
            let mut ids = Vec::new();
            for at in find(code, 0x103C) {
                ids.push(w(code, at + 2)?);
            }
            let Some((flag, chests)) = ids.split_last() else {
                return refuse("flag routine loads no flag");
            };
            let chests = if index == 0x38 {
                Vec::new()
            } else {
                chests.to_vec()
            };
            Ok(kind("flags")
                .with("chests", chests)
                .with("flag", *flag)
                .with("clear_chest", index == 0x38))
        }
        0x2E => {
            let table = l(code, one(code, 0x45F9)? + 2)? as usize;
            Ok(kind("edge").with("colors", words_at(rom, table, 5)?))
        }
        0x2F | 0x30 => Ok(kind("edge_line")
            .with("sliding", index == 0x30)
            .with("frames", Json::Array(Vec::new()))),
        0x37 => Ok(kind("music")
            .with("secondary_map", w(code, one(code, 0x31FC)? + 2)?)
            .with("sound", w(code, one(code, 0x11FC)? + 2)?)),
        0x39 => Ok(kind("encounters").with("y_max", w(code, one(code, 0x0C40)? + 2)?)),
        _ => palette(rom, code, address, index),
    }
}

/// The Edge's line programs read their frames from the map's own palette blob,
/// `$60` past its pointer.
fn edge_line_frames(rom: &[u8], record: &MapRecord, index: usize) -> Result<Json, PackError> {
    let table = record.palette.pointer + 0x60;
    let (length, stride) = if index == 0x30 { (14, 2) } else { (28, 28) };
    let mut rows = Vec::with_capacity(length);
    for step in 0..length {
        let at = table + step * stride;
        let mut row = vec![write(0, w(rom, at)?)];
        for color in 0..14 {
            row.push(write(16 + color, w(rom, at + color * 2)?));
        }
        rows.push(Json::Array(row));
    }
    Ok(Json::Array(rows))
}

/// What [`extract_map_updates`] returns.
pub struct MapUpdates {
    /// By map id, for every real map: its ordered programs.
    pub per_map: BTreeMap<usize, Vec<Json>>,
    pub census: Json,
}

/// `extract_map_updates`: all 64 routines, bound to every real map.
pub fn extract_map_updates<'a>(
    rom: &[u8],
    maps: impl Iterator<Item = &'a MapRecord>,
) -> Result<MapUpdates, PackError> {
    let table = dispatch_table(rom)?;
    let mut addresses = Vec::with_capacity(NAMES.len());
    for index in 0..NAMES.len() {
        addresses.push(bra_target(rom, table + index * 4)?);
    }
    if !addresses.windows(2).all(|pair| pair[0] < pair[1]) {
        return refuse("retail routine boundaries are not strictly increasing");
    }
    let mut catalog = Vec::with_capacity(NAMES.len());
    for (index, &address) in addresses.iter().enumerate() {
        let end = addresses.get(index + 1).copied().unwrap_or(address + 4);
        let Some(region) = rom.get(address..end) else {
            return refuse(format!("update {index:#x} runs outside the ROM"));
        };
        let last = (0..region.len().saturating_sub(3))
            .step_by(2)
            .rfind(|&at| region[at..at + 4] == NO_OP);
        let Some(last) = last else {
            return refuse(format!("update {index:#x} has no retail zero return"));
        };
        let code = &region[..last + 4];
        catalog.push(
            Json::object()
                .with("index", index)
                .with("routine", NAMES[index])
                .with("address", hex6(address))
                .with("program", program(rom, code, address, index)?)
                .with("code_sha256", hex(&Sha256::digest(code))),
        );
    }
    let mut per_map = BTreeMap::new();
    let mut uses: Vec<Vec<usize>> = vec![Vec::new(); NAMES.len()];
    for record in maps {
        let mut entries = Vec::with_capacity(record.map_updates.ids.len());
        for &id in &record.map_updates.ids {
            let index = usize::from(id);
            let Some(entry) = catalog.get(index) else {
                return refuse(format!("map {}: update {index} out of bounds", record.id));
            };
            let mut entry = entry.clone();
            if matches!(index, 0x2F | 0x30) {
                let frames = edge_line_frames(rom, record, index)?;
                if let Json::Object(pairs) = &mut entry
                    && let Some((_, program)) = pairs.iter_mut().find(|(key, _)| key == "program")
                {
                    program.insert("frames", frames);
                }
            }
            entries.push(entry);
            uses[index].push(record.id);
        }
        per_map.insert(record.id, entries);
    }
    let routines: Vec<Json> = catalog
        .into_iter()
        .zip(uses)
        .map(|(entry, maps)| entry.with("maps", maps))
        .collect();
    let census = Json::object()
        .with("jump_table", hex6(table))
        .with("routine_count", NAMES.len())
        .with("map_count", per_map.len())
        .with("routines", routines);
    Ok(MapUpdates { per_map, census })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rom_without_the_dispatcher_is_refused() {
        let error = dispatch_table(&[0; 64]).unwrap_err();
        assert!(error.0.contains("occurs 0 times"), "{}", error.0);
    }

    #[test]
    fn a_no_update_routine_must_be_the_no_op() {
        assert_eq!(
            program(&[], &NO_OP, 0, 0).unwrap(),
            Json::object().with("kind", "noop")
        );
        let error = program(&[], &[0x4E, 0x75], 0, 0x3A).unwrap_err();
        assert!(error.0.contains("no longer a no-op"), "{}", error.0);
    }

    #[test]
    fn flags_take_the_last_id_and_a_clear_routine_lists_no_chests() {
        let code = [
            0x10, 0x3C, 0x00, 0x01, 0x10, 0x3C, 0x00, 0x02, 0x10, 0x3C, 0x00, 0x03,
        ];
        let flags = |index| program(&[], &code, 0, index).unwrap();
        assert_eq!(
            flags(0x33),
            Json::object()
                .with("kind", "flags")
                .with("chests", vec![1u16, 2])
                .with("flag", 3u16)
                .with("clear_chest", false)
        );
        assert_eq!(
            flags(0x38),
            Json::object()
                .with("kind", "flags")
                .with("chests", Vec::<u16>::new())
                .with("flag", 3u16)
                .with("clear_chest", true)
        );
    }

    #[test]
    fn a_camera_routine_needs_exactly_two_shifts() {
        // asr.l #2,d0 and asr.l #3,d0.
        let code = [0xE4, 0x80, 0xE6, 0x80];
        assert_eq!(
            program(&[], &code, 0, 0x25).unwrap(),
            Json::object()
                .with("kind", "camera")
                .with("foreground", true)
                .with("x_shift", 2u16)
                .with("y_shift", 3u16)
        );
        assert!(program(&[], &code[..2], 0, 5).is_err());
    }

    #[test]
    fn frames_read_each_step_and_refuse_a_table_off_the_rom() {
        let rom = [0, 1, 0, 2, 0, 3, 0, 4];
        let rows = frames(&rom, 0, &[(0, 5), (2, 6)], 2, 4).unwrap();
        assert_eq!(
            rows,
            [
                Json::Array(vec![write(5, 1), write(6, 2)]),
                Json::Array(vec![write(5, 3), write(6, 4)]),
            ]
        );
        assert!(frames(&rom, 0, &[(0, 5)], 5, 2).is_err());
        assert!(frames(&rom, 0, &[], 1, 2).is_err());
    }

    #[test]
    fn a_reunion_gate_is_a_bypass_and_others_are_gates() {
        // move.w #$DA,d0 / jsr event test / bne, then move.b #4,d0 / jsr chest
        // test / beq, with padding so the scan reaches both.
        let code = [
            0x30, 0x3C, 0x00, 0xDA, 0x4E, 0xB9, 0x00, 0x05, 0x76, 0x24, 0x66, 0x00, 0x10, 0x3C,
            0x00, 0x04, 0x4E, 0xB9, 0x00, 0x05, 0x76, 0x2E, 0x67, 0x00, 0x4E, 0x75, 0x4E, 0x75,
        ];
        let (found, bypass) = gates(&code, 0).unwrap();
        assert_eq!(bypass, Some(0xDA));
        assert_eq!(
            found,
            [Json::object()
                .with("bank", "chest")
                .with("flag", 4u16)
                .with("set", true)]
        );
    }
}
