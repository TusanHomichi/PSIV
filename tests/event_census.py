"""The event census: every `Event_Index` the cartridge can fire, from the ROM.

`tests/test_event_coverage.py` is the guard; this module is the derivation, so
the guard, the document generator (`python3 -m tests.event_census --doc`) and
the Rust table generator (`--rust`) read one source.

An event index reaches the field's event dispatcher (`FieldRoutine_Event`,
`$5A266`) only through a write to `Event_Index` (`$FFFFECA8`, the word
`$ECA8` as an absolute-short operand) or through a direct call to an event
routine. Both are read from the US image:

1. **Every operand `$ECA8` below `$80000`** is classified by the instruction
   before it. An immediate write (`move.w #imm, Event_Index.w`, `31FC`), the
   two register writers, and the readers are known shapes; anything else fails
   the census naming its address, so a new writer cannot hide.
2. **Trigger writers** are the immediates inside a `RunEventsJmpTbl` routine
   (`$560E8`, 128 `bra.w` entries; `psiv_tools.newgame.run_events_jmp_tbl`). A
   routine is reachable when some map's event list (`psiv_tools.maps`) names a
   slot that reaches it.
3. **Interaction areas.** `Interaction_GetEvent` (`InteractionRoutines[2]`,
   `$58A4C`) loads `Interaction_EventIndexes` (the `lea` at `$58A4C`) and writes
   `table[d4]`; `d4` is byte 9 of a map interaction area whose routine byte is 2.
4. **Dialogue.** `GetEventFromDialogue` (`$58C9C`) writes the big-endian word
   after a `$F6` control byte; every `$F6` in every tree is read from the
   decoded trees (`psiv_tools.text.extract_dialogue`).
5. **Item and field-input writers** are the remaining immediate sites, named in
   `OTHER_SITES` (title start, leaving a vehicle, the vehicle and Pennant and
   Wood Carving item actions).
6. **Direct calls.** An absolute `jsr`/`jmp`, a `bsr`/`bra` or a PC-relative
   `jsr`/`jmp` that lands on the start of an `EventPtrs`/`CutscenePtrs`
   routine runs that event without `Event_Index`. A call from inside another
   event's own byte range is a *chain*: the callee is part of the caller's
   scene. Any other call site is named in `DIRECT_SITES`.

An event whose routine starts with `rts` (`4E75`) is null: firing it does
nothing, so it needs no scene.
"""
from __future__ import annotations

import re
import struct
import sys
from dataclasses import dataclass, field
from pathlib import Path

from psiv_tools import newgame
from psiv_tools.maps import extract_maps
from psiv_tools.text import extract_dialogue

ROOT = Path(__file__).resolve().parents[1]
ROM = ROOT / "Phantasy Star IV (USA).md"
SCENES_DIR = ROOT / "rust" / "psiv-core" / "src" / "scenes"
ASM = ROOT / "reference" / "ps4disasm" / "ps4.asm"

EVENT_INDEX_OPERAND = 0xECA8
#: Everything the 68000 executes (and every event body) lies below this.
CODE_END = 0x80000
EVENT_PTRS = newgame.EVENT_PTRS
EVENT_COUNT = 161
CUTSCENE_PTRS = 0x05A580
CUTSCENE_COUNT = 34
CUTSCENE_BIT = 0x8000
INTERACTION_ROUTINES = 0x058836
INTERACTION_GET_EVENT = 0x058A4C
INTERACTION_TABLE_WORDS = 32
GET_EVENT_FROM_DIALOGUE = 0x058C9C

