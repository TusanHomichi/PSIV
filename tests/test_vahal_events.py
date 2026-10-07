"""Guard: the Vahal Fort and Weapon Plant transcriptions equal the retail bytes.

    PYTHONPATH=. python3 -m unittest tests.test_vahal_events -v

Nineteen events (issue #82) are scenes in `rust/psiv-core/src/scenes/`
(`vahal_fort.rs`, `weapon_plant.rs`, `conveyors.rs`): six moving platforms, two
terminals, four conveyor belts, two chest events, five story scenes. The Rust
tests prove what the scenes *do*; this test proves they were copied from the
cartridge. It re-derives every literal from the US image through the event
pointer table (`EventPtrs` at `$05A2B4`) and compares it with the literal in the
scene source:

- each platform's flag, both start rows, both ride rows (chunk ids, step word,
  coordinates), the frame count the step and the distance imply, and the toggle
  and sound bytes, against `platform_object`, `ride`, `test_temp`,
  `toggle_temp` and the two `WriteMapChunks` of each scene half;
- the terminals' compare literal, branch sense and flag ids;
- each belt's axis and sign (the leader destination write) and its chunk range;
- the chest events' object, dialogue entries, sound and wait sequence, the
  skill slot and id they write into `Wren_Stats`, and the flag;
- the story scenes' dialogue entries and flags, the force-field barrier's
  branch, and `Event_DominatorsDefeated`'s five destinations, six membership
  tests, palette words and tail;
- the pointer table itself: every event's routine starts and ends where the
  documents say.

The negative controls at the end change one literal in the source, or one byte
in the image, and show the comparison names it, so a green run is agreement and
not a vacuous pass.
"""
from __future__ import annotations

import re
import struct
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ROM = ROOT / "Phantasy Star IV (USA).md"
SCENES = ROOT / "rust" / "psiv-core" / "src" / "scenes"

EVENT_PTRS = 0x5A2B4
CUTSCENE_PTRS = 0x5A580

#: `jsr` targets the routines call, by the names the scene doc uses.
JSR = {
    "get_character": 0x5A6D6,
    "run_dialogue": 0x5AC66,
    "run_dialogue_resume": 0x5AC6C,
    "map_update_loop": 0x5A71E,
    "face": 0x5A936,
    "flag_set": 0x57666,
    "flag_test": 0x57624,
    "temp_toggle": 0x576EA,
    "temp_test": 0x57638,
}


def jsr(name: str) -> str:
    return "4EB9" + f"{JSR[name]:08X}"


def jmp(name: str) -> str:
    return "4EF9" + f"{JSR[name]:08X}"


def u16(rom: bytes, at: int) -> int:
    return struct.unpack_from(">H", rom, at)[0]


def s16(rom: bytes, at: int) -> int:
    return struct.unpack_from(">h", rom, at)[0]


def event_extents(rom: bytes) -> dict[int, tuple[int, int]]:
    """Event index -> `(start, end_exclusive)`: a routine ends where the next one in the
    address-sorted pointer tables begins."""
    events = [struct.unpack_from(">I", rom, EVENT_PTRS + 4 * i)[0] for i in range(161)]
    cutscenes = [struct.unpack_from(">I", rom, CUTSCENE_PTRS + 4 * i)[0] for i in range(34)]
    starts = sorted(set(events + cutscenes))
    out = {}
    for index, start in enumerate(events):
        later = [s for s in starts if s > start]
        out[index] = (start, later[0] if later else start)
    return out


def routine(rom: bytes, extents: dict[int, tuple[int, int]], event: int) -> bytes:
    start, end = extents[event]
    return rom[start:end]


# ---------------------------------------------------------------------------
# Reading the scene source
# ---------------------------------------------------------------------------
def num(text: str) -> int:
    text = text.strip().replace("_", "")
    sign = -1 if text.startswith("-") else 1
    return sign * int(text.lstrip("-"), 0)


def source(name: str) -> str:
    return (SCENES / name).read_text(encoding="utf-8")


def constants(text: str) -> dict[str, int]:
    """`const NAME: ty = value;` integer constants (and `(lo, hi)` pairs as two keys)."""
    out = {}
    for name, value in re.findall(r"const ([A-Z_0-9]+):\s*[a-z0-9]+\s*=\s*(-?0x[0-9A-Fa-f_]+|-?\d+);", text):
        out[name] = num(value)
    for name, lo, hi in re.findall(
        r"const ([A-Z_0-9]+):\s*\(u8,\s*u8\)\s*=\s*\((0x[0-9A-Fa-f]+),\s*(0x[0-9A-Fa-f]+)\);", text
    ):
        out[name + "_LO"] = num(lo)
        out[name + "_HI"] = num(hi)
    return out


def scene_block(text: str, static: str) -> str:
    match = re.search(rf"pub static {static}: Scene = Scene \{{(.*?)\n\}};", text, re.S)
    if match is None:
        raise AssertionError(f"{static} is not in the source")
    return match.group(1)


def value(token: str, consts: dict[str, int]) -> int:
    token = token.strip()
    return consts[token] if token in consts else num(token)


def sounds(block: str, consts: dict[str, int]) -> list[int]:
    return [value(t, consts) for t in re.findall(r"\bsound\(([A-Za-z_0-9]+)\)", block)]


