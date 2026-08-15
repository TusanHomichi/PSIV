"""New game: the state the cartridge starts a fresh playthrough in.

The runtime cannot ask the ROM "where does the game begin" -- there is no table
for it. The answer is spread across three routines the title screen runs in
sequence, and the middle one is a scripted scene this project cannot execute.
So what this module extracts is deliberately not "the initial state": it is the
state at the **first controllable moment**, which is the only state a runtime
without an event engine can honestly resume from.

The path, in order, with retail offsets:

    loc_4335C           0x04335C  the title screen sets Field_Map_Index to
                                  MapID_PiataAcademy ($11) before the player has
                                  chosen anything -- this is the map the opening
                                  scene plays on, not where the player starts
    loc_44414           0x044414  new-game initialisation: party, money, empty
                                  inventory, flags, vehicles, character stats
    Title_StartOption   0x043788  hands off to GameMode_Field with
                                  Game_Mode_Routine = $C ("play event") and
                                  Event_Index = $9F
    Event_GameStart     0x073946  the opening scene, then an epilogue that
                                  places the player and returns control

The party changes twice along that path, which is why "who do you start with"
has three different true answers:

    after loc_44414     Chaz, Alys                  (slots 0 and 1)
    during the scene    Alys, Chaz                  (0x073A24, Alys leading)
    at first control    Chaz alone                  (0x073E24)

The last one is the one that matters. `Event_GameStart`'s epilogue rewrites the
party to Chaz and four empty slots, moves the player to
`MapID_PiataAcademy_F1`, and sets `EventFlag_PiataFirstTime` -- the flag the
disassembly annotates "set when Chaz is alone in Piata at the start of the
game". Alys is on that map as an ordinary field object (`NPCAlysPiata`), which
is the walk-and-find-her opening the game actually starts with.

Two cautions, both paid off by reading the cartridge instead of the clone:

* The clone's `Event_GameStart` carries `move.w #$58, (Map_Start_X_Pos).w ; was
  60` and two more like it. Those are Grand Cross edits. Retail says $60, $24
  and facing 0, and this module reads the retail immediates.
* The clone's `loc_44414` copies its 32-byte flag table to `Chest_Flags`
  ($FFFFF140). Retail copies it to `Extended_Event_Flags` ($FFFFF120) -- the
  constants file defines both, and the retail `lea` names the second. So a new
  game does *not* start with every flag clear: eleven extended flags are
  preloaded, and chest flags really are all zero.

Everything below is read out of the retail image at sites pinned by their own
opcode bytes. Nothing is a hardcoded value with a comment claiming provenance;
if a site does not match, extraction raises.
"""

from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass
from typing import Any

from .symbols import EVENT_FLAG_SYMBOLS, MAP_SYMBOLS, MUSIC_ID_BASE, MUSIC_SYMBOLS

# ---------------------------------------------------------------------------
# RAM addresses, from ps4.constants.asm. These are the operands the pinned
# instructions carry, so a wrong one simply fails to match.
# ---------------------------------------------------------------------------
GAME_MODE_ROUTINE = 0xEC20
FIELD_MAP_INDEX = 0xEC28
FIELD_MAP_INDEX_2 = 0xEC2A
MAP_START_FACING_DIR = 0xEC44
MAP_START_CHAR_ALIGN = 0xEC46
MAP_START_X_POS = 0xEC48
MAP_START_Y_POS = 0xEC4A
MAP_LOAD_FLAGS = 0xEC4E
EVENT_INDEX = 0xECA8
SAVED_SOUND_INDEX = 0xECEC
EVENT_FLAGS = 0xF100
EXTENDED_EVENT_FLAGS = 0xF120
CHEST_FLAGS = 0xF140
TOWN_FLAGS = 0xF160
WORLD_INDEX = 0xF400
CURRENT_PARTY_SLOTS = 0xF40A
CURRENT_PARTY_SLOT_5 = 0xF40E
INVENTORY = 0xF410
CURRENT_MONEY = 0xF438
MESSAGE_SPEED = 0xF440
BATTLE_SPEED = 0xF442
MACRO_DATA = 0xF444
CHARACTER_STATS = 0xF500

#: Party slots are six bytes, but `loc_535F8` places only five characters
#: (`moveq #4,d7`), so slot 5 is storage the field never draws.
PARTY_SLOTS = 6
PARTY_SLOTS_PLACED = 5
#: `cmpi.b #$FF` in the placement loop: the slot is empty.
EMPTY_SLOT = 0xFF

#: `CharID_*` in ps4.constants.asm, in id order. Character *display* names come
#: from the cartridge's own tables and are already in `generated/characters.json`;
#: these are the disassembly's identifiers, the same boundary `ITEM_SYMBOLS`
#: sits on. Eleven of them, which is also what `InitializeCharStats` loops over.
CHARACTER_SYMBOLS = (
    "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika", "Demi", "Wren", "Raja",
    "Kyra", "Seth",
)

#: `Map_Start_Facing_Dir` in the disassembly's constants.
FACING_NAMES = {0: "down", 4: "up", 8: "right", 0xC: "left"}

#: `loc_535D4`: `lsl.w #3` on both start coordinates before they become
#: `curr_x_pos`/`curr_y_pos`, so the stored word is in units of 8 pixels.
START_POS_PIXELS = 8
COLLISION_CELL_PIXELS = 16
#: `GetChunkAndCollision`: `addi.w #$10,d6`, the same shift the pack applies to
#: every Y a map record stores.
STANDING_CELL_Y_OFFSET = 1

