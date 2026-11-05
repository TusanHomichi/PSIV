//! The pre-control chain: `RunEventsJmpTbl`'s triggers, the flag setters, the
//! scenes the chain runs, and the flag banks at the first controllable moment.

use std::collections::BTreeSet;

use super::{
    CHARACTER_1, EVENT_FLAGS_SET, EVENT_FLAGS_TEST, EVENT_INDEX, FLAG_BANK_BYTES, FLAG_BANKS,
    FLAG_SETTER_STRIDE, GAME_START_EVENT_INDEX, RUN_EVENTS_JMP_TBL_SITE, Read, event_routine,
    event_routines, flag_symbol, game_start_epilogue, long, refuse, word,
};
use crate::PackError;
use crate::json::Json;
use crate::maps::records::MapRecord;
use crate::png::hex;

pub(super) fn run_events_jmp_tbl(rom: &[u8]) -> Result<usize, PackError> {
    let site = RUN_EVENTS_JMP_TBL_SITE;
    if rom.get(site..site + 2) != Some(&[0x45, 0xFA]) {
        return refuse(format!(
            "0x{site:06X} is not the `lea (d16,PC),a2` that names RunEventsJmpTbl"
        ));
    }
    Ok((site as i64 + 2 + i64::from(word(rom, site + 2) as i16)) as usize)
}

/// One `RunEventsJmpTbl` entry: what gates it and what it plays.
pub(super) struct Trigger {
    pub(super) index: u16,
    pub(super) rom_offset: usize,
    pub(super) event: Option<u16>,
    pub(super) gates: Vec<(u16, bool)>,
    pub(super) position_gated: bool,
}

impl Trigger {
    /// A position-gated trigger never fires before control: the party hasn't
    /// moved.
    pub(super) fn fires(&self, flags: &BTreeSet<u16>) -> bool {
        self.event.is_some()
            && !self.position_gated
            && self
                .gates
                .iter()
                .all(|(flag, wanted)| flags.contains(flag) == *wanted)
    }

    pub(super) fn to_json(&self) -> Json {
        Json::object()
            .with("index", self.index)
            .with("routine", format!("0x{:06X}", self.rom_offset))
            .with("event", self.event.map(|e| format!("0x{e:02X}")))
            .with(
                "gates",
                self.gates
                    .iter()
                    .map(|(flag, wanted)| {
                        Json::object()
                            .with("flag", *flag)
                            .with("flag_hex", format!("0x{flag:02X}"))
                            .with("required", if *wanted { "set" } else { "clear" })
                    })
                    .collect::<Vec<_>>(),
            )
            .with("position_gated", self.position_gated)
    }
}

/// `_preceding_d0`: the flag id loaded into d0 just before a call.
pub(super) fn preceding_d0(rom: &[u8], call: usize) -> Result<u16, PackError> {
    if rom[call - 2] == 0x70 {
        return Ok(u16::from(rom[call - 1]));
    }
    if rom[call - 4..call - 2] == [0x30, 0x3C] {
        return Ok(word(rom, call - 2));
    }
    refuse(format!(
        "the call at 0x{call:06X} is not preceded by a `moveq`/`move.w` into d0"
    ))
}

pub(super) fn read_trigger(rom: &[u8], index: u16) -> Result<Trigger, PackError> {
    let entry = run_events_jmp_tbl(rom)? + usize::from(index) * 4;
    if word(rom, entry) != 0x6000 {
        return refuse(format!(
            "RunEventsJmpTbl entry {index} at 0x{entry:06X} is not a `bra.w`"
        ));
    }
    let start = (entry as i64 + 2 + i64::from(word(rom, entry + 2) as i16)) as usize;
    let (mut event, mut gates, mut position_gated) = (None, Vec::new(), false);
    let mut pos = start;
    loop {
        if pos >= start + 0x100 {
            return refuse(format!(
                "trigger {index} at 0x{start:06X} has no `rts` within 256 bytes"
            ));
        }
        let op = word(rom, pos);
        if op == 0x4E75 {
            break;
        }
        if op & 0xF1FF == 0x41F8 && word(rom, pos + 2) == CHARACTER_1 {
            position_gated = true;
        }
        if op == 0x31FC && word(rom, pos + 4) == EVENT_INDEX {
            event = Some(word(rom, pos + 2));
        }
        if op == 0x4EB9 && long(rom, pos + 2) == EVENT_FLAGS_TEST {
            let flag = preceding_d0(rom, pos)?;
            match rom[pos + 6] {
                0x66 => gates.push((flag, false)),
                0x67 => gates.push((flag, true)),
                branch => {
                    return refuse(format!(
                        "trigger {index}: the flag test at 0x{pos:06X} is followed by 0x{branch:02X}, \
                         neither `beq` nor `bne`"
                    ));
                }
            }
        }
        pos += 2;
    }
    Ok(Trigger {
        index,
        rom_offset: start,
        event,
        gates,
        position_gated,
    })
}