def chunk_writes(block: str) -> list[list[tuple[int, int, int]]]:
    out = []
    for body in re.findall(r"WriteMapChunks\s*\{\s*chunks:\s*&\[(.*?)\]\s*,?\s*\}", block, re.S):
        out.append(
            [
                (num(a), num(b), num(c))
                for a, b, c in re.findall(
                    r"\(\s*(0x[0-9A-Fa-f]+|\d+)\s*,\s*(0x[0-9A-Fa-f]+|\d+)\s*,\s*(0x[0-9A-Fa-f]+|\d+)\s*\)", body
                )
            ]
        )
    return out


# ---------------------------------------------------------------------------
# Platforms
# ---------------------------------------------------------------------------
PLATFORMS = {
    # event: (registry static, source file, first free object slot, scene name)
    0x15: ("VAH_FORT_MOVING_PLATFORM_1", "vahal_fort.rs", 3),
    0x16: ("VAH_FORT_MOVING_PLATFORM_2", "vahal_fort.rs", 3),
    0x17: ("WPN_PLNT_MOVING_PLATFORM_1", "weapon_plant.rs", 5),
    0x18: ("WPN_PLNT_MOVING_PLATFORM_2", "weapon_plant.rs", 5),
    0x19: ("WPN_PLNT_MOVING_PLATFORM_3", "weapon_plant.rs", 5),
    0x1A: ("WPN_PLNT_MOVING_PLATFORM_4", "weapon_plant.rs", 5),
}

# `Field_LoadObject`, `move.w #$124,(a4)`, `Event_OverlapCharacters`, `GetMapLayoutOffset`,
# the 60-frame settle, the loop sound, the ride loop's `lsl.l #8` and the closing writes.
LOAD_OBJECT = bytes.fromhex("4EB900044D82")
SET_OBJECT_ID = bytes.fromhex("38BC0124")
OVERLAP = bytes.fromhex("4EB90005A87A")
LAYOUT_OFFSET = bytes.fromhex("4EB900053514")
SETTLE = bytes.fromhex("7E3B4EB90004204C51CFFFF8")
LOOP_SOUND = bytes.fromhex("13FC00E7FFFF500A")
STOP_SOUND = bytes.fromhex("13FC00FCFFFF500A")
SHIFT_STEP = bytes.fromhex("48C0E188")  # ext.l d0 / lsl.l #8, d0


def decode_platform(rom: bytes, code: bytes) -> dict:
    """One platform routine's literals, with its structure asserted on the way."""
    problems = []
    if code[:2] != bytes.fromhex("47FA"):
        problems.append("does not open with `lea table(pc), a3`")
    start_table = 2 + s16(code, 2)
    if code[4] != 0x70:
        problems.append("the flag is not a `moveq`")
    flag = code[5]
    for marker, label in [
        (jsr("temp_test"), "TempEveFlags_Test"),
        (LOAD_OBJECT, "Field_LoadObject"),
        (SET_OBJECT_ID, "the $124 object id"),
        (OVERLAP, "Event_OverlapCharacters"),
        (SETTLE, "the 60-frame settle"),
        (LOOP_SOUND, "the loop sound"),
        (SHIFT_STEP, "ext.l / lsl.l #8"),
        (bytes([0x70, flag]) + bytes.fromhex(jsr("temp_toggle")), "the temp-flag toggle"),
        (STOP_SOUND, "the stop sound"),
    ]:
        marker_bytes = bytes.fromhex(marker) if isinstance(marker, str) else marker
        if marker_bytes not in code:
            problems.append(f"lacks {label}")
    if code.count(LAYOUT_OFFSET) != 2:
        problems.append("does not call GetMapLayoutOffset twice")
    if STOP_SOUND + bytes.fromhex("4E75") not in code:
        problems.append("does not stop the sound and return")
    sound_at = code.index(LOOP_SOUND)
    if code[sound_at + 8 : sound_at + 10] != bytes.fromhex("47FA"):
        problems.append("the ride table's lea does not follow the loop sound")
    ride_table = sound_at + 8 + 2 + s16(code, sound_at + 10)
    rows = {"start": [], "ride": []}
    for state in range(2):
        x, y, a, b = struct.unpack_from(">HHBB", code, start_table + 6 * state)
        rows["start"].append({"x": x, "y": y, "chunks": (a, b)})
        step, x2, y2, c, d = struct.unpack_from(">hHHBB", code, ride_table + 8 * state)
        rows["ride"].append({"step": step, "x": x2, "y": y2, "chunks": (c, d)})
    return {"flag": flag, "rows": rows, "problems": problems}


def expected_platform_half(rows: dict, state: int) -> dict:
    """What the scene half for flag `state` must contain, from the two tables."""
    start, ride = rows["start"][state], rows["ride"][state]
    step_px = ride["step"] // 256
    distance = ride["y"] - start["y"]
    return {
        "object": (start["x"], start["y"]),
        "start_write": [
            (start["x"] >> 5, start["y"] >> 5, start["chunks"][0]),
            ((start["x"] >> 5) + 1, start["y"] >> 5, start["chunks"][1]),
        ],
        "step": ride["step"],
        "frames": distance // step_px if step_px and distance % step_px == 0 else None,
        "end_write": [
            (ride["x"] >> 5, ride["y"] >> 5, ride["chunks"][0]),
            ((ride["x"] >> 5) + 1, ride["y"] >> 5, ride["chunks"][1]),
        ],
        "consistent": ride["x"] == start["x"] and (distance > 0) == (step_px > 0),
    }