#: Immediate `Event_Index` writers that are not `RunEventsJmpTbl` routines.
OTHER_SITES = {
    0x043792: "Title_StartOption: Start on the title screen",
    0x056088: "field input: Cancel/Start-class press while on a vehicle (`loc_56084`)",
    0x05C14E: "ItemAction_LandRover: the ITEM menu boards the Land Rover",
    0x05C174: "ItemAction_IceDigger: the ITEM menu boards the Ice Digger",
    0x05C19A: "ItemAction_Hydrofoil: the ITEM menu boards the Hydrofoil",
    0x05C1B6: "ItemAction_Pennant: ITEM menu, Chaz's house only",
    0x05C1D2: "ItemAction_WoodCarvin: ITEM menu, Chaz's house only",
}
#: Register writers, by operand address: `(opcode word, description)`.
REGISTER_WRITERS = {
    0x058A5E: (0x31C4, "Interaction_GetEvent: `move.w d4, Event_Index`"),
    0x058CAA: (0x31C0, "GetEventFromDialogue: `move.w d0, Event_Index`"),
}
#: Direct-call sites outside any event body.
DIRECT_SITES = {
    0x066190: "the Aiedo shop/inn routine (`Event_GirlsSneakingOut`, `ps4.asm` shop code)",
    0x064C44: "the ship-arrival routine's tail jump to `Cutscene_Rykros`",
}

NULL_RTS = 0x4E75

#: Where each retail event is reached in play, by tracking issue. Every entry
#: is an event the census finds that has no scene. The list may only shrink:
#: `test_event_coverage` rejects an entry whose event is registered, an event
#: that is neither registered nor listed, and a list longer than
#: `ALLOWLIST_CEILING`.
ISSUE_TRIGGERS = 56
ISSUE_DIALOGUE2 = 71
ISSUE_MOTAVIA = 81
ISSUE_VAHAL = 82
ISSUE_DEZOLIS = 83
ISSUES = (ISSUE_TRIGGERS, ISSUE_DIALOGUE2, ISSUE_MOTAVIA, ISSUE_VAHAL, ISSUE_DEZOLIS)


@dataclass
class Source:
    kind: str  # trigger | interaction | dialogue | other | direct | chain | register
    where: str
    detail: str = ""


@dataclass
class Event:
    index: int
    label: str = ""
    routine: int = 0
    null: bool = False
    sources: list[Source] = field(default_factory=list)

    @property
    def cutscene(self) -> bool:
        return bool(self.index & CUTSCENE_BIT)

    def kinds(self) -> list[str]:
        return sorted({s.kind for s in self.sources})


def w16(rom: bytes, at: int) -> int:
    return struct.unpack_from(">H", rom, at)[0]


def w32(rom: bytes, at: int) -> int:
    return struct.unpack_from(">I", rom, at)[0]


def routine_for(rom: bytes, event: int) -> int:
    if event & CUTSCENE_BIT:
        return w32(rom, CUTSCENE_PTRS + 4 * (event & 0x7FFF))
    return w32(rom, EVENT_PTRS + 4 * event)


def pointer_tables(rom: bytes) -> tuple[list[int], list[int]]:
    events = [w32(rom, EVENT_PTRS + 4 * i) for i in range(EVENT_COUNT)]
    cutscenes = [w32(rom, CUTSCENE_PTRS + 4 * i) for i in range(CUTSCENE_COUNT)]
    if cutscenes[0] != events[0]:
        raise AssertionError("CutscenePtrs[0] is not EventPtrs[0]: wrong table address")
    return events, cutscenes


def labels(asm_path: Path = ASM) -> dict[int, str]:
    """Event index -> clone label, for navigation only (the clone has no authority)."""
    if not asm_path.exists():
        return {}
    found: dict[int, str] = {}
    mode, i = None, 0
    for line in asm_path.read_text(encoding="utf-8", errors="replace").split("\n"):
        if line.startswith("EventPtrs:"):
            mode, i = 0, 0
        elif line.startswith("CutscenePtrs:"):
            mode, i = CUTSCENE_BIT, 0
        elif line.startswith(("FieldRoutine", "Cutscene_")) and mode is not None:
            mode = None
        m = re.match(r"\s*dc\.l\s+(\w+)", line)
        if mode is not None and m:
            found[mode | i] = m.group(1)
            i += 1
    return found


