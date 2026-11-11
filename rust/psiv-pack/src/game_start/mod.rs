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

use std::path::Path;

use chain::{event_routine_bounds, read_first_control};
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

pub(super) const GAME_MODE_ROUTINE: u16 = 0xEC20;
pub(super) const FIELD_MAP_INDEX: u16 = 0xEC28;
pub(super) const FIELD_MAP_INDEX_2: u16 = 0xEC2A;
pub(super) const MAP_START_FACING_DIR: u16 = 0xEC44;
pub(super) const MAP_START_CHAR_ALIGN: u16 = 0xEC46;
pub(super) const MAP_START_X_POS: u16 = 0xEC48;
pub(super) const MAP_START_Y_POS: u16 = 0xEC4A;
pub(super) const EVENT_INDEX: u16 = 0xECA8;
pub(super) const SAVED_SOUND_INDEX: u16 = 0xECEC;
pub(super) const EVENT_FLAGS: u16 = 0xF100;
pub(super) const EXTENDED_EVENT_FLAGS: u16 = 0xF120;
pub(super) const CHEST_FLAGS: u16 = 0xF140;
pub(super) const TOWN_FLAGS: u16 = 0xF160;
/// `World_Index`: the initializer's `move.w #0, (World_Index).w`
/// (`ps4.asm:88670`) is decoded from the ROM into the start's `world_index`.
pub(super) const WORLD_INDEX: u16 = 0xF400;
pub(super) const CURRENT_PARTY_SLOTS: u16 = 0xF40A;
pub(super) const CURRENT_PARTY_SLOT_5: u16 = 0xF40E;
pub(super) const INVENTORY: u16 = 0xF410;
pub(super) const CURRENT_MONEY: u16 = 0xF438;
pub(super) const MESSAGE_SPEED: u16 = 0xF440;
pub(super) const BATTLE_SPEED: u16 = 0xF442;
pub(super) const MACRO_DATA: u16 = 0xF444;
pub(super) const CHARACTER_STATS: u16 = 0xF500;
pub(super) const PARTY_SLOTS: usize = 6;
pub(super) const EMPTY_SLOT: u8 = 0xFF;
pub(super) const CHARACTER_SYMBOLS: [&str; 11] = [
    "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika", "Demi", "Wren", "Raja", "Kyra", "Seth",
];
pub(super) const START_POS_PIXELS: u64 = 8;
pub(super) const COLLISION_CELL_PIXELS: u64 = 16;
pub(super) const STANDING_CELL_Y_OFFSET: u64 = 1;
pub(super) const TITLE_SET_MAP: usize = 0x04335C;
pub(super) const NEW_GAME_INIT: usize = 0x044414;
pub(super) const INIT_CHAR_STATS: usize = 0x044652;
pub(super) const TITLE_START_OPTION_HANDOFF: usize = 0x043788;
pub(super) const EVENT_DISPATCH: usize = 0x05A27A;
pub(super) const EVENT_PTRS: usize = 0x05A2B4;
pub(super) const EVENT_FLAGS_SET: usize = 0x057666;
pub(super) const EVENT_FLAGS_TEST: u32 = 0x057624;
/// The four banks, by the RAM their setter's `lea` names.
pub(super) const FLAG_BANKS: [(u16, &str); 4] = [
    (EVENT_FLAGS, "event_flags"),
    (EXTENDED_EVENT_FLAGS, "extended_event_flags"),
    (CHEST_FLAGS, "chest_flags"),
    (TOWN_FLAGS, "town_flags"),
];
pub(super) const FLAG_SETTER_STRIDE: usize = 0x0A;
pub(super) const GAME_START_EVENT_INDEX: u16 = 0x9F;
/// The `lea (d16,PC),a2` inside `RunEvents` that names `RunEventsJmpTbl`.
pub(super) const RUN_EVENTS_JMP_TBL_SITE: usize = 0x0560CA;
pub(super) const CHARACTER_1: u16 = 0xC000;
pub(super) const FLAG_BANK_BYTES: usize = 0x20;
/// `PACK_FORMAT_VERSION`.
pub(super) const FORMAT_VERSION: u32 = 2;

pub(super) fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

pub(super) fn word(rom: &[u8], at: usize) -> u16 {
    rom.get(at..at + 2)
        .map_or(0, |b| u16::from_be_bytes([b[0], b[1]]))
}

pub(super) fn long(rom: &[u8], at: usize) -> u32 {
    rom.get(at..at + 4)
        .map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// The flag symbols the chain needs: the shared table, and the two the
/// pre-control chain sets.
pub(super) fn flag_symbol(flag: u16) -> Option<&'static str> {
    match flag {
        0x07 => Some("EventFlag_PiataFirstTime"),
        0x15 => Some("EventFlag_PiataChazControl"),
        _ => event_flag_symbol(flag),
    }
}

/// A byte pattern: literals and captured runs of any byte.
pub(super) enum Part {
    Lit(Vec<u8>),
    Any(usize),
}