def source_platform_halves(text: str, static: str) -> tuple[dict, list[dict]]:
    """The scene's flag test and its two halves (flag clear, flag set)."""
    block = scene_block(text, static)
    match = re.search(r"test_temp\(\s*(0x[0-9A-Fa-f]+|\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)", block)
    assert match, f"{static} has no flag test"
    flag, if_set, if_clear = num(match.group(1)), int(match.group(2)), int(match.group(3))
    first_end = block.index("SceneOp::End,")
    halves = [block[:first_end], block[first_end:]]
    out = []
    for half in halves:
        obj = re.search(r"platform_object\(\s*[A-Z_]+\s*,\s*(0x[0-9A-Fa-f]+)\s*,\s*(0x[0-9A-Fa-f]+)\s*\)", half)
        ride = re.search(r"ride\(\s*[A-Z_]+\s*,\s*(-?0x[0-9A-Fa-f]+)\s*,\s*(\d+)\s*\)", half)
        writes = chunk_writes(half)
        toggles = [num(t) for t in re.findall(r"toggle_temp\((0x[0-9A-Fa-f]+|\d+)\)", half)]
        out.append(
            {
                "object": (num(obj.group(1)), num(obj.group(2))) if obj else None,
                "step": num(ride.group(1)) if ride else None,
                "frames": int(ride.group(2)) if ride else None,
                "writes": writes,
                "toggles": toggles,
            }
        )
    return {"flag": flag, "if_set": if_set, "if_clear": if_clear}, out


def check_platform(rom: bytes, extents, event: int, text: str, slot: int, static: str) -> list[str]:
    decoded = decode_platform(rom, routine(rom, extents, event))
    problems = [f"${event:02X} {static}: {p}" for p in decoded["problems"]]
    head, halves = source_platform_halves(text, static)
    if head["flag"] != decoded["flag"]:
        problems.append(f"${event:02X}: scene tests temp flag ${head['flag']:02X}, retail ${decoded['flag']:02X}")
    if (head["if_clear"], len(halves[0]["writes"])) != (1, 2):
        problems.append(f"${event:02X}: the clear half is not ops 1..")
    for state, half in enumerate(halves):
        label = f"${event:02X} {static} flag {'set' if state else 'clear'}"
        want = expected_platform_half(decoded["rows"], state)
        if not want["consistent"] or want["frames"] is None:
            problems.append(f"{label}: retail tables are inconsistent ({want})")
            continue
        if half["object"] != want["object"]:
            problems.append(f"{label}: platform object at {half['object']}, retail {want['object']}")
        if len(half["writes"]) != 2:
            problems.append(f"{label}: {len(half['writes'])} chunk writes, retail 2")
        else:
            if half["writes"][0] != want["start_write"]:
                problems.append(f"{label}: start write {half['writes'][0]}, retail {want['start_write']}")
            if half["writes"][1] != want["end_write"]:
                problems.append(f"{label}: end write {half['writes'][1]}, retail {want['end_write']}")
        if half["step"] != want["step"]:
            problems.append(f"{label}: step {half['step']}, retail {want['step']}")
        if half["frames"] != want["frames"]:
            problems.append(f"{label}: {half['frames']} frames, retail distance/step is {want['frames']}")
        if half["toggles"] != [decoded["flag"]]:
            problems.append(f"{label}: toggles {half['toggles']}, retail ${decoded['flag']:02X}")
    # The chunks the scene writes are the ones the atlas rows carry.
    return problems


# ---------------------------------------------------------------------------
# Terminals
# ---------------------------------------------------------------------------
TERMINALS = {
    0x1B: ("VAHAL_FORT_TERMINAL", "vahal_fort.rs"),
    0x1C: ("WEAPON_PLANT_TERMINAL", "weapon_plant.rs"),
}


def decode_terminal(code: bytes) -> dict:
    hexed = code.hex().upper()
    alarm = "13FC00DBFFFF500A11FC0003ED52"
    problems = []
    if hexed.count(alarm) != 2:
        problems.append("does not sound the alarm twice with `$ED52 = 3`")
    if jsr_hex(0x5B2BE) not in hexed or jsr_hex(0x5B30E) not in hexed:
        problems.append("lacks the red fades")
    match = re.search(r"49F8C0000C6C([0-9A-F]{4})00306([0-9A-F])([0-9A-F]{2})70([0-9A-F]{2})4EF9000576EA70([0-9A-F]{2})4EF9000576EA", hexed)
    if match is None:
        problems.append("the compare / branch / toggle tail is not the expected shape")
        return {"problems": problems}
    return {
        "threshold": int(match.group(1), 16),
        "branch": "bcs" if match.group(2) == "5" else "other",
        "at_least": int(match.group(4), 16),
        "below": int(match.group(5), 16),
        "problems": problems,
    }


def jsr_hex(address: int) -> str:
    return "4EB9" + f"{address:08X}"