/// `flag_setters`: one routine with four doors, each picking a bank by `lea`.
pub(super) fn flag_setters(rom: &[u8]) -> Result<Vec<(usize, &'static str)>, PackError> {
    let mut setters = Vec::with_capacity(FLAG_BANKS.len());
    for index in 0..FLAG_BANKS.len() {
        let address = EVENT_FLAGS_SET + index * FLAG_SETTER_STRIDE;
        if rom.get(address..address + 4) != Some(&[0x48, 0xE7, 0xE0, 0x80]) {
            return refuse(format!(
                "0x{address:06X} is not a flag setter's `movem.l d0-d2/a0,-(sp)`"
            ));
        }
        if rom.get(address + 4..address + 6) != Some(&[0x41, 0xF8]) {
            return refuse(format!("0x{address:06X} does not pick a bank with `lea`"));
        }
        let bank = word(rom, address + 6);
        let Some((_, name)) = FLAG_BANKS.iter().find(|(at, _)| *at == bank) else {
            return refuse(format!(
                "the setter at 0x{address:06X} writes 0xFFFF{bank:04X}, which is not one of the four \
                 flag banks"
            ));
        };
        setters.push((address, *name));
    }
    let names: BTreeSet<&str> = setters.iter().map(|(_, n)| *n).collect();
    if names.len() != FLAG_BANKS.len() {
        return refuse("the four setters do not cover the four banks".into());
    }
    Ok(setters)
}

/// `scene_flags`: every flag a scene sets, in any bank, in address order.
pub(super) fn scene_flags(
    rom: &[u8],
    start: usize,
    end: usize,
) -> Result<Vec<(&'static str, Read)>, PackError> {
    let mut found = Vec::new();
    for (address, bank) in flag_setters(rom)? {
        for call in [0x4EB9u16, 0x4EF9] {
            let mut pattern = call.to_be_bytes().to_vec();
            pattern.extend_from_slice(&(address as u32).to_be_bytes());
            let mut offset = start;
            while offset + pattern.len() <= end {
                let Some(at) = rom[offset..end].windows(6).position(|w| w == pattern) else {
                    break;
                };
                let at = offset + at;
                found.push((
                    bank,
                    Read {
                        label: format!("{bank} set at 0x{at:06X}"),
                        rom_offset: at,
                        raw_hex: hex(&rom[at - 4..at + 6]),
                        values: vec![u64::from(preceding_d0(rom, at)?)],
                    },
                ));
                offset = at + 2;
            }
        }
    }
    found.sort_by_key(|(_, read)| read.rom_offset);
    Ok(found)
}

/// `event_routine_bounds`: the next routine by address, or the last one's
/// tail jump to `EventFlags_Set`.
pub(super) fn event_routine_bounds(rom: &[u8], index: u16) -> Result<(usize, usize), PackError> {
    let start = event_routine(rom, index)?;
    if let Some(next) = event_routines(rom)?
        .into_iter()
        .filter(|&a| a > start)
        .min()
    {
        return Ok((start, next));
    }
    let mut tail = vec![0x4E, 0xF9];
    tail.extend_from_slice(&(EVENT_FLAGS_SET as u32).to_be_bytes());
    let span = &rom[start..(start + 0x100).min(rom.len())];
    match span.windows(6).position(|w| w == tail) {
        Some(at) => Ok((start, start + at + 6)),
        None => refuse(format!(
            "event 0x{index:02X} at 0x{start:06X} is the last routine and does not end in a tail \
             jump to EventFlags_Set, so its extent is unknown"
        )),
    }
}

/// `_pack_flags`: the inverse of [`flag_ids`].
pub(super) fn pack_flags(flags: &BTreeSet<u16>, base: u16) -> Result<Vec<u8>, PackError> {
    let mut packed = vec![0u8; FLAG_BANK_BYTES];
    for &flag in flags {
        let index = usize::from(flag.wrapping_sub(base) >> 3);
        if flag < base || index >= FLAG_BANK_BYTES {
            return refuse(format!(
                "flag 0x{flag:03X} is outside the {FLAG_BANK_BYTES}-byte bank that starts at 0x{base:03X}"
            ));
        }
        packed[index] |= 1 << (7 - ((flag - base) & 7));
    }
    Ok(packed)
}