pub(super) fn lit(bytes: &[u8]) -> Part {
    Part::Lit(bytes.to_vec())
}

pub(super) fn ram(address: u16) -> Part {
    Part::Lit(address.to_be_bytes().to_vec())
}

/// One instruction sequence found in the image, and what it held.
#[derive(Clone, Debug)]
pub(super) struct Read {
    pub(super) label: String,
    pub(super) rom_offset: usize,
    pub(super) raw_hex: String,
    pub(super) values: Vec<u64>,
}

impl Read {
    pub(super) fn value(&self) -> Result<u64, PackError> {
        match self.values[..] {
            [value] => Ok(value),
            _ => refuse(format!(
                "{} captured {} values, not one",
                self.label,
                self.values.len()
            )),
        }
    }

    pub(super) fn to_json(&self) -> Json {
        Json::object()
            .with("label", self.label.as_str())
            .with("rom_offset", format!("0x{:06X}", self.rom_offset))
            .with("raw_hex", self.raw_hex.as_str())
    }
}

/// `_read`: the pattern must match exactly `expect` times in the window, as
/// `re.finditer` scans (left to right, a match resuming after itself).
pub(super) fn read_pattern(
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

pub(super) fn move_w(
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

pub(super) fn move_b(
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
pub(super) fn flag_ids(data: &[u8], base: u16) -> Vec<u16> {
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
pub(super) fn event_routines(rom: &[u8]) -> Result<Vec<usize>, PackError> {
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

pub(super) fn event_routine(rom: &[u8], index: u16) -> Result<usize, PackError> {
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

/// `_game_start_epilogue`: where the opening scene puts the player.
pub(super) fn game_start_epilogue(
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
pub fn build(rom: &Rom, root: &Path) -> Result<crate::json::Json, PackError> {
    let data = rom.bytes();
    let decodes = Decodes::new(data);
    let maps = extract_maps(data, &decodes)?;
    let (typed, research) = extract_new_game(data, maps.real())?;
    let value = serde_json::to_value(&typed)
        .map_err(|error| PackError(format!("cannot serialize game_start.json: {error}")))?;
    let mut file = Json::from(&value);
    overlay(&mut file, &research);
    let sha = crate::write_json(root, NAME, &file)?;
    Ok(Json::object().with("game_start", headline(&file, sha)?))
}

const NAME: &str = "game_start.json";

/// `write_manifest`'s headline of the file: where a new game begins, so
/// the manifest alone answers it.
fn headline(file: &Json, sha256: String) -> Result<Json, PackError> {
    let start = file.at(&["first_control"])?;
    let list = |key: &str| -> Result<Vec<&Json>, PackError> {
        match start.at(&[key])? {
            Json::Array(items) => Ok(items.iter().collect()),
            _ => Err(PackError(format!("first_control.{key} is not a list"))),
        }
    };
    let mut party = Vec::new();
    for slot in list("party")? {
        if slot.at(&["empty"])? == &Json::Bool(false) {
            party.push(slot.at(&["symbol"])?.clone());
        }
    }
    let flags = list("event_flags_set")?
        .into_iter()
        .map(|flag| flag.at(&["id"]).cloned())
        .collect::<Result<Vec<_>, _>>()?;
    let Json::Object(banks) = start.at(&["flag_banks"])? else {
        return Err(PackError(
            "first_control.flag_banks is not an object".into(),
        ));
    };
    let mut flag_banks = Json::object();
    for (bank, value) in banks {
        flag_banks.insert(
            bank.clone(),
            Json::object()
                .with("address", value.at(&["address"])?.clone())
                .with("raw_hex", value.at(&["raw_hex"])?.clone()),
        );
    }
    let chain = list("scene_chain")?
        .into_iter()
        .map(|scene| scene.at(&["event_hex"]).cloned())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json::object()
        .with("file", NAME)
        .with("sha256", sha256)
        .with("map", start.at(&["map"])?.clone())
        .with("x_cell", start.at(&["position", "x_cell"])?.clone())
        .with("y_cell", start.at(&["position", "y_cell"])?.clone())
        .with("facing", start.at(&["facing"])?.clone())
        .with("party", Json::Array(party))
        .with("music", start.at(&["music"])?.clone())
        .with("event_flags_set", Json::Array(flags))
        .with(
            "extended_event_flags_set",
            start.at(&["extended_event_flags_set"])?.clone(),
        )
        .with("town_flags_set", start.at(&["town_flags_set"])?.clone())
        .with("chest_flags_set", start.at(&["chest_flags_set"])?.clone())
        .with("flag_banks", flag_banks)
        .with("scene_chain", Json::Array(chain))
        .with(
            "note",
            "the first controllable moment, after the two scenes the title screen's event \
             0x9F chains through. The party the new-game initialiser writes (Chaz and Alys) \
             does not survive them, and neither scene ends the flag state -- \
             Event_PiataChazAlone sets the flag its own trigger tests, which is what stops \
             the chain.",
        ))
}

mod chain;
#[cfg(test)]
mod tests;