def check_terminal(rom: bytes, extents, event: int, text: str, static: str, consts: dict[str, int]) -> list[str]:
    decoded = decode_terminal(routine(rom, extents, event))
    problems = [f"${event:02X}: {p}" for p in decoded["problems"]]
    if problems:
        return problems
    block = scene_block(text, static)
    cmp = re.search(r"value:\s*(0x[0-9A-Fa-f]+),\s*if_true:\s*(\d+),\s*if_false:\s*(\d+)", block)
    toggles = [num(t) for t in re.findall(r"toggle_temp\((0x[0-9A-Fa-f]+|\d+)\)", block)]
    if "CoordCmp::Below" not in block or "Axis::X" not in block:
        problems.append(f"${event:02X}: the branch is not `cmpi.w #v, $30(a4)` / `bcs`")
    if decoded["branch"] != "bcs":
        problems.append(f"${event:02X}: retail branch is not bcs")
    if num(cmp.group(1)) != decoded["threshold"]:
        problems.append(f"${event:02X}: threshold {num(cmp.group(1)):#x}, retail {decoded['threshold']:#x}")
    # Source order: the `x >= threshold` toggle, `End`, then the `x < threshold` toggle.
    if toggles != [decoded["at_least"], decoded["below"]]:
        problems.append(f"${event:02X}: toggles {toggles}, retail {[decoded['at_least'], decoded['below']]}")
    if (int(cmp.group(2)), int(cmp.group(3))) != (7, 5):
        problems.append(f"${event:02X}: branch targets are not (below -> 7, at-least -> 5)")
    if sounds(block, consts) != [0xDB, 0xDB]:
        problems.append(f"${event:02X}: sounds {sounds(block, consts)}")
    return problems


# ---------------------------------------------------------------------------
# Belts
# ---------------------------------------------------------------------------
BELTS = {
    0x1D: ("Down", "VERTICAL"),
    0x1E: ("Up", "VERTICAL"),
    0x1F: ("Right", "HORIZONTAL"),
    0x20: ("Left", "HORIZONTAL"),
}


def decode_belt(code: bytes) -> dict:
    hexed = code.hex().upper()
    problems = []
    if not hexed.startswith("4238ECE0"):
        problems.append("does not clear FieldObj_Step_Offset first")
    if "11FC0001ECE0" not in hexed:
        problems.append("does not restore it to 1")
    if LOOP_SOUND.hex().upper() not in hexed or STOP_SOUND.hex().upper() not in hexed:
        problems.append("lacks the loop and stop sounds")
    if jsr_hex(0x5A87A) not in hexed:
        problems.append("lacks Event_OverlapCharacters")
    chunks = re.findall(r"0C0700([0-9A-F]{2})65[0-9A-F]{2}0C0700([0-9A-F]{2})62", hexed)
    # The same range twice: the start test and the loop test.
    if len(chunks) != 2 or chunks[0] != chunks[1]:
        problems.append(f"chunk range tests are not the same pair twice: {chunks}")
        return {"problems": problems}
    dest = re.search(r"302C00(34|30)0640([0-9A-F]{4})3940003(A|8)", hexed)
    if dest is None:
        problems.append("the leader destination write is not the expected shape")
        return {"problems": problems}
    axis = "y" if dest.group(1) == "34" else "x"
    delta = struct.unpack(">h", bytes.fromhex(dest.group(2)))[0]
    if (axis, dest.group(3)) not in (("y", "A"), ("x", "8")):
        problems.append("the destination word is not the read axis's")
    direction = {("y", 16): "Down", ("y", -16): "Up", ("x", 16): "Right", ("x", -16): "Left"}.get((axis, delta))
    return {
        "direction": direction,
        "lo": int(chunks[0][0], 16),
        "hi": int(chunks[0][1], 16),
        "problems": problems,
    }


def check_belt(rom: bytes, extents, event: int, text: str, consts: dict[str, int]) -> list[str]:
    name, range_name = BELTS[event]
    decoded = decode_belt(routine(rom, extents, event))
    problems = [f"${event:02X}: {p}" for p in decoded["problems"]]
    if problems:
        return problems
    if decoded["direction"] != name:
        problems.append(f"${event:02X}: retail carries {decoded['direction']}, scene is {name}")
    want = (consts[range_name + "_LO"], consts[range_name + "_HI"])
    if (decoded["lo"], decoded["hi"]) != want:
        problems.append(f"${event:02X}: retail chunk range {decoded['lo']:#x}..{decoded['hi']:#x}, scene {want}")
    pattern = rf"belt\(Direction::{name},\s*{range_name}\)"
    if not re.search(pattern, text):
        problems.append(f"${event:02X}: the scene is not `belt(Direction::{name}, {range_name})`")
    return problems


# ---------------------------------------------------------------------------
# Chests, story scenes and the Dominators
# ---------------------------------------------------------------------------
def decode_chest(code: bytes) -> dict:
    hexed = code.hex().upper()
    obj = re.search(r"49F8(C[0-9A-F]{3})7004" + jsr_hex(JSR["face"]), hexed)
    skill = re.search(r"41F8F880117C([0-9A-F]{4})([0-9A-F]{4})", hexed)
    flag = re.search(r"(?:70([0-9A-F]{2})|103C00([0-9A-F]{2}))" + jmp("flag_set") + "$", hexed)
    return {
        "chest_object": (int(obj.group(1), 16) - 0xC300) // 0x40 if obj else None,
        "face": 4 if obj else None,
        "sounds": [int(x, 16) for x in re.findall(r"13FC00([0-9A-F]{2})FFFF500A", hexed)],
        "waits": [int(x, 16) + 1 for x in re.findall(r"70([0-9A-F]{2})" + jsr_hex(JSR["map_update_loop"]), hexed)],
        "entries": [int(x, 16) for x in re.findall(r"70([0-9A-F]{2})" + jsr_hex(JSR["run_dialogue"]), hexed)],
        "resumes": hexed.count(jsr_hex(JSR["run_dialogue_resume"])),
        "skill": (int(skill.group(1), 16) & 0xFF, int(skill.group(2), 16)) if skill else None,
        "flag": int(flag.group(1) or flag.group(2), 16) if flag else None,
    }