#: Sites, all retail addresses. Each is checked by the pattern that reads it.
TITLE_SET_MAP = 0x04335C
NEW_GAME_INIT = 0x044414
INIT_CHAR_STATS = 0x044652
TITLE_START_OPTION_HANDOFF = 0x043788
EVENT_DISPATCH = 0x05A27A
EVENT_PTRS = 0x05A2B4
EVENT_FLAGS_SET = 0x057666
EVENT_FLAGS_TEST = 0x057624
#: The four flag banks, by the RAM address their setter's `lea` names. They
#: share one bit routine and differ only in that base, so scanning a scene for
#: writes means scanning for all four -- a scan that covered only the first
#: would under-report exactly the way this module once did.
FLAG_BANKS = {
    EVENT_FLAGS: "event_flags",
    EXTENDED_EVENT_FLAGS: "extended_event_flags",
    CHEST_FLAGS: "chest_flags",
    TOWN_FLAGS: "town_flags",
}
#: `movem.l / lea / bra.s` three times and then `movem.l / lea` -- the four
#: entry points fall out of the block at a fixed stride.
FLAG_SETTER_STRIDE = 0x0A
GAME_START_EVENT_INDEX = 0x9F
#: The `lea (d16,PC),a2` inside `RunEvents` that names `RunEventsJmpTbl`.
RUN_EVENTS_JMP_TBL_SITE = 0x0560CA
#: `Character_1`. A trigger that reads it is gated on where the player is
#: standing, which is the discriminator the chain walk below turns on.
CHARACTER_1 = 0xC000

#: `EventFlags_Set`, transcribed: `andi.w #$FF,d0` then byte `id >> 3`, bit
#: `7 - (id & 7)`. Flags are packed most-significant-bit first.
FLAG_BITS_PER_BYTE = 8

#: The two flags the pre-control chain sets. `psiv_tools.symbols` holds the
#: shared table but was not in this task's ownership to extend; these belong
#: there, and an oracle test checks both names against ps4.constants.asm.
EVENT_FLAG_PIATA_FIRST_TIME = 0x07
EVENT_FLAG_PIATA_CHAZ_CONTROL = 0x15
PRE_CONTROL_FLAG_SYMBOLS = {
    EVENT_FLAG_PIATA_FIRST_TIME: "EventFlag_PiataFirstTime",
    EVENT_FLAG_PIATA_CHAZ_CONTROL: "EventFlag_PiataChazControl",
}
FLAG_SYMBOLS = {**EVENT_FLAG_SYMBOLS, **PRE_CONTROL_FLAG_SYMBOLS}


class NewGameError(ValueError):
    pass


# ---------------------------------------------------------------------------
# Reading pinned instruction sequences
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class Read:
    """One instruction sequence found in the retail image, and what it held."""

    label: str
    rom_offset: int
    raw_hex: str
    values: tuple[int, ...]

    @property
    def value(self) -> int:
        if len(self.values) != 1:
            raise NewGameError(
                f"{self.label} captured {len(self.values)} values, not one"
            )
        return self.values[0]

    def to_json(self) -> dict[str, Any]:
        return {
            "label": self.label,
            "rom_offset": f"0x{self.rom_offset:06X}",
            "raw_hex": self.raw_hex,
        }


def _read(
    rom: bytes,
    pattern: bytes,
    label: str,
    *,
    width: int | tuple[int, ...] = 1,
    window: tuple[int, int] | None = None,
    expect: int = 1,
    pick: int = 0,
) -> Read:
    """Find one instruction sequence and pull its captured immediates.

    `pattern` is a regular expression over bytes whose groups are the values to
    read; `width` is how many bytes each group holds, one number for all of
    them or one per group. A pattern that matches other than `expect` times
    raises, which is what makes an address in this module a claim the ROM can
    refute rather than a constant to trust; `pick` selects which match to read
    when a routine genuinely writes the same place twice.
    """
    start, end = window or (0, len(rom))
    matches = list(re.finditer(pattern, rom[start:end], re.DOTALL))
    if len(matches) != expect:
        where = f" within 0x{start:06X}..0x{end:06X}" if window else ""
        raise NewGameError(
            f"{label}: expected {expect} match(es){where}, found {len(matches)}"
        )
    match = matches[pick]
    groups = match.groups()
    widths = width if isinstance(width, tuple) else (width,) * len(groups)
    if len(widths) != len(groups):
        raise NewGameError(
            f"{label}: {len(widths)} widths given for {len(groups)} captured groups"
        )
    for index, (group, expected) in enumerate(zip(groups, widths)):
        if len(group) != expected:
            raise NewGameError(
                f"{label}: captured group {index} is {len(group)} bytes, not {expected}"
            )
    values = tuple(int.from_bytes(g, "big") for g in groups)
    return Read(
        label=label,
        rom_offset=start + match.start(),
        raw_hex=match.group(0).hex(),
        values=values,
    )


def _move_w_imm(rom: bytes, ram: int, label: str, **kwargs) -> Read:
    """`move.w #imm,(ram).w` -- opcode 31FC, immediate, 16-bit address."""
    return _read(
        rom, b"\x31\xfc(..)" + re.escape(ram.to_bytes(2, "big")), label, width=2, **kwargs
    )