def event_index_sites(rom: bytes) -> list[tuple[int, str, int | None]]:
    """Every `$ECA8` operand below `CODE_END`: `(operand address, shape, value)`.

    Shapes: `imm` (an immediate write), `reg` (a known register write), `read`.
    Anything else raises, naming the address.
    """
    sites: list[tuple[int, str, int | None]] = []
    for m in re.finditer(b"\xec\xa8", rom[:CODE_END]):
        at = m.start()
        if at % 2:
            continue
        before = w16(rom, at - 2)
        if w16(rom, at - 4) == 0x31FC:
            sites.append((at, "imm", before))
        elif at in REGISTER_WRITERS and before == REGISTER_WRITERS[at][0]:
            sites.append((at, "reg", None))
        elif before == 0x3038 or (w16(rom, at - 4) == 0x0838 and before == 0x0007):
            sites.append((at, "read", None))
        else:
            raise AssertionError(
                f"${at:06X}: an `Event_Index` operand ({rom[at-6:at+2].hex()}) that is "
                "not a known write or read shape; classify it before the census can be trusted"
            )
    for m in re.finditer(b"\xff\xff\xec\xa8", rom[:CODE_END]):
        raise AssertionError(f"${m.start():06X}: a long-form `Event_Index` operand")
    return sites


def trigger_starts(rom: bytes) -> list[int]:
    table = newgame.run_events_jmp_tbl(rom)
    starts = []
    for slot in range(128):
        entry = table + 4 * slot
        if w16(rom, entry) != 0x6000:
            raise AssertionError(f"RunEventsJmpTbl slot {slot} is not a bra.w")
        starts.append(entry + 2 + struct.unpack_from(">h", rom, entry + 2)[0])
    return starts


def interaction_table(rom: bytes) -> list[int]:
    """`Interaction_EventIndexes`, located by `Interaction_GetEvent`'s own `lea`."""
    routine = INTERACTION_ROUTINES + 2 * 2
    if INTERACTION_ROUTINES + w16(rom, routine) != INTERACTION_GET_EVENT:
        raise AssertionError("InteractionRoutines[2] is not Interaction_GetEvent")
    if rom[INTERACTION_GET_EVENT : INTERACTION_GET_EVENT + 2] != b"\x41\xf9":
        raise AssertionError("Interaction_GetEvent does not start with `lea (abs).l, a0`")
    table = w32(rom, INTERACTION_GET_EVENT + 2)
    shape = bytes.fromhex("0244 00FF D844 3830 4000 31C4 ECA8".replace(" ", ""))
    if rom[INTERACTION_GET_EVENT + 6 : INTERACTION_GET_EVENT + 6 + len(shape)] != shape:
        raise AssertionError("Interaction_GetEvent is not `table[d4] -> Event_Index`")
    return [w16(rom, table + 2 * i) for i in range(INTERACTION_TABLE_WORDS)]


def dialogue_events(rom: bytes) -> dict[int, list[tuple[int, int]]]:
    found: dict[int, list[tuple[int, int]]] = {}
    for tree in extract_dialogue(rom)["trees"]:
        for entry in tree["entries"]:
            for seg in entry["segments"]:
                if seg.get("ctrl") == "0xF6":
                    value = (seg["operands"][0] << 8) | seg["operands"][1]
                    found.setdefault(value, []).append((tree["tree"], entry["id"]))
    return found


def direct_calls(rom: bytes, targets: dict[int, int]) -> list[tuple[int, int, str]]:
    """`(site, callee event, form)` for every call landing on an event routine start."""
    found = []
    for at in range(0, CODE_END - 6, 2):
        op = w16(rom, at)
        target, form = None, ""
        if op in (0x4EB9, 0x4EF9):
            target, form = w32(rom, at + 2), "abs"
        elif op in (0x4EBA, 0x4EFA):
            target, form = at + 2 + struct.unpack_from(">h", rom, at + 2)[0], "pc"
        elif (op & 0xFE00) == 0x6000 and (op & 0xF00) in (0x000, 0x100):
            disp = op & 0xFF
            if disp == 0:
                target = at + 2 + struct.unpack_from(">h", rom, at + 2)[0]
            elif disp < 0x80:
                target = at + 2 + disp
            form = "bra/bsr"
        if target in targets:
            found.append((at, targets[target], form))
    return found