CHESTS = {
    0x6E: ("BURSTROC", "weapon_plant.rs", 7, 8, 5, 0x0E, 0x74),
    0x6F: ("POSI_BOLT", "vahal_fort.rs", 8, 6, 6, 0x0F, 0x90),
}

SKILLS_OFFSET = 0x62  # `skills` in the character record (`ps4.constants.asm:56`)


def check_chest(rom: bytes, extents, event: int, text: str, consts: dict[str, int]) -> list[str]:
    static, _, chest, entry, slot, skill, flag = CHESTS[event]
    got = decode_chest(routine(rom, extents, event))
    problems = []
    if got["chest_object"] != chest:
        problems.append(f"${event:02X}: retail faces object {got['chest_object']}, expected {chest}")
    if got["entries"] != [entry]:
        problems.append(f"${event:02X}: retail dialogue entries {got['entries']}, expected [{entry}]")
    if got["resumes"] != 1:
        problems.append(f"${event:02X}: retail resumes {got['resumes']} dialogues, expected 1")
    if got["skill"] is None:
        problems.append(f"${event:02X}: no skill write")
    else:
        # `move.b #skill, offset(a0)` encodes as `117C 00ss dddd`: skill, then displacement.
        written_skill, displacement = got["skill"]
        if written_skill != skill or displacement != SKILLS_OFFSET + slot:
            problems.append(
                f"${event:02X}: retail writes skill {written_skill:#x} at +{displacement:#x}, "
                f"expected {skill:#x} at +{SKILLS_OFFSET + slot:#x}"
            )
    if got["flag"] != flag:
        problems.append(f"${event:02X}: retail flag {got['flag']}, expected {flag:#x}")
    block = scene_block(text, static)
    if sounds(block, consts) != got["sounds"]:
        problems.append(f"${event:02X}: sounds {sounds(block, consts)}, retail {got['sounds']}")
    waits = [int(w) for w in re.findall(r"\bwait\((\d+)\)", block)]
    if waits != got["waits"]:
        problems.append(f"${event:02X}: waits {waits}, retail {got['waits']}")
    if f"actor: ActorRef::Npc({chest})" not in block or "facing: Direction::Up" not in block:
        problems.append(f"${event:02X}: the lid is not `Face(Npc({chest}), Up)`")
    if [num(t) for t in re.findall(r"standard\((0x[0-9A-Fa-f]+|\d+)\)", block)] != [entry]:
        problems.append(f"${event:02X}: scene dialogue entries differ from [{entry}]")
    skill_op = re.search(r"slot:\s*(\d+),\s*skill:\s*(0x[0-9A-Fa-f]+)", block)
    if (int(skill_op.group(1)), num(skill_op.group(2))) != (slot, skill):
        problems.append(f"${event:02X}: scene skill slot/id differ from retail ({slot}, {skill:#x})")
    if [num(t) for t in re.findall(r"set_event\((0x[0-9A-Fa-f]+|\d+)\)", block)] != [flag]:
        problems.append(f"${event:02X}: scene sets another flag than {flag:#x}")
    return problems


STORY = {
    0x8D: ("VAHAL_FORT_ENTRANCE", "vahal_fort.rs", 0, 0xB4),
    0x8E: ("VAHAL_FORT_MIDWAY", "vahal_fort.rs", 3, 0xB5),
    0x92: ("WEAPON_PLANT_ARRIVAL", "weapon_plant.rs", 7, 0xC7),
}


def check_story(rom: bytes, extents, event: int, text: str) -> list[str]:
    static, _, entry, flag = STORY[event]
    hexed = routine(rom, extents, event).hex().upper()
    want = f"70{entry:02X}" + jsr_hex(JSR["run_dialogue"]) + f"103C00{flag:02X}" + jmp("flag_set")
    problems = []
    if hexed != want:
        problems.append(f"${event:02X}: retail bytes are {hexed}, not the expected `{want}`")
    block = scene_block(text, static)
    if f"standard({entry})" not in block or f"set_event(0x{flag:02X})" not in block:
        problems.append(f"${event:02X}: scene is not standard({entry}) then set_event(0x{flag:02X})")
    return problems


def check_barrier(rom: bytes, extents, text: str) -> list[str]:
    hexed = routine(rom, extents, 0x90).hex().upper()
    want = (
        "103C00B9" + jsr_hex(JSR["flag_test"]) + "6612" + "7001" + jsr_hex(JSR["run_dialogue"])
        + "103C00B9" + jmp("flag_set") + "7002" + jmp("run_dialogue")
    )
    problems = []
    if hexed != want:
        problems.append(f"$90: retail bytes are {hexed}, not `{want}`")
    block = scene_block(text, "VAHAL_FORT_BARRIER")
    for needle in ("Flag::event(0xB9)", "if_set: 4", "if_clear: 1", "standard(1)", "set_event(0xB9)", "standard(2)"):
        if needle not in block:
            problems.append(f"$90: scene lacks `{needle}`")
    return problems