def _move_b_imm(rom: bytes, ram: int, label: str, **kwargs) -> Read:
    """`move.b #imm,(ram).w` -- the immediate is a word, low byte used."""
    return _read(
        rom, b"\x11\xfc\x00(.)" + re.escape(ram.to_bytes(2, "big")), label,
        width=1, **kwargs,
    )


def _move_l_imm(rom: bytes, ram: int, label: str, **kwargs) -> Read:
    """`move.l #imm,(ram).w` -- opcode 21FC."""
    return _read(
        rom, b"\x21\xfc(....)" + re.escape(ram.to_bytes(2, "big")), label,
        width=4, **kwargs,
    )


def _flag_ids(data: bytes, base: int) -> tuple[int, ...]:
    """Which flag ids a packed flag region has set.

    `EventFlags_Set` puts flag `n` in byte `n >> 3` at bit `7 - (n & 7)`, so a
    region is most-significant-bit first and byte order is flag order.
    """
    return tuple(
        base + index * FLAG_BITS_PER_BYTE + bit
        for index, byte in enumerate(data)
        for bit in range(FLAG_BITS_PER_BYTE)
        if byte >> (FLAG_BITS_PER_BYTE - 1 - bit) & 1
    )


def _character(slot_value: int) -> dict[str, Any]:
    if slot_value == EMPTY_SLOT:
        return {"id": None, "symbol": None, "empty": True}
    if slot_value >= len(CHARACTER_SYMBOLS):
        raise NewGameError(
            f"party slot holds character id 0x{slot_value:02X}, outside the "
            f"{len(CHARACTER_SYMBOLS)} the cartridge defines"
        )
    return {"id": slot_value, "symbol": CHARACTER_SYMBOLS[slot_value], "empty": False}


def _map_reference(map_id: int) -> dict[str, Any]:
    return {
        "id": map_id,
        "id_hex": f"0x{map_id:03X}",
        "symbol": MAP_SYMBOLS[map_id] if 0 <= map_id < len(MAP_SYMBOLS) else None,
    }


def _music(value: int) -> dict[str, Any]:
    index = value - MUSIC_ID_BASE
    return {
        "id": value,
        "id_hex": f"0x{value:02X}",
        "symbol": MUSIC_SYMBOLS[index] if 0 <= index < len(MUSIC_SYMBOLS) else None,
    }


# ---------------------------------------------------------------------------
# The three routines
# ---------------------------------------------------------------------------
def event_routines(rom: bytes) -> tuple[int, ...]:
    """Every `EventPtrs` entry, up to where the table stops holding addresses.

    The table is not length-tagged and nothing follows it but code, so its
    extent is the first entry that is not a ROM address. Retail gives 0xA1
    entries, ending flush against the routine at 0x05A538.
    """
    base = event_ptrs(rom)
    routines: list[int] = []
    while True:
        entry = base + len(routines) * 4
        value = int.from_bytes(rom[entry:entry + 4], "big")
        if not 0 < value < len(rom):
            break
        routines.append(value)
        if len(routines) > 0x200:
            raise NewGameError("EventPtrs does not end within 512 entries")
    if base + len(routines) * 4 > min(routines):
        raise NewGameError(
            f"EventPtrs runs to 0x{base + len(routines) * 4:06X}, past its own "
            f"first routine at 0x{min(routines):06X}"
        )
    return tuple(routines)


def event_ptrs(rom: bytes) -> int:
    """`EventPtrs`, the table `loc_5A27A` indexes by Event_Index.

    Reached PC-relative, so its address cannot be grepped; it is pinned instead
    by the `lea (d16,PC),a0` that names it.
    """
    lea = _read(
        rom, b"\x48\xe7\xff\xf8\x41\xfa(..)\xd0\x40\xd0\x40\x20\x70\x00\x00\x4e\x90",
        "loc_5A27A: lea EventPtrs(pc),a0", width=2,
        window=(EVENT_DISPATCH, EVENT_DISPATCH + 20),
    )
    base = lea.rom_offset + 6 + lea.value
    if base != EVENT_PTRS:
        raise NewGameError(
            f"EventPtrs resolves to 0x{base:06X}, not the expected 0x{EVENT_PTRS:06X}"
        )
    return base


def event_routine(rom: bytes, index: int) -> int:
    """One entry of `EventPtrs`."""
    routines = event_routines(rom)
    if not 0 <= index < len(routines):
        raise NewGameError(
            f"event 0x{index:02X} is outside the {len(routines)}-entry EventPtrs table"
        )
    return routines[index]


