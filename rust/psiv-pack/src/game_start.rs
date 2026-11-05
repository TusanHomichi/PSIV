//! `game_start.json`, where a fresh playthrough begins: the port of
//! `psiv_tools/newgame.py`, which owns the retail citations (#216, slice 5).
//!
//! Nothing here is a constant with a comment claiming provenance: every value
//! is read from an instruction sequence pinned by its own opcode bytes, and a
//! site that doesn't match fails. The state emitted is the first controllable
//! moment, after the two scenes the chain runs before control.
//!
//! The file is `psiv_data::GameStartFile`, so what the game reads is built as
//! its types. Everything else (the reads, the chain, `first_control`) has no
//! typed home yet and is laid over it.

use std::collections::BTreeSet;
use std::path::Path;

use psiv_data::{
    FlagList, GameStartFile, GameStartMap, HandoffEvent, NewGameFlags, NewGameInit,
    NewGameSettings, PartySlot, TitleHandoff,
};
use sha2::{Digest, Sha256};

use crate::decode::Decodes;
use crate::json::{Json, overlay};
use crate::map_symbols::{MAP_SYMBOLS, event_flag_symbol};
use crate::maps::extract_maps;
use crate::maps::records::MapRecord;
use crate::png::hex;
use crate::sound_defs::MUSIC_NAMES;
use crate::{PackError, Rom};

const GAME_MODE_ROUTINE: u16 = 0xEC20;
const FIELD_MAP_INDEX: u16 = 0xEC28;
const FIELD_MAP_INDEX_2: u16 = 0xEC2A;
const MAP_START_FACING_DIR: u16 = 0xEC44;
const MAP_START_CHAR_ALIGN: u16 = 0xEC46;
const MAP_START_X_POS: u16 = 0xEC48;
const MAP_START_Y_POS: u16 = 0xEC4A;
const EVENT_INDEX: u16 = 0xECA8;
const SAVED_SOUND_INDEX: u16 = 0xECEC;
const EVENT_FLAGS: u16 = 0xF100;
const EXTENDED_EVENT_FLAGS: u16 = 0xF120;
const CHEST_FLAGS: u16 = 0xF140;
const TOWN_FLAGS: u16 = 0xF160;
const WORLD_INDEX: u16 = 0xF400;
const CURRENT_PARTY_SLOTS: u16 = 0xF40A;
const CURRENT_PARTY_SLOT_5: u16 = 0xF40E;
const INVENTORY: u16 = 0xF410;
const CURRENT_MONEY: u16 = 0xF438;
const MESSAGE_SPEED: u16 = 0xF440;
const BATTLE_SPEED: u16 = 0xF442;
const MACRO_DATA: u16 = 0xF444;
const CHARACTER_STATS: u16 = 0xF500;
const PARTY_SLOTS: usize = 6;
const EMPTY_SLOT: u8 = 0xFF;
const CHARACTER_SYMBOLS: [&str; 11] = [
    "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika", "Demi", "Wren", "Raja", "Kyra", "Seth",
];
const START_POS_PIXELS: u64 = 8;
const COLLISION_CELL_PIXELS: u64 = 16;
const STANDING_CELL_Y_OFFSET: u64 = 1;
const TITLE_SET_MAP: usize = 0x04335C;
const NEW_GAME_INIT: usize = 0x044414;
const INIT_CHAR_STATS: usize = 0x044652;
const TITLE_START_OPTION_HANDOFF: usize = 0x043788;
const EVENT_DISPATCH: usize = 0x05A27A;
const EVENT_PTRS: usize = 0x05A2B4;
const EVENT_FLAGS_SET: usize = 0x057666;
const EVENT_FLAGS_TEST: u32 = 0x057624;
/// The four banks, by the RAM their setter's `lea` names.
const FLAG_BANKS: [(u16, &str); 4] = [
    (EVENT_FLAGS, "event_flags"),
    (EXTENDED_EVENT_FLAGS, "extended_event_flags"),
    (CHEST_FLAGS, "chest_flags"),
    (TOWN_FLAGS, "town_flags"),
];
const FLAG_SETTER_STRIDE: usize = 0x0A;
const GAME_START_EVENT_INDEX: u16 = 0x9F;
/// The `lea (d16,PC),a2` inside `RunEvents` that names `RunEventsJmpTbl`.
const RUN_EVENTS_JMP_TBL_SITE: usize = 0x0560CA;
const CHARACTER_1: u16 = 0xC000;
const FLAG_BANK_BYTES: usize = 0x20;
/// `PACK_FORMAT_VERSION`.
const FORMAT_VERSION: u32 = 2;

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn word(rom: &[u8], at: usize) -> u16 {
    rom.get(at..at + 2)
        .map_or(0, |b| u16::from_be_bytes([b[0], b[1]]))
}