def registry_events(sources: dict[str, str] | None = None) -> dict[int, str]:
    """Event index -> scene name for every scene in `rust/psiv-core/src/scenes`."""
    if sources is None:
        sources = {p.name: p.read_text(encoding="utf-8") for p in SCENES_DIR.glob("*.rs")}
    consts: dict[str, int] = {}
    for text in sources.values():
        for m in re.finditer(
            r"const (\w+): EventIndex = EventIndex\(\s*(0x[0-9A-Fa-f]+|\d+)\s*\)", text
        ):
            consts[m.group(1)] = int(m.group(2), 0)
    found: dict[int, str] = {}
    scene = re.compile(
        r"pub static (\w+): Scene = Scene \{\s*name:\s*\"([^\"]+)\",\s*event:\s*"
        r"(?:EventIndex\(\s*(0x[0-9A-Fa-f]+|\d+)\s*\)|\w+::(\w+)),",
    )
    for text in sources.values():
        for m in scene.finditer(text):
            index = int(m.group(3), 0) if m.group(3) else consts[m.group(4)]
            found[index] = m.group(2)
    return found


@dataclass
class Census:
    events: dict[int, Event]
    unreferenced_slots: list[int]
    sites: list[tuple[int, str, int | None]]


def build(rom: bytes, labels_by_index: dict[int, str] | None = None) -> Census:
    events_tbl, cutscenes_tbl = pointer_tables(rom)
    names = labels_by_index if labels_by_index is not None else labels()
    census: dict[int, Event] = {}

    def event(index: int) -> Event:
        if index not in census:
            routine = routine_for(rom, index)
            census[index] = Event(
                index=index,
                label=names.get(index, ""),
                routine=routine,
                null=w16(rom, routine) == NULL_RTS,
            )
        return census[index]

    sites = event_index_sites(rom)
    starts = trigger_starts(rom)
    maps = [m for m in extract_maps(rom)["maps"] if not m.get("is_null")]
    slot_maps: dict[int, list[str]] = {}
    for m in maps:
        for slot in m["events"]["ids"]:
            slot_maps.setdefault(slot, []).append(m["symbol"])
    lowest, highest = min(starts), max(starts) + 0x40
    reached_slots: set[int] = set()
    for at, shape, value in sites:
        if shape != "imm":
            continue
        if lowest <= at < highest:
            routine = max(s for s in starts if s <= at)
            slots = [i for i, s in enumerate(starts) if s == routine]
            used = [slot for slot in slots if slot in slot_maps]
            reached_slots.update(used)
            if used:
                symbols = sorted({sym for slot in used for sym in slot_maps[slot]})
                event(value).sources.append(
                    Source("trigger", "slots " + ",".join(f"${s:02X}" for s in used), ", ".join(symbols))
                )
        elif at in OTHER_SITES:
            event(value).sources.append(Source("other", OTHER_SITES[at]))
        else:
            raise AssertionError(f"${at:06X}: unclassified `Event_Index` immediate write {value:#X}")
    unreferenced = [i for i in range(128) if i not in slot_maps]

    table = interaction_table(rom)
    for m in maps:
        for area in m["interaction_areas"]["entries"]:
            if area["interaction_type"] == 2:
                param = area["parameter"]
                if param >= INTERACTION_TABLE_WORDS:
                    raise AssertionError(f"{m['symbol']}: interaction parameter {param:#X} is off the table")
                if table[param]:
                    event(table[param]).sources.append(
                        Source("interaction", f"interaction area (table slot ${param:02X})", m["symbol"])
                    )
    for value, places in dialogue_events(rom).items():
        trees = ", ".join(f"tree {t} entry {e}" for t, e in places)
        event(value).sources.append(Source("dialogue", "`$F6` in dialogue", trees))

    targets = {}
    for index, routine in list(enumerate(events_tbl)) + [(CUTSCENE_BIT | i, r) for i, r in enumerate(cutscenes_tbl)]:
        if w16(rom, routine) != NULL_RTS:
            targets.setdefault(routine, index)
    bounds = sorted(set(targets) | {CODE_END})
    for site, callee, form in direct_calls(rom, targets):
        start = routine_for(rom, callee)
        if start <= site < min(b for b in bounds if b > start):
            continue  # a routine looping to its own start
        owner = max((s for s in targets if s <= site), default=None)
        end = min((b for b in bounds if owner is not None and b > owner), default=CODE_END)
        if owner is not None and owner <= site < end and site < 0x7B000:
            event(callee).sources.append(
                Source("chain", f"called from event ${targets[owner]:04X} (${site:06X}, {form})", f"{targets[owner]}")
            )
        elif site in DIRECT_SITES:
            event(callee).sources.append(Source("direct", DIRECT_SITES[site]))
        else:
            raise AssertionError(f"${site:06X}: an unclassified direct call to event {callee:#X}")
    return Census(census, unreferenced, sites)