def read_title_handoff(rom: bytes) -> dict[str, Any]:
    """What the title screen leaves behind when "Start" is chosen.

    `Title_StartOption` does not load a map. It sets the field game mode with
    routine $C -- "play event" -- and an event index, so the first thing a new
    game runs is a script, not a map load.
    """
    scene_map = _move_w_imm(rom, FIELD_MAP_INDEX, "loc_4335C: Field_Map_Index",
                            window=(TITLE_SET_MAP, TITLE_SET_MAP + 6))
    routine = _move_w_imm(rom, GAME_MODE_ROUTINE, "Title_StartOption: Game_Mode_Routine",
                          window=(TITLE_START_OPTION_HANDOFF, TITLE_START_OPTION_HANDOFF + 6))
    event = _move_w_imm(rom, EVENT_INDEX, "Title_StartOption: Event_Index",
                        window=(TITLE_START_OPTION_HANDOFF + 6, TITLE_START_OPTION_HANDOFF + 12))
    if event.value != GAME_START_EVENT_INDEX:
        raise NewGameError(
            f"the title hands off to event 0x{event.value:X}, not the expected "
            f"0x{GAME_START_EVENT_INDEX:X}"
        )
    return {
        "scene_map": {**_map_reference(scene_map.value),
                      "note": "the map the opening scene plays on, not where the player starts"},
        "game_mode_routine": {"value": routine.value, "meaning": "GameMode_Field, play event"},
        "event_index": {"value": event.value, "value_hex": f"0x{event.value:02X}"},
        "reads": [r.to_json() for r in (scene_map, routine, event)],
    }


def read_new_game_init(rom: bytes) -> dict[str, Any]:
    """`loc_44414`: everything a new game writes before the scene runs."""
    window = (NEW_GAME_INIT, INIT_CHAR_STATS)

    party = _read(
        rom,
        b"\x41\xf8" + re.escape(CURRENT_PARTY_SLOTS.to_bytes(2, "big"))
        + b"\x10\xfc\x00(.)" * 5 + b"\x10\xbc\x00(.)",
        "loc_44414: Current_Party_Slots", window=window,
    )
    if len(party.values) != PARTY_SLOTS:
        raise NewGameError(
            f"the party init writes {len(party.values)} slots, not {PARTY_SLOTS}"
        )
    money = _read(
        rom,
        b"\x41\xf8" + re.escape(CURRENT_MONEY.to_bytes(2, "big")) + b"\x20\xbc(....)",
        "loc_44414: Current_Money", width=4, window=window,
    )
    inventory = _read(
        rom,
        b"\x41\xf8" + re.escape(INVENTORY.to_bytes(2, "big"))
        + b"\x3e\x3c(..)\x70\x00\x10\xc0\x51\xcf\xff\xfc",
        "loc_44414: Inventory clear", width=2, window=window,
    )
    flags_cleared = _read(
        rom,
        b"\x3e\x3c(..)\x41\xf8" + re.escape(EVENT_FLAGS.to_bytes(2, "big")) + b"\x4e\x40",
        "loc_44414: Event_Flags clear", width=2, window=window,
    )
    extended = _read(
        rom,
        b"\x41\xf9(....)\x43\xf8" + re.escape(EXTENDED_EVENT_FLAGS.to_bytes(2, "big"))
        + b"\x3e\x3c(..)\x4e\x41",
        "loc_44414: Extended_Event_Flags table", width=(4, 2), window=window,
    )
    macro = _read(
        rom,
        b"\x41\xf9(....)\x43\xf8" + re.escape(MACRO_DATA.to_bytes(2, "big"))
        + b"\x3e\x3c(..)\x4e\x41",
        "loc_44414: Macro_Data table", width=(4, 2), window=window,
    )
    char_stats = _read(
        rom,
        b"\x41\xf8" + re.escape(CHARACTER_STATS.to_bytes(2, "big"))
        + b"\x43\xf9(....)\x7e(.)",
        "InitializeCharStats: InitialCharStats", width=(4, 1),
    )

    extended_source, extended_words = extended.values
    extended_bytes = (extended_words + 1) * 2
    extended_table = rom[extended_source:extended_source + extended_bytes]
    cleared_bytes = (flags_cleared.value + 1) * 4

    speeds = {
        name: _move_w_imm(rom, ram, f"loc_44414: {name}", window=window).value
        for name, ram in (("message_speed", MESSAGE_SPEED),
                          ("battle_speed", BATTLE_SPEED),
                          ("world_index", WORLD_INDEX))
    }
    # Town_Flags is written as a word and then two further bytes, so the region
    # a new game starts with is four bytes wide, not two.
    town_word = _move_w_imm(rom, TOWN_FLAGS, "loc_44414: Town_Flags", window=window)
    town_bytes = [
        _move_b_imm(rom, TOWN_FLAGS + offset, f"loc_44414: Town_Flags+{offset}",
                    window=window)
        for offset in (2, 3)
    ]
    town = town_word.value.to_bytes(2, "big") + bytes(r.value for r in town_bytes)

    return {
        "routine": f"0x{NEW_GAME_INIT:06X}",
        "party_slots": [_character(v) for v in party.values],
        "money": money.value,
        "inventory": {
            "address": f"0xFFFF{INVENTORY:04X}",
            "slots": inventory.value + 1,
            "all_empty": True,
            "note": "every slot is written 0; a new game carries no items",
        },
        "settings": speeds,
        "flags": {
            "cleared_region": {
                "start": f"0xFFFF{EVENT_FLAGS:04X}",
                "bytes": cleared_bytes,
                "end": f"0xFFFF{EVENT_FLAGS + cleared_bytes:04X}",
                "note": (
                    "one trap #0 clears event flags, extended event flags, chest "
                    "flags and town flags together; the two below are written back "
                    "afterwards"
                ),
            },
            "extended_event_flags": {
                "address": f"0xFFFF{EXTENDED_EVENT_FLAGS:04X}",
                "source": f"0x{extended_source:06X}",
                "bytes": extended_bytes,
                "raw_hex": extended_table.hex(),
                "sha256": hashlib.sha256(extended_table).hexdigest(),
                "first_id": 0x100,
                "set": [f"0x{value:03X}" for value in _flag_ids(extended_table, 0x100)],
                "note": (
                    "retail copies this to Extended_Event_Flags ($FFFFF120); the "
                    "disassembly clone names Chest_Flags ($FFFFF140) here and is "
                    "wrong. The disassembly names no flag above 0xEB, so these "
                    "eleven have ids and no symbols."
                ),
            },
            "chest_flags": {
                "address": f"0xFFFF{CHEST_FLAGS:04X}",
                "set": [],
                "note": "cleared with the rest and never written back: no chest is opened",
            },
            "town_flags": {
                "address": f"0xFFFF{TOWN_FLAGS:04X}",
                "bytes": len(town),
                "raw_hex": town.hex(),
                "set": [f"0x{value:03X}" for value in _flag_ids(town, 0)],
            },
        },
        "macro_data": {
            "address": f"0xFFFF{MACRO_DATA:04X}",
            "source": f"0x{macro.values[0]:06X}",
            "bytes": (macro.values[1] + 1) * 2,
        },
        "character_stats": {
            "routine": f"0x{INIT_CHAR_STATS:06X}",
            "source": f"0x{char_stats.values[0]:06X}",
            "characters": char_stats.values[1] + 1,
            "note": (
                "a separate system this module does not re-extract: the same "
                "InitialCharStats records generated/characters.json decodes, "
                "applied to all eleven characters whether or not they are in the "
                "party"
            ),
        },
        "reads": [r.to_json() for r in
                  (party, money, inventory, flags_cleared, extended, macro,
                   char_stats, town_word, *town_bytes)],
    }