DOMINATOR_CHARACTERS = {0: "CHAZ", 3: "RUNE", 5: "RIKA", 7: "WREN"}
FIFTH_CANDIDATES = (2, 4, 6, 8, 9, 10)
CHARACTER_NAMES = {2: "HAHN", 4: "GRYZ", 6: "DEMI", 8: "RAJA", 9: "KYRA", 10: "SETH"}


def decode_dominators(rom: bytes, code: bytes, start: int) -> dict:
    hexed = code.hex().upper()
    placements = re.findall(
        r"70([0-9A-F]{2})" + jsr_hex(JSR["get_character"]) + r"397C([0-9A-F]{4})0038397C([0-9A-F]{4})003A", hexed
    )
    candidates = re.findall(r"70([0-9A-F]{2})" + jsr_hex(JSR["get_character"]) + r"6B(?:02|0C)", hexed)
    # The palette: `move.w #v12,$18(a0)` / `move.w #v13,$1A(a0)` before the table, then the
    # table the loop reads (`lea table.l, a1`).
    palette_start = re.search(r"317C([0-9A-F]{4})0018317C([0-9A-F]{4})001A", hexed)
    table = re.search(r"43F9([0-9A-F]{8})", hexed)
    first_table = int(table.group(1), 16) if table else None
    words = []
    if first_table is not None:
        at = first_table
        while u16(rom, at) != 0xFFFF:
            words.append((u16(rom, at), u16(rom, at + 10)))
            at += 2
    return {
        "placements": [(int(c, 16), int(x, 16), int(y, 16)) for c, x, y in placements],
        "candidates": [int(c, 16) for c in candidates],
        "palette_start": (int(palette_start.group(1), 16), int(palette_start.group(2), 16)) if palette_start else None,
        "palette_rows": words,
        "dialogue_entries": [int(x, 16) for x in re.findall(r"70([0-9A-F]{2})" + jsr_hex(JSR["run_dialogue"]), hexed)],
        "flag": re.search(r"103C00([0-9A-F]{2})" + jsr_hex(JSR["flag_set"]), hexed),
        "tail": hexed[hexed.index("4238ECE0") :] if "4238ECE0" in hexed else "",
        "head_waits": re.findall(r"303C([0-9A-F]{4})4EB90005A73C", hexed),
    }


def check_dominators(rom: bytes, extents, text: str, consts: dict[str, int]) -> list[str]:
    start, end = extents[0x91]
    got = decode_dominators(rom, rom[start:end], start)
    block = scene_block(text, "DOMINATORS_DEFEATED")
    problems = []
    # Placements: the four fixed members, then (below) the fifth's `($2F0,$2E0)`.
    fixed = got["placements"][:4]
    source_go = re.findall(
        r"go\(\s*([A-Z]+)\s*,\s*(0x[0-9A-Fa-f]+)\s*,\s*(0x[0-9A-Fa-f]+)\s*\)", block
    )
    source_fixed = [(name, num(x), num(y)) for name, x, y in source_go[:4]]
    want_fixed = [(DOMINATOR_CHARACTERS[c], x, y) for c, x, y in fixed]
    if source_fixed != want_fixed:
        problems.append(f"$91: scene destinations {source_fixed}, retail {want_fixed}")
    # The fifth member's destination is the `loc_73296` pair, reached by a branch,
    # so it is read as the two immediates just before the tail (`clr.b $ECE0`).
    tail = routine(rom, extents, 0x91).hex().upper()
    gap = re.search(r"397C([0-9A-F]{4})0038397C([0-9A-F]{4})003A4238ECE0", tail)
    if gap is None:
        problems.append("$91: the fifth member's destination is not where the retail tail puts it")
    else:
        gap_xy = (int(gap.group(1), 16), int(gap.group(2), 16))
        movers = source_go[4:]
        if any((num(x), num(y)) != gap_xy for _, x, y in movers):
            problems.append(f"$91: a fifth-member destination differs from retail {gap_xy}")
        if [name for name, _, _ in movers] != [CHARACTER_NAMES[c] for c in FIFTH_CANDIDATES]:
            problems.append(f"$91: fifth-member order {[n for n, _, _ in movers]}")
    if got["candidates"] != list(FIFTH_CANDIDATES):
        problems.append(f"$91: retail tests members {got['candidates']}, expected {list(FIFTH_CANDIDATES)}")
    source_tests = re.findall(r"BranchIfPartyMember\s*\{\s*who:\s*([A-Z]+)", block)
    if source_tests != [CHARACTER_NAMES[c] for c in got["candidates"]]:
        problems.append(f"$91: scene tests members {source_tests}, retail {got['candidates']}")
    # The palette rows.
    palette = [
        (num(a), num(b))
        for a, b in re.findall(r"first:\s*(0x[0-9A-Fa-f]+),\s*second:\s*(0x[0-9A-Fa-f]+)", block)
    ]
    want_palette = [got["palette_start"]] + got["palette_rows"]
    if palette != want_palette:
        problems.append(f"$91: palette words {palette}, retail {want_palette}")
    if got["dialogue_entries"][:1] != [5] or "standard(5)" not in block:
        problems.append("$91: the first dialogue is not entry 5")
    if got["flag"] is None or f"set_event(0x{int(got['flag'].group(1), 16):02X})" not in block:
        problems.append("$91: the flag set differs from retail")
    leader = re.search(r"303C([0-9A-F]{4})323C([0-9A-F]{4})" + jsr_hex(0x5A9FC), routine(rom, extents, 0x91).hex().upper())
    if leader is None or f"x: 0x{int(leader.group(1), 16):X}" not in block or f"y: 0x{int(leader.group(2), 16):X}" not in block:
        problems.append("$91: the leader's final destination differs from retail")
    # Waits: `DoMainUpdatesLoop($77)`, the `moveq #$77` loop, `DoMainUpdatesLoop($3B)`.
    if [num(w) for w in re.findall(r"\bwait\((\d+)\)", block)] != [120, 120, 60]:
        problems.append("$91: the waits are not 120, 120, 60")
    if "bits: 0b11" not in block or "SceneOp::SetFollowMode { bits: 0 }" not in block:
        problems.append("$91: the follow-mode bits are not bset 0 and 1, then bclr")
    return problems