fn long(rom: &[u8], at: usize) -> u32 {
    rom.get(at..at + 4)
        .map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// The flag symbols the chain needs: the shared table, and the two the
/// pre-control chain sets.
fn flag_symbol(flag: u16) -> Option<&'static str> {
    match flag {
        0x07 => Some("EventFlag_PiataFirstTime"),
        0x15 => Some("EventFlag_PiataChazControl"),
        _ => event_flag_symbol(flag),
    }
}

/// A byte pattern: literals and captured runs of any byte.
enum Part {
    Lit(Vec<u8>),
    Any(usize),
}

fn lit(bytes: &[u8]) -> Part {
    Part::Lit(bytes.to_vec())
}

fn ram(address: u16) -> Part {
    Part::Lit(address.to_be_bytes().to_vec())
}

/// One instruction sequence found in the image, and what it held.
#[derive(Clone, Debug)]
struct Read {
    label: String,
    rom_offset: usize,
    raw_hex: String,
    values: Vec<u64>,
}

impl Read {
    fn value(&self) -> Result<u64, PackError> {
        match self.values[..] {
            [value] => Ok(value),
            _ => refuse(format!(
                "{} captured {} values, not one",
                self.label,
                self.values.len()
            )),
        }
    }

    fn to_json(&self) -> Json {
        Json::object()
            .with("label", self.label.as_str())
            .with("rom_offset", format!("0x{:06X}", self.rom_offset))
            .with("raw_hex", self.raw_hex.as_str())
    }
}

/// `_read`: the pattern must match exactly `expect` times in the window, as
/// `re.finditer` scans (left to right, a match resuming after itself).
fn read_pattern(
    rom: &[u8],
    parts: &[Part],
    label: &str,
    window: Option<(usize, usize)>,
    expect: usize,
    pick: usize,
) -> Result<Read, PackError> {
    let (start, end) = window.unwrap_or((0, rom.len()));
    let span = &rom[start.min(rom.len())..end.min(rom.len())];
    let length: usize = parts
        .iter()
        .map(|p| match p {
            Part::Lit(b) => b.len(),
            Part::Any(n) => *n,
        })
        .sum();
    let mut hits = Vec::new();
    let mut at = 0;
    while at + length <= span.len() {
        let mut cursor = at;
        let mut values = Vec::new();
        let mut ok = true;
        for part in parts {
            match part {
                Part::Lit(bytes) => {
                    if span[cursor..cursor + bytes.len()] != bytes[..] {
                        ok = false;
                        break;
                    }
                    cursor += bytes.len();
                }
                Part::Any(n) => {
                    values.push(
                        span[cursor..cursor + n]
                            .iter()
                            .fold(0u64, |v, &b| (v << 8) | u64::from(b)),
                    );
                    cursor += n;
                }
            }
        }
        if ok {
            hits.push((at, values));
            at += length;
        } else {
            at += 1;
        }
    }
    if hits.len() != expect {
        let place = window.map_or(String::new(), |(s, e)| {
            format!(" within 0x{s:06X}..0x{e:06X}")
        });
        return refuse(format!(
            "{label}: expected {expect} match(es){place}, found {}",
            hits.len()
        ));
    }
    let (at, values) = hits.swap_remove(pick);
    Ok(Read {
        label: label.to_owned(),
        rom_offset: start + at,
        raw_hex: hex(&span[at..at + length]),
        values,
    })
}

fn move_w(
    rom: &[u8],
    address: u16,
    label: &str,
    window: Option<(usize, usize)>,
) -> Result<Read, PackError> {
    read_pattern(
        rom,
        &[lit(&[0x31, 0xFC]), Part::Any(2), ram(address)],
        label,
        window,
        1,
        0,
    )
}