/// `read_first_control`: walk the start map's triggers until none fires.
pub(super) fn read_first_control(
    rom: &[u8],
    record: &MapRecord,
    extended: &[u16],
    town: &[u16],
) -> Result<Json, PackError> {
    let mut scenes: Vec<Json> = Vec::new();
    let mut played: Vec<u16> = Vec::new();
    let mut reads: Vec<Read> = Vec::new();
    let mut banks: [(&str, BTreeSet<u16>); 4] = [
        ("event_flags", BTreeSet::new()),
        ("extended_event_flags", extended.iter().copied().collect()),
        ("chest_flags", BTreeSet::new()),
        ("town_flags", town.iter().copied().collect()),
    ];
    let mut epilogue: Option<Json> = None;
    let mut index = Some(GAME_START_EVENT_INDEX);
    while let Some(current) = index {
        let (start, end) = event_routine_bounds(rom, current)?;
        let sets = scene_flags(rom, start, end)?;
        reads.extend(sets.iter().map(|(_, read)| read.clone()));
        if current == GAME_START_EVENT_INDEX {
            let (json, epilogue_reads, _) = game_start_epilogue(rom, start, end)?;
            epilogue = Some(json);
            reads.extend(epilogue_reads);
        }
        let mut sets_json = Vec::with_capacity(sets.len());
        for (bank, read) in &sets {
            let flag = read.value()? as u16;
            sets_json.push(
                Json::object()
                    .with("bank", *bank)
                    .with("flag", flag)
                    .with("flag_hex", format!("0x{flag:02X}"))
                    .with(
                        "symbol",
                        if *bank == "event_flags" {
                            flag_symbol(flag)
                        } else {
                            None
                        },
                    )
                    .with("at", format!("0x{:06X}", read.rom_offset)),
            );
            if let Some((_, set)) = banks.iter_mut().find(|(name, _)| name == bank) {
                set.insert(flag);
            }
        }
        scenes.push(
            Json::object()
                .with("event", current)
                .with("event_hex", format!("0x{current:02X}"))
                .with("routine", format!("0x{start:06X}"))
                .with("rom_end", format!("0x{end:06X}"))
                .with("sets", sets_json),
        );
        played.push(current);
        // `_next_scene`: the start map's trigger list.
        let triggers = record
            .events
            .ids
            .iter()
            .map(|&id| read_trigger(rom, id))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(last) = scenes.last_mut() {
            last.insert(
                "triggers_after",
                triggers.iter().map(Trigger::to_json).collect::<Vec<_>>(),
            );
        }
        index = None;
        for trigger in &triggers {
            if trigger.fires(&banks[0].1) {
                let event = trigger.event.unwrap_or(0);
                if played.contains(&event) {
                    return refuse(format!(
                        "trigger {} would replay event 0x{event:02X}; the pre-control chain does not \
                         terminate",
                        trigger.index
                    ));
                }
                index = Some(event);
                break;
            }
        }
    }
    let Some(mut out) = epilogue else {
        return refuse("the chain did not start at Event_GameStart".into());
    };
    out.insert("scene_chain", scenes);
    out.insert(
        "event_flags_set",
        banks[0]
            .1
            .iter()
            .map(|&flag| {
                Json::object()
                    .with("id", flag)
                    .with("id_hex", format!("0x{flag:02X}"))
                    .with("symbol", flag_symbol(flag))
            })
            .collect::<Vec<_>>(),
    );
    out.insert(
        "extended_event_flags_set",
        banks[1].1.iter().copied().collect::<Vec<_>>(),
    );
    out.insert(
        "town_flags_set",
        banks[3].1.iter().copied().collect::<Vec<_>>(),
    );
    out.insert(
        "chest_flags_set",
        banks[2].1.iter().copied().collect::<Vec<_>>(),
    );
    let mut flag_banks = Json::object();
    for ((address, _), (name, set)) in FLAG_BANKS.iter().zip(&banks) {
        let base = if *name == "extended_event_flags" {
            0x100
        } else {
            0
        };
        let packed = pack_flags(set, base)?;
        flag_banks.insert(
            *name,
            Json::object()
                .with("address", format!("0xFFFF{address:04X}"))
                .with("first_id", base)
                .with("bytes", FLAG_BANK_BYTES)
                .with("raw_hex", hex(&packed))
                .with("first_long", format!("0x{:08X}", long(&packed, 0)))
                .with(
                    "set",
                    set.iter().map(|f| format!("0x{f:03X}")).collect::<Vec<_>>(),
                ),
        );
    }
    out.insert("flag_banks", flag_banks);
    out.insert("reads", reads.iter().map(Read::to_json).collect::<Vec<_>>());
    Ok(out)
}