def run_events_jmp_tbl(rom: bytes) -> int:
    """`RunEventsJmpTbl`, from the `lea (d16,PC),a2` in `RunEvents` that names it.

    PC-relative, so it cannot be grepped; the site is pinned and the table is
    computed from its displacement.
    """
    site = RUN_EVENTS_JMP_TBL_SITE
    if rom[site:site + 2] != b"\x45\xfa":
        raise NewGameError(
            f"0x{site:06X} is not the `lea (d16,PC),a2` that names RunEventsJmpTbl"
        )
    return site + 2 + int.from_bytes(rom[site + 2:site + 4], "big", signed=True)


@dataclass(frozen=True)
class Trigger:
    """One `RunEventsJmpTbl` entry: what gates it and what it plays.

    Only the shape retail's start-of-game triggers are written in is decoded --
    flag tests, an optional `Event_Index` write, a `moveq #1,d7` and an `rts`.
    Anything else in the routine shows up as `position_gated` or as no dispatch
    at all, and the chain walk refuses to guess about either.
    """

    index: int
    rom_offset: int
    event: int | None
    #: `(flag id, must be set)` in test order.
    gates: tuple[tuple[int, bool], ...]
    position_gated: bool

    def fires(self, flags: frozenset[int]) -> bool:
        """Would this trigger dispatch, given the flags set so far?

        A position-gated trigger never fires here, and that is a statement
        about the player rather than about the code: it is waiting for the
        party to stand somewhere, and the party cannot move until it has
        control. See `read_first_control`.
        """
        if self.event is None or self.position_gated:
            return False
        return all((flag in flags) == wanted for flag, wanted in self.gates)

    def to_json(self) -> dict[str, Any]:
        return {
            "index": self.index,
            "routine": f"0x{self.rom_offset:06X}",
            "event": None if self.event is None else f"0x{self.event:02X}",
            "gates": [
                {"flag": flag, "flag_hex": f"0x{flag:02X}",
                 "required": "set" if wanted else "clear"}
                for flag, wanted in self.gates
            ],
            "position_gated": self.position_gated,
        }


def read_trigger(rom: bytes, index: int) -> Trigger:
    """Decode one `RunEventsJmpTbl` entry, up to its first `rts`."""
    table = run_events_jmp_tbl(rom)
    entry = table + index * 4
    if int.from_bytes(rom[entry:entry + 2], "big") != 0x6000:
        raise NewGameError(
            f"RunEventsJmpTbl entry {index} at 0x{entry:06X} is not a `bra.w`"
        )
    start = entry + 2 + int.from_bytes(rom[entry + 2:entry + 4], "big", signed=True)

    event: int | None = None
    gates: list[tuple[int, bool]] = []
    position_gated = False
    pos = start
    limit = start + 0x100
    while pos < limit:
        word = int.from_bytes(rom[pos:pos + 2], "big")
        if word == 0x4E75:  # rts -- the routine ends here
            break
        # `lea (Character_1).w,An` for any address register.
        if (word & 0xF1FF) == 0x41F8 and int.from_bytes(rom[pos + 2:pos + 4], "big") == CHARACTER_1:
            position_gated = True
        # `move.w #imm,(Event_Index).w`
        if word == 0x31FC and int.from_bytes(rom[pos + 4:pos + 6], "big") == EVENT_INDEX:
            event = int.from_bytes(rom[pos + 2:pos + 4], "big")
        # `jsr (EventFlags_Test).l` preceded by the flag load, followed by the
        # branch whose sense says which way the test has to go.
        if word == 0x4EB9 and int.from_bytes(rom[pos + 2:pos + 6], "big") == EVENT_FLAGS_TEST:
            flag = _preceding_d0(rom, pos)
            branch = rom[pos + 6]
            if branch == 0x66:      # bne -> skip when set, so the gate wants it clear
                gates.append((flag, False))
            elif branch == 0x67:    # beq -> skip when clear, so the gate wants it set
                gates.append((flag, True))
            else:
                raise NewGameError(
                    f"trigger {index}: the flag test at 0x{pos:06X} is followed by "
                    f"0x{branch:02X}, neither `beq` nor `bne`"
                )
        pos += 2
    else:
        raise NewGameError(f"trigger {index} at 0x{start:06X} has no `rts` within 256 bytes")
    return Trigger(index=index, rom_offset=start, event=event,
                   gates=tuple(gates), position_gated=position_gated)