fn move_b(
    rom: &[u8],
    address: u16,
    label: &str,
    window: Option<(usize, usize)>,
) -> Result<Read, PackError> {
    read_pattern(
        rom,
        &[lit(&[0x11, 0xFC, 0x00]), Part::Any(1), ram(address)],
        label,
        window,
        1,
        0,
    )
}

fn move_l(
    rom: &[u8],
    address: u16,
    label: &str,
    window: (usize, usize),
    expect: usize,
    pick: usize,
) -> Result<Read, PackError> {
    read_pattern(
        rom,
        &[lit(&[0x21, 0xFC]), Part::Any(4), ram(address)],
        label,
        Some(window),
        expect,
        pick,
    )
}

/// `_flag_ids`: flag `n` is byte `n >> 3`, bit `7 - (n & 7)`.
fn flag_ids(data: &[u8], base: u16) -> Vec<u16> {
    data.iter()
        .enumerate()
        .flat_map(|(index, byte)| {
            (0..8u16)
                .filter(move |bit| byte >> (7 - bit) & 1 != 0)
                .map(move |bit| base + index as u16 * 8 + bit)
        })
        .collect()
}

fn character(slot: u8) -> Result<Json, PackError> {
    if slot == EMPTY_SLOT {
        return Ok(Json::object()
            .with("id", Json::Null)
            .with("symbol", Json::Null)
            .with("empty", true));
    }
    let Some(symbol) = CHARACTER_SYMBOLS.get(usize::from(slot)) else {
        return refuse(format!(
            "party slot holds character id 0x{slot:02X}, outside the {} the cartridge defines",
            CHARACTER_SYMBOLS.len()
        ));
    };
    Ok(Json::object()
        .with("id", slot)
        .with("symbol", *symbol)
        .with("empty", false))
}

fn map_reference(map_id: u64) -> Json {
    Json::object()
        .with("id", map_id)
        .with("id_hex", format!("0x{map_id:03X}"))
        .with("symbol", MAP_SYMBOLS.get(map_id as usize).copied())
}

fn music(value: u64) -> Json {
    let symbol = (value as i64 - 0x81)
        .try_into()
        .ok()
        .and_then(|i: usize| MUSIC_NAMES.get(i).copied());
    Json::object()
        .with("id", value)
        .with("id_hex", format!("0x{value:02X}"))
        .with("symbol", symbol)
}

fn event_ptrs(rom: &[u8]) -> Result<usize, PackError> {
    let lea = read_pattern(
        rom,
        &[
            lit(&[0x48, 0xE7, 0xFF, 0xF8, 0x41, 0xFA]),
            Part::Any(2),
            lit(&[0xD0, 0x40, 0xD0, 0x40, 0x20, 0x70, 0x00, 0x00, 0x4E, 0x90]),
        ],
        "loc_5A27A: lea EventPtrs(pc),a0",
        Some((EVENT_DISPATCH, EVENT_DISPATCH + 20)),
        1,
        0,
    )?;
    let base = lea.rom_offset + 6 + lea.value()? as usize;
    if base != EVENT_PTRS {
        return refuse(format!(
            "EventPtrs resolves to 0x{base:06X}, not the expected 0x{EVENT_PTRS:06X}"
        ));
    }
    Ok(base)
}

/// `event_routines`: entries up to the first that isn't a ROM address.
fn event_routines(rom: &[u8]) -> Result<Vec<usize>, PackError> {
    let base = event_ptrs(rom)?;
    let mut routines = Vec::new();
    loop {
        let value = long(rom, base + routines.len() * 4) as usize;
        if value == 0 || value >= rom.len() {
            break;
        }
        routines.push(value);
        if routines.len() > 0x200 {
            return refuse("EventPtrs does not end within 512 entries".into());
        }
    }
    let first = routines.iter().copied().min().unwrap_or(0);
    if base + routines.len() * 4 > first {
        return refuse(format!(
            "EventPtrs runs to 0x{:06X}, past its own first routine at 0x{first:06X}",
            base + routines.len() * 4
        ));
    }
    Ok(routines)
}

fn event_routine(rom: &[u8], index: u16) -> Result<usize, PackError> {
    let routines = event_routines(rom)?;
    routines.get(usize::from(index)).copied().ok_or_else(|| {
        PackError(format!(
            "event 0x{index:02X} is outside the {}-entry EventPtrs table",
            routines.len()
        ))
    })
}