def chain_callers(ev: Event) -> list[int]:
    return [int(s.detail) for s in ev.sources if s.kind == "chain"]



#: The allowlist: events the census finds that have no scene, by tracking
#: issue. `(issue, area, events)`; an event takes its group's area as its
#: reason. A scene that lands removes its event from here (and the list
#: shrinks: `ALLOWLIST_CEILING` is the most it may ever hold).
ALLOW_GROUPS: tuple[tuple[int, str, tuple[int, ...]], ...] = (
    (ISSUE_TRIGGERS, "trigger-fired, no scene (issue #56)", (0x2A, 0x37, 0x38, 0x71, 0x96)),
    (ISSUE_DIALOGUE2, "`Event_GetAndRunDialogue2` caller, no scene (issue #71)", (0x62, 0x7D, 0x88, 0x8F)),
    (
        ISSUE_MOTAVIA,
        "Motavia side content: Aiedo, Piata, Monsen, Mile, Zosa, the Soldier's Temple, the Plate System and Wreckage",
        (
            0x0E, 0x22, 0x24, 0x29, 0x2C, 0x2D, 0x31, 0x66, 0x6C, 0x70, 0x72, 0x73, 0x74,
            0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7E, 0x7F, 0x80, 0x81, 0x82, 0x83,
            0x84, 0x85, 0x86, 0x87, 0x89, 0x93, 0x94, 0x95, 0x97, 0x98, 0x9A, 0x9B, 0x9C,
            0x9D,
        ),
    ),
    (
        ISSUE_MOTAVIA,
        "vehicle dismount from the field input (`loc_56084`); no area issue fits, closest is Motavia where the vehicles start",
        (0x10,),
    ),
    (
        ISSUE_VAHAL,
        "Vahal Fort and Weapon Plant: platforms, conveyors, terminals, bosses and gates",
        (
            0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x6E,
            0x6F, 0x8D, 0x8E, 0x90, 0x91, 0x92,
        ),
    ),
    (
        ISSUE_DEZOLIS,
        "Dezolis late arc: Garuberk Tower, the Esper and Inner Sanctuary dialogue controls, the Musk Cat and Penguin towns, Raja and Anger Tower controls",
        (
            0x35, 0x36, 0x39, 0x3A, 0x42, 0x45, 0x46, 0x49, 0x4A, 0x4B, 0x4F, 0x51,
            0x52, 0x5B, 0x5C, 0x67, 0x68, 0x6D, 0x99,
        ),
    ),
    (
        ISSUE_DEZOLIS,
        "generic recovery tile (`RunEvent_Recovery`); its maps are Dezolis late-arc rooms and the spaceports",
        (0x21,),
    ),
)
ALLOWLIST: dict[int, tuple[int, str]] = {
    event: (issue, area) for issue, area, events in ALLOW_GROUPS for event in events
}
#: The most the allowlist may hold. Lower it when a scene lands; never raise it.
ALLOWLIST_CEILING = 90