def _preceding_d0(rom: bytes, call: int) -> int:
    """The flag id loaded into d0 just before a call: `moveq` or `move.w`."""
    if rom[call - 2] == 0x70:
        return rom[call - 1]
    if rom[call - 4:call - 2] == b"\x30\x3c":
        return int.from_bytes(rom[call - 2:call], "big")
    raise NewGameError(
        f"the call at 0x{call:06X} is not preceded by a `moveq`/`move.w` into d0"
    )


def flag_setters(rom: bytes) -> dict[int, str]:
    """The four flag-setting entry points, by address, with the bank each writes.

    They are one routine with four doors: each door is `movem.l d0-d2/a0,-(sp)`
    then the `lea` that picks the bank, and the shared tail does the bit
    arithmetic. Reading the `lea` operands is what binds an address to a bank.
    """
    setters: dict[int, str] = {}
    for index in range(len(FLAG_BANKS)):
        address = EVENT_FLAGS_SET + index * FLAG_SETTER_STRIDE
        if rom[address:address + 4] != b"\x48\xe7\xe0\x80":
            raise NewGameError(
                f"0x{address:06X} is not a flag setter's `movem.l d0-d2/a0,-(sp)`"
            )
        if rom[address + 4:address + 6] != b"\x41\xf8":
            raise NewGameError(f"0x{address:06X} does not pick a bank with `lea`")
        bank = int.from_bytes(rom[address + 6:address + 8], "big")
        if bank not in FLAG_BANKS:
            raise NewGameError(
                f"the setter at 0x{address:06X} writes 0xFFFF{bank:04X}, which is "
                "not one of the four flag banks"
            )
        setters[address] = FLAG_BANKS[bank]
    if sorted(setters.values()) != sorted(FLAG_BANKS.values()):
        raise NewGameError("the four setters do not cover the four banks")
    return setters


def scene_flags(rom: bytes, start: int, end: int) -> tuple[tuple[str, Read], ...]:
    """Every flag a scene routine sets, in any of the four banks."""
    found: list[tuple[str, Read]] = []
    for address, bank in flag_setters(rom).items():
        for call in (0x4EB9, 0x4EF9):  # jsr and the tail jmp
            pattern = int.to_bytes(call, 2, "big") + address.to_bytes(4, "big")
            offset = start
            while True:
                offset = rom.find(pattern, offset, end)
                if offset < 0:
                    break
                found.append((bank, Read(
                    label=f"{bank} set at 0x{offset:06X}",
                    rom_offset=offset,
                    raw_hex=rom[offset - 4:offset + 6].hex(),
                    values=(_preceding_d0(rom, offset),),
                )))
                offset += 2
    return tuple(sorted(found, key=lambda pair: pair[1].rom_offset))


def event_routine_bounds(rom: bytes, index: int) -> tuple[int, int]:
    """One event routine's extent.

    Ordinarily the next routine by address bounds it. `Event_PiataChazAlone` is
    the last one in the table, and it ends in a tail `jmp (EventFlags_Set).l`,
    so the terminator bounds it instead.
    """
    start = event_routine(rom, index)
    later = [address for address in event_routines(rom) if address > start]
    if later:
        return start, min(later)
    tail = rom.find(b"\x4e\xf9" + EVENT_FLAGS_SET.to_bytes(4, "big"), start, start + 0x100)
    if tail < 0:
        raise NewGameError(
            f"event 0x{index:02X} at 0x{start:06X} is the last routine and does not "
            "end in a tail jump to EventFlags_Set, so its extent is unknown"
        )
    return start, tail + 6