fn read_title_handoff(rom: &[u8]) -> Result<(TitleHandoff, Json), PackError> {
    let scene_map = move_w(
        rom,
        FIELD_MAP_INDEX,
        "loc_4335C: Field_Map_Index",
        Some((TITLE_SET_MAP, TITLE_SET_MAP + 6)),
    )?;
    let routine = move_w(
        rom,
        GAME_MODE_ROUTINE,
        "Title_StartOption: Game_Mode_Routine",
        Some((TITLE_START_OPTION_HANDOFF, TITLE_START_OPTION_HANDOFF + 6)),
    )?;
    let event = move_w(
        rom,
        EVENT_INDEX,
        "Title_StartOption: Event_Index",
        Some((
            TITLE_START_OPTION_HANDOFF + 6,
            TITLE_START_OPTION_HANDOFF + 12,
        )),
    )?;
    let index = event.value()? as u16;
    if index != GAME_START_EVENT_INDEX {
        return refuse(format!(
            "the title hands off to event 0x{index:X}, not the expected 0x{GAME_START_EVENT_INDEX:X}"
        ));
    }
    let map = scene_map.value()?;
    let typed = TitleHandoff {
        scene_map: GameStartMap {
            id: map as u16,
            symbol: MAP_SYMBOLS.get(map as usize).map(|s| (*s).to_owned()),
        },
        event_index: HandoffEvent { value: index },
    };
    let json = Json::object()
        .with(
            "scene_map",
            map_reference(map).with(
                "note",
                "the map the opening scene plays on, not where the player starts",
            ),
        )
        .with(
            "game_mode_routine",
            Json::object()
                .with("value", routine.value()?)
                .with("meaning", "GameMode_Field, play event"),
        )
        .with(
            "event_index",
            Json::object()
                .with("value", index)
                .with("value_hex", format!("0x{index:02X}")),
        )
        .with(
            "reads",
            vec![scene_map.to_json(), routine.to_json(), event.to_json()],
        );
    Ok((typed, json))
}

fn hex_flags(ids: &[u16]) -> Vec<String> {
    ids.iter().map(|v| format!("0x{v:03X}")).collect()
}