# ---------------------------------------------------------------------------
@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class VahalEvents(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = ROM.read_bytes()
        cls.extents = event_extents(cls.rom)
        cls.common = constants(source("vahal_common.rs"))
        cls.fort = source("vahal_fort.rs")
        cls.plant = source("weapon_plant.rs")
        cls.belts = source("conveyors.rs")
        cls.belt_consts = constants(cls.belts)

    def text(self, name: str) -> str:
        return {"vahal_fort.rs": self.fort, "weapon_plant.rs": self.plant}[name]

    def test_the_nineteen_events_are_where_the_documents_say(self) -> None:
        docs = {
            0x15: (0x6C478, 0x6C608), 0x16: (0x6C608, 0x6C798), 0x17: (0x6C798, 0x6C928),
            0x18: (0x6C928, 0x6CAB8), 0x19: (0x6CAB8, 0x6CC48), 0x1A: (0x6CC48, 0x6CDD4),
            0x1B: (0x6CDD4, 0x6CE18), 0x1C: (0x6CE18, 0x6CE5C), 0x1D: (0x6CE5C, 0x6CFA4),
            0x1E: (0x6CFA4, 0x6D0EC), 0x1F: (0x6D0EC, 0x6D234), 0x20: (0x6D234, 0x6D37C),
            0x6E: (0x724A4, 0x72520), 0x6F: (0x72520, 0x7259E), 0x8D: (0x7318E, 0x731A0),
            0x8E: (0x731A0, 0x731B2), 0x90: (0x731DA, 0x73200), 0x91: (0x73200, 0x733BA),
            0x92: (0x733BA, 0x733CC),
        }
        for event, extent in docs.items():
            self.assertEqual(self.extents[event], extent, f"event ${event:02X}")

    def test_every_platform_matches_its_retail_tables(self) -> None:
        problems = []
        for event, (static, name, slot) in PLATFORMS.items():
            problems += check_platform(self.rom, self.extents, event, self.text(name), slot, static)
        self.assertEqual(problems, [], "\n".join(problems))

    def test_the_platform_slots_are_the_first_free_object_of_their_maps(self) -> None:
        # Vahal Fort F2 holds three map objects, Weapon Plant F1 five
        # (`docs/field/PLATFORMS_AND_BELTS.md`); `Field_LoadObject` takes the next slot.
        for static, name, slot in PLATFORMS.values():
            block = scene_block(self.text(name), static)
            self.assertIn(f"PLATFORM_SLOT", block, static)
        self.assertIn("const PLATFORM_SLOT: usize = 3;", self.fort)
        self.assertIn("const PLATFORM_SLOT: usize = 5;", self.plant)

    def test_both_terminals_match_retail(self) -> None:
        problems = []
        for event, (static, name) in TERMINALS.items():
            problems += check_terminal(self.rom, self.extents, event, self.text(name), static, self.common)
        self.assertEqual(problems, [], "\n".join(problems))

    def test_every_belt_matches_retail(self) -> None:
        problems = []
        for event in BELTS:
            problems += check_belt(self.rom, self.extents, event, self.belts, self.belt_consts)
        self.assertEqual(problems, [], "\n".join(problems))

    def test_both_chest_events_match_retail(self) -> None:
        problems = []
        for event, (_, name, *_rest) in CHESTS.items():
            problems += check_chest(self.rom, self.extents, event, self.text(name), self.common)
        self.assertEqual(problems, [], "\n".join(problems))

    def test_the_story_scenes_match_retail(self) -> None:
        problems = []
        for event, (_, name, *_rest) in STORY.items():
            problems += check_story(self.rom, self.extents, event, self.text(name))
        problems += check_barrier(self.rom, self.extents, self.fort)
        self.assertEqual(problems, [], "\n".join(problems))

    def test_the_dominators_aftermath_matches_retail(self) -> None:
        problems = check_dominators(self.rom, self.extents, self.fort, self.common)
        self.assertEqual(problems, [], "\n".join(problems))

    def test_every_platform_write_has_an_atlas_row(self) -> None:
        # `SCENE_CHUNK_WRITES` carries the chunks the runtime resolves each write
        # against; `tests.test_scene_chunk_atlas` holds the pack to them.
        from psiv_tools import map_patches

        rows = {row.scene: row for row in map_patches.SCENE_CHUNK_WRITES}
        names = {
            0x15: "Event_VahFortMovingPlatform1", 0x16: "Event_VahFortMovingPlatform2",
            0x17: "Event_WpnPlntMovingPlatform1", 0x18: "Event_WpnPlntMovingPlatform2",
            0x19: "Event_WpnPlntMovingPlatform3", 0x1A: "Event_WpnPlntMovingPlatform4",
        }
        for event, (static, name, _) in PLATFORMS.items():
            written = {chunk for write in chunk_writes(scene_block(self.text(name), static)) for _, _, chunk in write}
            row = rows[names[event]]
            self.assertEqual(set(map_patches.chunk_ids(self.rom, row)), written, names[event])
            self.assertEqual(row.map_id, 0x0CA if event < 0x17 else 0x0C5, names[event])


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class NegativeControl(unittest.TestCase):
    """A changed literal or byte is named by the comparison."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = ROM.read_bytes()
        cls.extents = event_extents(cls.rom)
        cls.common = constants(source("vahal_common.rs"))
        cls.fort = source("vahal_fort.rs")
        cls.plant = source("weapon_plant.rs")
        cls.belts = source("conveyors.rs")

    def test_a_wrong_frame_count_is_named(self) -> None:
        # Weapon Plant platform 2 rides 112 frames; the draft of this scene said 64.
        mutated = self.plant.replace("ride(PLATFORM_SLOT, 0x0200, 112),", "ride(PLATFORM_SLOT, 0x0200, 64),", 4)
        self.assertNotEqual(mutated, self.plant)
        problems = []
        for event, (static, name, slot) in PLATFORMS.items():
            if name == "weapon_plant.rs":
                problems += check_platform(self.rom, self.extents, event, mutated, slot, static)
        self.assertTrue(any("64 frames, retail distance/step is 112" in p for p in problems), problems)

    def test_a_moved_chunk_pair_is_named(self) -> None:
        mutated = self.fort.replace("chunks: &[(22, 17, 0xC2), (23, 17, 0xC3)]", "chunks: &[(22, 17, 0xC2), (23, 17, 0xC4)]")
        self.assertNotEqual(mutated, self.fort)
        problems = check_platform(self.rom, self.extents, 0x15, mutated, 3, "VAH_FORT_MOVING_PLATFORM_1")
        self.assertTrue(any("start write" in p and "0xC4" not in p for p in problems) or problems, problems)
        self.assertTrue(any("flag clear" in p for p in problems), problems)

    def test_a_wrong_terminal_threshold_is_named(self) -> None:
        mutated = self.fort.replace("value: 0x1F0", "value: 0x1E0")
        self.assertNotEqual(mutated, self.fort)
        problems = check_terminal(self.rom, self.extents, 0x1B, mutated, "VAHAL_FORT_TERMINAL", self.common)
        self.assertTrue(any("threshold 0x1e0, retail 0x1f0" in p for p in problems), problems)

    def test_a_swapped_belt_range_is_named(self) -> None:
        consts = constants(self.belts)
        consts["VERTICAL_HI"] = 0xAF
        problems = check_belt(self.rom, self.extents, 0x1D, self.belts, consts)
        self.assertTrue(any("retail chunk range 0xa8..0xab" in p for p in problems), problems)

    def test_a_flipped_belt_direction_is_named(self) -> None:
        mutated = self.belts.replace("belt(Direction::Up, VERTICAL)", "belt(Direction::Down, VERTICAL)")
        problems = check_belt(self.rom, self.extents, 0x1E, mutated, constants(self.belts))
        self.assertTrue(any("scene is not `belt(Direction::Up, VERTICAL)`" in p for p in problems), problems)

    def test_a_changed_image_byte_is_named(self) -> None:
        rom = bytearray(self.rom)
        # Burstroc's skill id (`move.b #$0E, $67(a0)`) is the byte after `117C 00`.
        at = rom.index(bytes.fromhex("41F8F880117C000E0067"), 0x724A4) + 7
        rom[at] = 0x0D
        problems = check_chest(bytes(rom), self.extents, 0x6E, self.plant, self.common)
        self.assertTrue(any("retail writes skill 0xd" in p for p in problems), problems)

    def test_a_wrong_dialogue_entry_is_named(self) -> None:
        mutated = self.fort.replace("ops: &[standard(3), set_event(0xB5)]", "ops: &[standard(4), set_event(0xB5)]")
        self.assertNotEqual(mutated, self.fort)
        problems = check_story(self.rom, self.extents, 0x8E, mutated)
        self.assertTrue(any("scene is not standard(3)" in p for p in problems), problems)

    def test_a_missing_membership_test_is_named(self) -> None:
        mutated = re.sub(
            r"SceneOp::BranchIfPartyMember \{\s*who: KYRA,.*?\},\n", "", self.fort, count=1, flags=re.S
        )
        self.assertNotEqual(mutated, self.fort)
        problems = check_dominators(self.rom, self.extents, mutated, self.common)
        self.assertTrue(any("scene tests members" in p for p in problems), problems)


if __name__ == "__main__":
    unittest.main()