def _game_start_epilogue(rom: bytes, routine: int, end: int) -> dict[str, Any]:
    """`Event_GameStart`'s epilogue: where the opening scene puts the player.

    Everything before it in the routine is the scene, which needs an event
    interpreter to run. The epilogue rewrites the party, the map, the position
    and the music outright, so what it leaves is a complete placement and not a
    partial one.
    """
    window = (routine, end)
    # The routine writes the party twice: once to stage the scene, once to hand
    # it to the player. Reading both is the point -- the first is why "you start
    # with Alys and Chaz" is a real memory of a state that does not survive.
    scene_party = _move_l_imm(rom, CURRENT_PARTY_SLOTS,
                              "Event_GameStart: party during the scene",
                              window=window, expect=2, pick=0)
    party = _move_l_imm(rom, CURRENT_PARTY_SLOTS, "Event_GameStart: party",
                        window=window, expect=2, pick=1)
    slot5 = _move_b_imm(rom, CURRENT_PARTY_SLOT_5, "Event_GameStart: slot 5", window=window)

    # The scene moves the camera and the map around before it ends, so the
    # placement is read from the epilogue alone -- which begins at the party
    # write above and runs to the end of the routine.
    epilogue_window = (party.rom_offset, end)
    reads = {
        name: _move_w_imm(rom, ram, f"Event_GameStart: {name}", window=epilogue_window)
        for name, ram in (
            ("map", FIELD_MAP_INDEX),
            ("map_2", FIELD_MAP_INDEX_2),
            ("x", MAP_START_X_POS),
            ("y", MAP_START_Y_POS),
            ("facing", MAP_START_FACING_DIR),
            ("align", MAP_START_CHAR_ALIGN),
        )
    }
    music = _move_b_imm(rom, SAVED_SOUND_INDEX, "Event_GameStart: Saved_Sound_Index",
                        window=epilogue_window)

    # The four slots the long covers, then slot 5. Slot 4 keeps whatever
    # `loc_44414` left, which is empty.
    slots = list(party.value.to_bytes(4, "big")) + [EMPTY_SLOT, slot5.value]

    x_pixels = reads["x"].value * START_POS_PIXELS
    y_pixels = reads["y"].value * START_POS_PIXELS
    return {
        "event": {
            "index": GAME_START_EVENT_INDEX,
            "index_hex": f"0x{GAME_START_EVENT_INDEX:02X}",
            "routine": f"0x{routine:06X}",
            "rom_end": f"0x{end:06X}",
            "symbol": "Event_GameStart",
            "note": (
                "an opening scene runs first and this project cannot execute it; "
                "the placement below is the epilogue, and the flags below are "
                "every scene the chain runs before control"
            ),
            "party_during_scene": [
                _character(v) for v in scene_party.value.to_bytes(4, "big")
            ],
        },
        "map": _map_reference(reads["map"].value),
        "previous_map_word": f"0x{reads['map_2'].value:04X}",
        "party": [_character(v) for v in slots],
        "position": {
            "x_start_word": reads["x"].value,
            "y_start_word": reads["y"].value,
            "x_pixels": x_pixels,
            "y_pixels": y_pixels,
            "x_cell": x_pixels // COLLISION_CELL_PIXELS,
            "y_cell": y_pixels // COLLISION_CELL_PIXELS + STANDING_CELL_Y_OFFSET,
            "note": (
                "loc_535D4 shifts both start words left by three to make "
                "curr_x_pos/curr_y_pos, so the stored word counts 8-pixel steps; "
                "y_cell carries the standing-cell shift the pack applies "
                "everywhere else"
            ),
        },
        "facing": {"id": reads["facing"].value,
                   "name": FACING_NAMES.get(reads["facing"].value)},
        "character_alignment": reads["align"].value,
        "music": _music(music.value),
        "reads": [scene_party, party, slot5, music, *reads.values()],
    }


def read_first_control(rom: bytes, record: dict[str, Any] | None = None) -> dict[str, Any]:
    """The state at the first moment the player has input.

    A new game does not hand over control when `Event_GameStart` ends. That
    scene places the player on a map, and `RunEvents` then walks that map's own
    trigger list before `FieldRoutine_Controls` ever runs -- so any trigger that
    is satisfied plays another scene, and only when none is satisfied does the
    player get to move. This walks that chain.

    The walk is sound without an event interpreter because of one asymmetry: a
    trigger gated on the party's position cannot fire before the party has
    moved, and the party cannot move before it has control. So the pre-control
    chain is made only of triggers decidable from event flags, which is exactly
    the shape `read_trigger` decodes. Position-gated triggers on the start map
    are still reported, so nothing is silently dropped.

    Retail runs two scenes: `Event_GameStart` sets `EventFlag_PiataFirstTime`,
    and the trigger that survives it plays `Event_PiataChazAlone`, which sets
    `EventFlag_PiataChazControl` -- the very flag its own trigger tests, which
    is what stops the chain.
    """
    scenes: list[dict[str, Any]] = []
    reads: list[Read] = []
    # Seeded from the initialiser: the chain only ever writes the base bank, but
    # the state a runtime has to seed is all four.
    init = read_new_game_init(rom)["flags"]
    banks: dict[str, set[int]] = {
        "event_flags": set(),
        "extended_event_flags": {
            int(value, 16) for value in init["extended_event_flags"]["set"]
        },
        "chest_flags": set(),
        "town_flags": {int(value, 16) for value in init["town_flags"]["set"]},
    }
    index: int | None = GAME_START_EVENT_INDEX
    epilogue_of: dict[str, Any] | None = None

    while index is not None:
        start, end = event_routine_bounds(rom, index)
        sets = scene_flags(rom, start, end)
        reads.extend(read for _, read in sets)
        if index == GAME_START_EVENT_INDEX:
            epilogue_of = _game_start_epilogue(rom, start, end)
            reads.extend(epilogue_of.pop("reads"))
        scenes.append({
            "event": index,
            "event_hex": f"0x{index:02X}",
            "routine": f"0x{start:06X}",
            "rom_end": f"0x{end:06X}",
            "sets": [
                {"bank": bank, "flag": read.value, "flag_hex": f"0x{read.value:02X}",
                 "symbol": FLAG_SYMBOLS.get(read.value) if bank == "event_flags" else None,
                 "at": f"0x{read.rom_offset:06X}"}
                for bank, read in sets
            ],
        })
        for bank, read in sets:
            banks[bank].add(read.value)
        index = _next_scene(rom, record, banks["event_flags"], scenes)

    if epilogue_of is None:
        raise NewGameError("the chain did not start at Event_GameStart")

    epilogue_of["scene_chain"] = scenes
    epilogue_of["event_flags_set"] = [
        {"id": flag, "id_hex": f"0x{flag:02X}", "symbol": FLAG_SYMBOLS.get(flag)}
        for flag in sorted(banks["event_flags"])
    ]
    epilogue_of["extended_event_flags_set"] = sorted(banks["extended_event_flags"])
    epilogue_of["town_flags_set"] = sorted(banks["town_flags"])
    epilogue_of["chest_flags_set"] = sorted(banks["chest_flags"])
    # The banks as bytes, which is what a runtime seeds and what an emulator
    # oracle dumps. `EventFlags_Set` packs most-significant-bit first, so this
    # is the same arithmetic run backwards.
    epilogue_of["flag_banks"] = {
        bank: {
            "address": f"0xFFFF{address:04X}",
            "first_id": FLAG_BANK_BASES[bank],
            "bytes": FLAG_BANK_BYTES,
            "raw_hex": packed.hex(),
            "first_long": f"0x{int.from_bytes(packed[:4], 'big'):08X}",
            "set": [f"0x{flag:03X}" for flag in sorted(banks[bank])],
        }
        for address, bank in FLAG_BANKS.items()
        for packed in (_pack_flags(banks[bank], FLAG_BANK_BASES[bank]),)
    }
    epilogue_of["reads"] = [read.to_json() for read in reads]
    return epilogue_of