def disposition(
    census: Census,
    registry: dict[int, str],
    allowlist: dict[int, tuple[int, str]],
    index: int,
    _seen: frozenset[int] = frozenset(),
) -> tuple[str, int | None]:
    """`(kind, extra)`: `scene`, `null`, `chained` (extra: a caller), `allow` (extra: issue) or `missing`."""
    ev = census.events[index]
    if index in registry:
        return ("scene", None)
    if ev.null:
        return ("null", None)
    callers = chain_callers(ev)
    if callers and len(callers) == len(ev.sources) and index not in _seen:
        verdicts = [disposition(census, registry, allowlist, c, _seen | {index}) for c in callers]
        if all(kind in ("scene", "chained") for kind, _ in verdicts):
            return ("chained", callers[0])
    if index in allowlist:
        return ("allow", allowlist[index][0])
    return ("missing", None)


def problems(
    census: Census,
    registry: dict[int, str],
    allowlist: dict[int, tuple[int, str]] = ALLOWLIST,
    ceiling: int = ALLOWLIST_CEILING,
) -> list[str]:
    out: list[str] = []
    for index in sorted(census.events):
        ev = census.events[index]
        kind, _ = disposition(census, registry, allowlist, index)
        if kind == "missing":
            out.append(
                f"event ${index:04X} {ev.label or '?'} is fired ({'; '.join(sorted({s.kind for s in ev.sources}))}) "
                "but has no registered scene and is not allowlisted"
            )
    for index, (issue, reason) in sorted(allowlist.items()):
        if index not in census.events:
            out.append(f"allowlist entry ${index:04X} is not an event the cartridge fires")
        elif index in registry:
            out.append(
                f"allowlist entry ${index:04X} {census.events[index].label} is registered "
                f"as {registry[index]}: remove it (the allowlist only shrinks)"
            )
        elif census.events[index].null:
            out.append(f"allowlist entry ${index:04X} is a null routine and needs no entry")
        if issue not in ISSUES or not reason.strip():
            out.append(f"allowlist entry ${index:04X} lacks a tracked issue or a reason")
    if len(allowlist) > ceiling:
        out.append(f"the allowlist holds {len(allowlist)} entries, over its ceiling of {ceiling}")
    return out


# ---- generated artifacts ----------------------------------------------------
def _where(ev: Event) -> tuple[str, str]:
    """`(reached by, map or tree)` for the document, deduplicated."""
    by: list[str] = []
    places: list[str] = []
    for s in ev.sources:
        if s.kind == "trigger":
            text = "map trigger " + s.where.split(" ", 1)[1]
        elif s.kind == "interaction":
            text = "interaction area " + s.where.split("table slot ")[1].rstrip(")")
        elif s.kind == "dialogue":
            text = "dialogue `$F6`"
        elif s.kind == "chain":
            text = "called from " + s.where.split(" ")[3]
        elif s.kind == "direct":
            text = "direct call: " + s.where
        else:
            text = s.where
        if text not in by:
            by.append(text)
        if s.kind == "chain":
            continue
        for part in s.detail.split(", "):
            if part and part not in places:
                places.append(part)
    shown = ", ".join(places[:3]) + (f" (+{len(places) - 3} more)" if len(places) > 3 else "")
    return "; ".join(by), shown


def doc_rows(census: Census, registry: dict[int, str], allowlist: dict[int, tuple[int, str]] = ALLOWLIST) -> str:
    lines = ["| Event | Routine | Reached by | Map or tree | Status |", "|---|---|---|---|---|"]
    for index in sorted(census.events):
        ev = census.events[index]
        kind, extra = disposition(census, registry, allowlist, index)
        if kind == "scene":
            status = f"scene `{registry[index]}`"
        elif kind == "null":
            status = "null routine (`rts`), no scene needed"
        elif kind == "chained":
            status = f"inside the caller's scene (`${extra:04X}`)"
        elif kind == "allow":
            status = f"allowlisted, #{extra}"
        else:
            status = "MISSING"
        by, places = _where(ev)
        lines.append(f"| `${index:04X}` | `{ev.label or '?'}` | {by} | {places} | {status} |")
    return "\n".join(lines)