fn read_new_game_init(rom: &[u8]) -> Result<(NewGameInit, Json, Vec<u16>, Vec<u16>), PackError> {
    let window = Some((NEW_GAME_INIT, INIT_CHAR_STATS));
    let mut parts = vec![lit(&[0x41, 0xF8]), ram(CURRENT_PARTY_SLOTS)];
    for _ in 0..5 {
        parts.push(lit(&[0x10, 0xFC, 0x00]));
        parts.push(Part::Any(1));
    }
    parts.push(lit(&[0x10, 0xBC, 0x00]));
    parts.push(Part::Any(1));
    let party = read_pattern(rom, &parts, "loc_44414: Current_Party_Slots", window, 1, 0)?;
    if party.values.len() != PARTY_SLOTS {
        return refuse(format!(
            "the party init writes {} slots, not {PARTY_SLOTS}",
            party.values.len()
        ));
    }
    let money = read_pattern(
        rom,
        &[
            lit(&[0x41, 0xF8]),
            ram(CURRENT_MONEY),
            lit(&[0x20, 0xBC]),
            Part::Any(4),
        ],
        "loc_44414: Current_Money",
        window,
        1,
        0,
    )?;
    let inventory = read_pattern(
        rom,
        &[
            lit(&[0x41, 0xF8]),
            ram(INVENTORY),
            lit(&[0x3E, 0x3C]),
            Part::Any(2),
            lit(&[0x70, 0x00, 0x10, 0xC0, 0x51, 0xCF, 0xFF, 0xFC]),
        ],
        "loc_44414: Inventory clear",
        window,
        1,
        0,
    )?;
    let cleared = read_pattern(
        rom,
        &[
            lit(&[0x3E, 0x3C]),
            Part::Any(2),
            lit(&[0x41, 0xF8]),
            ram(EVENT_FLAGS),
            lit(&[0x4E, 0x40]),
        ],
        "loc_44414: Event_Flags clear",
        window,
        1,
        0,
    )?;
    let table = |address: u16, label: &str| {
        read_pattern(
            rom,
            &[
                lit(&[0x41, 0xF9]),
                Part::Any(4),
                lit(&[0x43, 0xF8]),
                ram(address),
                lit(&[0x3E, 0x3C]),
                Part::Any(2),
                lit(&[0x4E, 0x41]),
            ],
            label,
            window,
            1,
            0,
        )
    };
    let extended = table(
        EXTENDED_EVENT_FLAGS,
        "loc_44414: Extended_Event_Flags table",
    )?;
    let macro_data = table(MACRO_DATA, "loc_44414: Macro_Data table")?;
    let char_stats = read_pattern(
        rom,
        &[
            lit(&[0x41, 0xF8]),
            ram(CHARACTER_STATS),
            lit(&[0x43, 0xF9]),
            Part::Any(4),
            lit(&[0x7E]),
            Part::Any(1),
        ],
        "InitializeCharStats: InitialCharStats",
        None,
        1,
        0,
    )?;
    let (extended_source, extended_words) =
        (extended.values[0] as usize, extended.values[1] as usize);
    let extended_bytes = (extended_words + 1) * 2;
    let extended_table = rom
        .get(extended_source..extended_source + extended_bytes)
        .ok_or_else(|| PackError("the extended flag table runs past the ROM".into()))?;
    let cleared_bytes = (cleared.value()? as usize + 1) * 4;
    let mut speeds = Vec::new();
    for (name, address) in [
        ("message_speed", MESSAGE_SPEED),
        ("battle_speed", BATTLE_SPEED),
        ("world_index", WORLD_INDEX),
    ] {
        speeds.push(move_w(rom, address, &format!("loc_44414: {name}"), window)?.value()? as u16);
    }
    // Town_Flags is a word and then two bytes: four bytes wide.
    let town_word = move_w(rom, TOWN_FLAGS, "loc_44414: Town_Flags", window)?;
    let town_bytes = [2u16, 3]
        .iter()
        .map(|offset| {
            move_b(
                rom,
                TOWN_FLAGS + offset,
                &format!("loc_44414: Town_Flags+{offset}"),
                window,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut town = (town_word.value()? as u16).to_be_bytes().to_vec();
    for read in &town_bytes {
        town.push(read.value()? as u8);
    }
    let extended_set = flag_ids(extended_table, 0x100);
    let town_set = flag_ids(&town, 0);
    let slots: Vec<u8> = party.values.iter().map(|&v| v as u8).collect();
    let typed = NewGameInit {
        money: money.value()? as u32,
        party_slots: std::array::from_fn(|i| PartySlot {
            empty: slots[i] == EMPTY_SLOT,
            id: (slots[i] != EMPTY_SLOT).then_some(slots[i]),
        }),
        flags: NewGameFlags {
            extended_event_flags: FlagList {
                set: hex_flags(&extended_set),
            },
            town_flags: FlagList {
                set: hex_flags(&town_set),
            },
        },
        settings: NewGameSettings {
            message_speed: speeds[0],
            battle_speed: speeds[1],
            world_index: speeds[2],
        },
    };
    let mut reads = vec![
        &party,
        &money,
        &inventory,
        &cleared,
        &extended,
        &macro_data,
        &char_stats,
        &town_word,
    ];
    reads.extend(town_bytes.iter());
    let json = Json::object()
        .with("routine", format!("0x{NEW_GAME_INIT:06X}"))
        .with("party_slots", slots.iter().map(|&s| character(s)).collect::<Result<Vec<_>, _>>()?)
        .with("money", money.value()?)
        .with(
            "inventory",
            Json::object()
                .with("address", format!("0xFFFF{INVENTORY:04X}"))
                .with("slots", inventory.value()? + 1)
                .with("all_empty", true)
                .with("note", "every slot is written 0; a new game carries no items"),
        )
        .with(
            "settings",
            Json::object()
                .with("message_speed", speeds[0])
                .with("battle_speed", speeds[1])
                .with("world_index", speeds[2]),
        )
        .with(
            "flags",
            Json::object()
                .with(
                    "cleared_region",
                    Json::object()
                        .with("start", format!("0xFFFF{EVENT_FLAGS:04X}"))
                        .with("bytes", cleared_bytes)
                        .with("end", format!("0xFFFF{:04X}", usize::from(EVENT_FLAGS) + cleared_bytes))
                        .with(
                            "note",
                            "one trap #0 clears event flags, extended event flags, chest flags and \
                             town flags together; the two below are written back afterwards",
                        ),
                )
                .with(
                    "extended_event_flags",
                    Json::object()
                        .with("address", format!("0xFFFF{EXTENDED_EVENT_FLAGS:04X}"))
                        .with("source", format!("0x{extended_source:06X}"))
                        .with("bytes", extended_bytes)
                        .with("raw_hex", hex(extended_table))
                        .with("sha256", hex(&Sha256::digest(extended_table)))
                        .with("first_id", 0x100)
                        .with("set", hex_flags(&extended_set))
                        .with(
                            "note",
                            "retail copies this to Extended_Event_Flags ($FFFFF120); the \
                             disassembly clone names Chest_Flags ($FFFFF140) here and is wrong. The \
                             disassembly names no flag above 0xEB, so these eleven have ids and no \
                             symbols.",
                        ),
                )
                .with(
                    "chest_flags",
                    Json::object()
                        .with("address", format!("0xFFFF{CHEST_FLAGS:04X}"))
                        .with("set", Json::Array(Vec::new()))
                        .with("note", "cleared with the rest and never written back: no chest is opened"),
                )
                .with(
                    "town_flags",
                    Json::object()
                        .with("address", format!("0xFFFF{TOWN_FLAGS:04X}"))
                        .with("bytes", town.len())
                        .with("raw_hex", hex(&town))
                        .with("set", hex_flags(&town_set)),
                ),
        )
        .with(
            "macro_data",
            Json::object()
                .with("address", format!("0xFFFF{MACRO_DATA:04X}"))
                .with("source", format!("0x{:06X}", macro_data.values[0]))
                .with("bytes", (macro_data.values[1] + 1) * 2),
        )
        .with(
            "character_stats",
            Json::object()
                .with("routine", format!("0x{INIT_CHAR_STATS:06X}"))
                .with("source", format!("0x{:06X}", char_stats.values[0]))
                .with("characters", char_stats.values[1] + 1)
                .with(
                    "note",
                    "a separate system this module does not re-extract: the same InitialCharStats \
                     records generated/characters.json decodes, applied to all eleven characters \
                     whether or not they are in the party",
                ),
        )
        .with("reads", reads.iter().map(|r| r.to_json()).collect::<Vec<_>>());
    Ok((typed, json, extended_set, town_set))
}

fn run_events_jmp_tbl(rom: &[u8]) -> Result<usize, PackError> {
    let site = RUN_EVENTS_JMP_TBL_SITE;
    if rom.get(site..site + 2) != Some(&[0x45, 0xFA]) {
        return refuse(format!(
            "0x{site:06X} is not the `lea (d16,PC),a2` that names RunEventsJmpTbl"
        ));
    }
    Ok((site as i64 + 2 + i64::from(word(rom, site + 2) as i16)) as usize)
}

/// One `RunEventsJmpTbl` entry: what gates it and what it plays.
struct Trigger {
    index: u16,
    rom_offset: usize,
    event: Option<u16>,
    gates: Vec<(u16, bool)>,
    position_gated: bool,
}

impl Trigger {
    /// A position-gated trigger never fires before control: the party hasn't
    /// moved.
    fn fires(&self, flags: &BTreeSet<u16>) -> bool {
        self.event.is_some()
            && !self.position_gated
            && self
                .gates
                .iter()
                .all(|(flag, wanted)| flags.contains(flag) == *wanted)
    }

    fn to_json(&self) -> Json {
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
fn preceding_d0(rom: &[u8], call: usize) -> Result<u16, PackError> {
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

fn read_trigger(rom: &[u8], index: u16) -> Result<Trigger, PackError> {
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
fn flag_setters(rom: &[u8]) -> Result<Vec<(usize, &'static str)>, PackError> {
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
fn scene_flags(
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
fn event_routine_bounds(rom: &[u8], index: u16) -> Result<(usize, usize), PackError> {
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

/// `_game_start_epilogue`: where the opening scene puts the player.
fn game_start_epilogue(
    rom: &[u8],
    routine: usize,
    end: usize,
) -> Result<(Json, Vec<Read>, u64), PackError> {
    let window = (routine, end);
    let scene_party = move_l(
        rom,
        CURRENT_PARTY_SLOTS,
        "Event_GameStart: party during the scene",
        window,
        2,
        0,
    )?;
    let party = move_l(
        rom,
        CURRENT_PARTY_SLOTS,
        "Event_GameStart: party",
        window,
        2,
        1,
    )?;
    let slot5 = move_b(
        rom,
        CURRENT_PARTY_SLOT_5,
        "Event_GameStart: slot 5",
        Some(window),
    )?;
    let epilogue = Some((party.rom_offset, end));
    let mut placed = Vec::new();
    for (name, address) in [
        ("map", FIELD_MAP_INDEX),
        ("map_2", FIELD_MAP_INDEX_2),
        ("x", MAP_START_X_POS),
        ("y", MAP_START_Y_POS),
        ("facing", MAP_START_FACING_DIR),
        ("align", MAP_START_CHAR_ALIGN),
    ] {
        placed.push(move_w(
            rom,
            address,
            &format!("Event_GameStart: {name}"),
            epilogue,
        )?);
    }
    let music_read = move_b(
        rom,
        SAVED_SOUND_INDEX,
        "Event_GameStart: Saved_Sound_Index",
        epilogue,
    )?;
    let value = |i: usize| placed[i].value();
    let mut slots: Vec<u8> = (party.value()? as u32).to_be_bytes().to_vec();
    slots.push(EMPTY_SLOT);
    slots.push(slot5.value()? as u8);
    let (x, y) = (value(2)?, value(3)?);
    let (x_pixels, y_pixels) = (x * START_POS_PIXELS, y * START_POS_PIXELS);
    let facing = value(4)?;
    let facing_name = match facing {
        0 => Some("down"),
        4 => Some("up"),
        8 => Some("right"),
        0xC => Some("left"),
        _ => None,
    };
    let scene_slots = (scene_party.value()? as u32).to_be_bytes();
    let map = value(0)?;
    let json = Json::object()
        .with(
            "event",
            Json::object()
                .with("index", GAME_START_EVENT_INDEX)
                .with("index_hex", format!("0x{GAME_START_EVENT_INDEX:02X}"))
                .with("routine", format!("0x{routine:06X}"))
                .with("rom_end", format!("0x{end:06X}"))
                .with("symbol", "Event_GameStart")
                .with(
                    "note",
                    "an opening scene runs first and this project cannot execute it; the placement \
                     below is the epilogue, and the flags below are every scene the chain runs \
                     before control",
                )
                .with(
                    "party_during_scene",
                    scene_slots.iter().map(|&s| character(s)).collect::<Result<Vec<_>, _>>()?,
                ),
        )
        .with("map", map_reference(map))
        .with("previous_map_word", format!("0x{:04X}", value(1)?))
        .with("party", slots.iter().map(|&s| character(s)).collect::<Result<Vec<_>, _>>()?)
        .with(
            "position",
            Json::object()
                .with("x_start_word", x)
                .with("y_start_word", y)
                .with("x_pixels", x_pixels)
                .with("y_pixels", y_pixels)
                .with("x_cell", x_pixels / COLLISION_CELL_PIXELS)
                .with("y_cell", y_pixels / COLLISION_CELL_PIXELS + STANDING_CELL_Y_OFFSET)
                .with(
                    "note",
                    "loc_535D4 shifts both start words left by three to make curr_x_pos/curr_y_pos, \
                     so the stored word counts 8-pixel steps; y_cell carries the standing-cell shift \
                     the pack applies everywhere else",
                ),
        )
        .with("facing", Json::object().with("id", facing).with("name", facing_name))
        .with("character_alignment", value(5)?)
        .with("music", music(music_read.value()?));
    let mut reads = vec![scene_party, party, slot5, music_read];
    reads.extend(placed);
    Ok((json, reads, map))
}

/// `_pack_flags`: the inverse of [`flag_ids`].
fn pack_flags(flags: &BTreeSet<u16>, base: u16) -> Result<Vec<u8>, PackError> {
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
fn read_first_control(
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

/// `extract_new_game`: the typed record and the whole document beside it.
pub fn extract_new_game<'a>(
    rom: &[u8],
    maps: impl Iterator<Item = &'a MapRecord>,
) -> Result<(GameStartFile, Json), PackError> {
    let maps: Vec<&MapRecord> = maps.collect();
    let (handoff, handoff_json) = read_title_handoff(rom)?;
    let (init, init_json, extended, town) = read_new_game_init(rom)?;
    let (start, end) = event_routine_bounds(rom, GAME_START_EVENT_INDEX)?;
    let (_, _, start_map) = game_start_epilogue(rom, start, end)?;
    let Some(record) = maps.iter().find(|r| r.id as u64 == start_map) else {
        return refuse(format!(
            "the game starts on map 0x{start_map:03X}, which is not a real record"
        ));
    };
    let first_control = read_first_control(rom, record, &extended, &town)?;
    let typed = GameStartFile {
        format_version: FORMAT_VERSION,
        kind: "game_start".into(),
        new_game_init: init,
        title_handoff: handoff,
    };
    let json = Json::object()
        .with("kind", "game_start")
        .with(
            "note",
            "Where a fresh playthrough begins. `first_control` is the state to resume from: two \
             scenes run between `new_game_init` and it, and every flag they set is recorded there.",
        )
        .with("title_handoff", handoff_json)
        .with("new_game_init", init_json)
        .with("first_control", first_control);
    Ok((typed, json))
}

/// The section.
pub fn build(rom: &Rom, root: &Path) -> Result<(), PackError> {
    let data = rom.bytes();
    let decodes = Decodes::new(data);
    let maps = extract_maps(data, &decodes)?;
    let (typed, research) = extract_new_game(data, maps.real())?;
    let value = serde_json::to_value(&typed)
        .map_err(|error| PackError(format!("cannot serialize game_start.json: {error}")))?;
    let mut file = Json::from(&value);
    overlay(&mut file, &research);
    crate::write_json(root, "game_start.json", &file).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_pack_most_significant_bit_first_and_round_trip() {
        let set: BTreeSet<u16> = [0x100, 0x107, 0x108, 0x11F].into();
        let packed = pack_flags(&set, 0x100).unwrap();
        assert_eq!(&packed[..4], &[0x81, 0x80, 0x00, 0x01]);
        assert_eq!(
            flag_ids(&packed, 0x100),
            set.into_iter().collect::<Vec<_>>()
        );
        assert!(pack_flags(&[0x20].into(), 0x100).is_err());
        assert!(pack_flags(&[0x100].into(), 0).is_err());
    }

    #[test]
    fn a_pattern_must_match_the_expected_number_of_times() {
        let rom = [
            0x31, 0xFC, 0x12, 0x34, 0xEC, 0x28, 0x31, 0xFC, 0x56, 0x78, 0xEC, 0x28,
        ];
        let read = |expect, pick| {
            read_pattern(
                &rom,
                &[lit(&[0x31, 0xFC]), Part::Any(2), ram(0xEC28)],
                "x",
                None,
                expect,
                pick,
            )
        };
        assert_eq!(read(2, 1).unwrap().value().unwrap(), 0x5678);
        assert_eq!(read(2, 0).unwrap().rom_offset, 0);
        assert!(
            read(1, 0)
                .unwrap_err()
                .0
                .contains("expected 1 match(es), found 2")
        );
        let windowed = move_w(&rom, 0xEC28, "x", Some((6, 12))).unwrap();
        assert_eq!(
            (windowed.rom_offset, windowed.value().unwrap()),
            (6, 0x5678)
        );
    }

    #[test]
    fn a_position_gated_trigger_never_fires_before_control() {
        let trigger = |position_gated, gates: Vec<(u16, bool)>| Trigger {
            index: 0,
            rom_offset: 0,
            event: Some(1),
            gates,
            position_gated,
        };
        let flags: BTreeSet<u16> = [7].into();
        assert!(trigger(false, vec![(7, true), (0x15, false)]).fires(&flags));
        assert!(!trigger(false, vec![(7, false)]).fires(&flags));
        assert!(!trigger(true, vec![]).fires(&flags));
    }

    #[test]
    fn the_flag_id_comes_from_a_moveq_or_a_move_w() {
        assert_eq!(preceding_d0(&[0x70, 0x15, 0, 0], 2).unwrap(), 0x15);
        assert_eq!(
            preceding_d0(&[0x30, 0x3C, 0x01, 0x07, 0, 0], 4).unwrap(),
            0x107
        );
        assert!(preceding_d0(&[0, 0, 0, 0, 0], 4).is_err());
    }
}