#: Each bank is 32 bytes: $F100, $F120, $F140 and $F160 sit that far apart.
FLAG_BANK_BYTES = 0x20

#: What an id in each bank counts from. Every setter masks with `andi.w #$FF`,
#: so a bank holds 256 flags; the extended bank's ids are quoted from $100 up
#: because that is the space `ExtendedEventFlags_Test` and the dialogue `$FB`
#: control code address it in.
FLAG_BANK_BASES = {
    "event_flags": 0x000,
    "extended_event_flags": 0x100,
    "chest_flags": 0x000,
    "town_flags": 0x000,
}


def _pack_flags(flags: set[int], base: int = 0) -> bytes:
    """A flag set as the cartridge holds it, most-significant-bit first.

    The inverse of `_flag_ids`, so a round trip through the two is the check
    that the packing is right.
    """
    packed = bytearray(FLAG_BANK_BYTES)
    for flag in flags:
        index = (flag - base) >> 3
        if not 0 <= index < FLAG_BANK_BYTES:
            raise NewGameError(
                f"flag 0x{flag:03X} is outside the {FLAG_BANK_BYTES}-byte bank "
                f"that starts at 0x{base:03X}"
            )
        packed[index] |= 1 << (FLAG_BITS_PER_BYTE - 1 - ((flag - base) & 7))
    return bytes(packed)


def _next_scene(
    rom: bytes,
    record: dict[str, Any] | None,
    flags: set[int],
    scenes: list[dict[str, Any]],
) -> int | None:
    """The event the start map's trigger list plays next, if any."""
    if record is None:
        return None
    triggers = [read_trigger(rom, index) for index in record["events"]["ids"]]
    scenes[-1]["triggers_after"] = [trigger.to_json() for trigger in triggers]
    played = {scene["event"] for scene in scenes}
    for trigger in triggers:
        if trigger.fires(frozenset(flags)):
            if trigger.event in played:
                raise NewGameError(
                    f"trigger {trigger.index} would replay event "
                    f"0x{trigger.event:02X}; the pre-control chain does not terminate"
                )
            return trigger.event
    return None


def extract_new_game(
    rom: bytes, maps: list[dict[str, Any]] | None = None
) -> dict[str, Any]:
    """The whole new-game path, as the pack emits it.

    `maps` is `psiv_tools.maps.extract_maps(rom)["maps"]`. The chain walk needs
    the start map's event list to know which scenes run before control; without
    it the walk stops after `Event_GameStart` and the flag set is incomplete, so
    this extracts the table itself rather than emitting a half answer.
    """
    if maps is None:
        from .maps import extract_maps

        maps = extract_maps(rom)["maps"]
    handoff = read_title_handoff(rom)
    init = read_new_game_init(rom)
    # Which map the chain walks the triggers of is the map the epilogue moves
    # the player to, so the first pass reads the placement and the second walks
    # the chain over that map's list.
    start, end = event_routine_bounds(rom, GAME_START_EVENT_INDEX)
    placement = _game_start_epilogue(rom, start, end)
    start_map = placement["map"]["id"]
    record = next((r for r in maps if r["id"] == start_map), None)
    if record is None or record.get("is_null"):
        raise NewGameError(
            f"the game starts on map 0x{start_map:03X}, which is not a real record"
        )
    return {
        "kind": "game_start",
        "note": (
            "Where a fresh playthrough begins. `first_control` is the state to "
            "resume from: two scenes run between `new_game_init` and it, and "
            "every flag they set is recorded there."
        ),
        "title_handoff": handoff,
        "new_game_init": init,
        "first_control": read_first_control(rom, record),
    }