ISSUE_AREAS = {
    ISSUE_TRIGGERS: "trigger-fired events",
    ISSUE_DIALOGUE2: "`Event_GetAndRunDialogue2` callers",
    ISSUE_MOTAVIA: "Motavia side content",
    ISSUE_VAHAL: "Vahal Fort and Weapon Plant",
    ISSUE_DEZOLIS: "Dezolis late arc",
}


def issue_counts(allowlist: dict[int, tuple[int, str]] = ALLOWLIST) -> str:
    lines = ["| Issue | Area | Allowlisted events |", "|---:|---|---:|"]
    for issue in ISSUES:
        count = sum(1 for i, _ in allowlist.values() if i == issue)
        lines.append(f"| #{issue} | {ISSUE_AREAS[issue]} | {count} |")
    lines.append(f"| | total | {len(allowlist)} |")
    return "\n".join(lines)


def rust_rows(census: Census, registry: dict[int, str], allowlist: dict[int, tuple[int, str]] = ALLOWLIST) -> str:
    out = []
    for index in sorted(census.events):
        kind, extra = disposition(census, registry, allowlist, index)
        if kind == "scene":
            disp = "Disposition::Scene"
        elif kind == "null":
            disp = "Disposition::Null"
        elif kind == "chained":
            disp = f"Disposition::InsideScene(0x{extra:04X})"
        elif kind == "allow":
            disp = f"Disposition::Allowlisted({extra})"
        else:
            disp = "Disposition::Missing"
        out.append(f'    (0x{index:04X}, "{census.events[index].label or "?"}", {disp}),')
    return "\n".join(out)


RUST_TABLE = ROOT / "rust" / "psiv-core" / "tests" / "event_census.rs"
DOC = ROOT / "docs" / "scenes" / "EVENT_COVERAGE.md"
DOC_BEGIN = "<!-- census:begin -->"
DOC_END = "<!-- census:end -->"
COUNTS_BEGIN = "<!-- counts:begin -->"
COUNTS_END = "<!-- counts:end -->"
RUST_BEGIN = "    // census:begin"
RUST_END = "    // census:end"


def between(text: str, begin: str, end: str) -> str:
    start = text.index(begin) + len(begin)
    return text[start : text.index(end)].strip("\n")


def replace_between(text: str, begin: str, end: str, body: str) -> str:
    start = text.index(begin) + len(begin)
    return text[:start] + "\n" + body + "\n" + text[text.index(end) :]


def write_generated(census: Census, registry: dict[int, str]) -> None:
    """Regenerate the table in the Rust test and the tables in the document."""
    rust = RUST_TABLE.read_text(encoding="utf-8")
    RUST_TABLE.write_text(
        replace_between(rust, RUST_BEGIN, RUST_END, rust_rows(census, registry)), encoding="utf-8"
    )
    doc = DOC.read_text(encoding="utf-8")
    doc = replace_between(doc, DOC_BEGIN, DOC_END, doc_rows(census, registry))
    doc = replace_between(doc, COUNTS_BEGIN, COUNTS_END, issue_counts())
    DOC.write_text(doc, encoding="utf-8")


def main(argv: list[str]) -> int:
    census = build(ROM.read_bytes())
    registry = registry_events()
    if "--write" in argv:
        write_generated(census, registry)
    elif "--doc" in argv:
        print(doc_rows(census, registry))
        print()
        print(issue_counts())
    elif "--rust" in argv:
        print(rust_rows(census, registry))
    else:
        for line in problems(census, registry):
            print(line)
        print(f"{len(census.events)} events, {len(ALLOWLIST)} allowlisted")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
