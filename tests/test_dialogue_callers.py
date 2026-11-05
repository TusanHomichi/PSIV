"""Structural census: every dialogue-entry caller runs its routine's window.

    PYTHONPATH=. python3 -m unittest tests.test_dialogue_callers -v

`Event_ZioNurvus` ends its presentation with `moveq #$B, d0 / jsr
Event_GetAndRunDialogue2` (`ps4.asm:148622-148623`). The port transcribed that
as a `RunDialogueResume`, which reopens `Saved_Dialogue_Addr`: in play that is
whatever dialogue ran last, and after a load there is nothing to resume. Every
other `Event_GetAndRunDialogue2` caller sat on the standard window, which
destroys the panels the cartridge leaves up; seven of the eight
`Event_GetAndRunDialogue3` callers sat on a window that destroys them too
(`docs/scenes/DIALOGUE_CALLERS.md`).

This test pins the class, not the instances. It re-derives from the US image
every `jsr`/`jmp` to the five `Event_GetAndRunDialogue*` entries, every entry
`d0` can carry into the call (the loader just before it and each
`moveq`/`bra` that lands on it), and every inline `jsr GetDialogueByID` a
transcribed scene's bytes hold. It ties each caller to the scene whose
documented range holds it and requires the Rust table `DIALOGUE_CALLERS`
(`rust/psiv-core/src/scenes/dialogue_callers.rs`) to hold exactly those rows,
and the audit document to name every caller. The Rust test
`every_dialogue_entry_caller_runs_its_window` then holds the typed registry to
that table: each row's entry run through its routine's `DialogueWindow`, and no
scene running a window none of its callers asks for.

A caller whose scene is not transcribed is named in `UNTRANSCRIBED`, so a lane
that transcribes it later has to move it into the table instead of dropping it
out of the census.

The routine-to-window mapping is checked here too, against the exits read from
the image (`EXITS`), and the Rust `ROUTINE_WINDOWS` must agree with it.

The negative controls at the end show each leg reports its own defect.
"""
from __future__ import annotations

import re
import unittest
from pathlib import Path

from tests.test_scene_trees import ROM, SCENES_DIR, documented_ranges

ROOT = Path(__file__).resolve().parents[1]
AUDIT_DOC = ROOT / "docs" / "scenes" / "DIALOGUE_CALLERS.md"
TABLE_RS = SCENES_DIR / "dialogue_callers.rs"
TABLE_REL = "rust/psiv-core/src/scenes/dialogue_callers.rs"

# The five entries, `jsr GetDialogueByID` at each (`ps4.asm:121603..121718`),
# by the name the Rust table gives them, and the port's window for each.
ROUTINES: dict[str, tuple[int, str, str]] = {
    "D1": (0x5AC66, "Event_GetAndRunDialogue", "Standard"),
    "D2": (0x5ACDC, "Event_GetAndRunDialogue2", "Retained"),
    "D3": (0x5AD32, "Event_GetAndRunDialogue3", "Cutscene3"),
    "D4": (0x5ADA2, "Event_GetAndRunDialogue4", "Ending"),
    "D5": (0x5ADF8, "Event_GetAndRunDialogue5", "Cutscene"),
}
# `GetDialogueByID` (`$59164`) called by a scene's own window code.
INLINE = 0x59164
# The routines themselves, through `Panel_DestroyAll` (`$5AE74`): their own
# `jsr GetDialogueByID` are not callers.
ROUTINE_SPAN = (0x5AC66, 0x5AE74)

# What tells the exits apart, read from the image (`$5AC66..$5AE73`): a `jsr`
# or `jmp` each routine's body must (`True`) or must not (`False`) contain.
JSR = "4eb9"
JMP = "4ef9"
PANEL_DESTROY_ALL = JSR + "0005ae74"
WINDOW_DESTROY = JSR + "000687cc"
PAL_FADE_OUT = JSR + "0004223e"
RUN_TEXT = JSR + "0006a090"
RUN_TEXT3 = JSR + "0006ad00"
MAP_LOAD_CHUNKS = JMP + "00051adc"
# `moveq #0, d0` / `move.b d0, Panel_Num`, `Windows_Opened_Num`, `$ECA4` / `rts`.
FORGET_WINDOWS = "700011c0ed0211c0ec9a11c0eca44e75"
EXITS: dict[str, dict[str, bool]] = {
    "D1": {PANEL_DESTROY_ALL: True, WINDOW_DESTROY: True, PAL_FADE_OUT: False,
           RUN_TEXT: True, MAP_LOAD_CHUNKS: True, FORGET_WINDOWS: False},
    "D2": {PANEL_DESTROY_ALL: False, WINDOW_DESTROY: False, PAL_FADE_OUT: False,
           RUN_TEXT: True, MAP_LOAD_CHUNKS: False, FORGET_WINDOWS: True},
    "D3": {PANEL_DESTROY_ALL: False, WINDOW_DESTROY: True, PAL_FADE_OUT: False,
           RUN_TEXT: True, MAP_LOAD_CHUNKS: True, FORGET_WINDOWS: False},
    "D4": {PANEL_DESTROY_ALL: False, WINDOW_DESTROY: True, PAL_FADE_OUT: False,
           RUN_TEXT3: True, RUN_TEXT: False, MAP_LOAD_CHUNKS: True},
    "D5": {PANEL_DESTROY_ALL: True, WINDOW_DESTROY: True, PAL_FADE_OUT: True,
           RUN_TEXT: True, MAP_LOAD_CHUNKS: True, FORGET_WINDOWS: False},
}

DYN = "DYN"

# Instructions seen between a `d0` loader and its call that leave `d0` alone:
# `move.b #1, Render_Sprites_In_Cutscenes` and `clr.b` of it.
D0_SAFE_GAPS = ("", "11fc0001ecfd", "4238ecfd")

# Callers no documented scene holds: (routine, `jsr` address) -> (event label,
# `EventPtrs` index, the entries the image gives `d0`, the owning issue). The
# spans are in `docs/scenes/DIALOGUE_CALLERS.md`.
UNTRANSCRIBED: dict[tuple[str, int], tuple[str, int, frozenset, int]] = {
    ("D1", 0x06D62E): ("Event_FaintingPriest", 0x24, frozenset({0x45}), 283),
    ("D1", 0x06EE58): ("Event_ZioFanatic", 0x31, frozenset({0x49}), 283),
    ("D1", 0x06EEB8): ("Event_ZioFanatic", 0x31, frozenset({0x1F}), 283),
    ("D1", 0x071F34): ("Event_PickingFifthCharacter", 0x68, frozenset({DYN}), 284),
    ("D1", 0x071F44): ("Event_PickingFifthCharacter", 0x68, frozenset({DYN}), 284),
    ("D1", 0x071F86): ("Event_PickingFifthCharacter", 0x68, frozenset({DYN}), 284),
    ("D1", 0x072DDE): ("Event_DyingBoy", 0x86, frozenset({0x33}), 283),
    ("D1", 0x072FA6): ("Event_DyingBoy", 0x86, frozenset({0x3B}), 283),
    ("D1", 0x072FAE): ("Event_DyingBoy", 0x86, frozenset({0x3E}), 283),
}

ROW_RE = re.compile(
    r'\(\s*"([A-Za-z_0-9]+)"\s*,\s*(D[1-5]|INLINE)\s*,\s*(0x[0-9A-Fa-f_]+)\s*,\s*'
    r"(0x[0-9A-Fa-f]+|DYN)\s*,?\s*\)"
)
WINDOW_ROW_RE = re.compile(r"\(\s*(D[1-5])\s*,\s*DialogueWindow::([A-Za-z0-9]+)\s*\)")
CONST_RE = re.compile(r"const (D[1-5]|INLINE): u32 = (0x[0-9A-Fa-f_]+);")

Row = tuple[str, str, int, object]


def _word(rom: bytes, at: int) -> int:
    return int.from_bytes(rom[at : at + 2], "big")


def _literal(rom: bytes, at: int) -> tuple[int, int] | None:
    """`(entry, length)` of a `d0` literal load at `at`, if there is one."""
    word = _word(rom, at)
    if 0x7000 <= word <= 0x707F:
        return word & 0xFF, 2
    if word in (0x103C, 0x303C) and _word(rom, at + 2) <= 0xFF:
        return _word(rom, at + 2), 4
    return None


def _branch_target(rom: bytes, at: int) -> int | None:
    """Where a `bra.s`/`bra.w` at `at` goes."""
    word = _word(rom, at)
    if word == 0x6000:
        return at + 2 + int.from_bytes(rom[at + 2 : at + 4], "big", signed=True)
    if word & 0xFF00 == 0x6000 and word & 0xFF != 0xFF:
        disp = word & 0xFF
        return at + 2 + (disp - 0x100 if disp & 0x80 else disp)
    return None


def d0_entries(rom: bytes, at: int) -> frozenset:
    """Every literal entry `d0` carries into the call at `at`, or `{DYN}`.

    The fall-through loader is a `moveq`/`move.b`/`move.w #n, d0` ending
    right before the call, or before one of `D0_SAFE_GAPS`. Each branch
    loader is such a load followed at once by a `bra` that lands on the call
    (`Event_RajaSick`'s three `moveq`/`bra.s loc_70C72`). A call with no
    literal path is `DYN`: `d0` comes from RAM or a table.
    """
    found: set = set()
    for gap in D0_SAFE_GAPS:
        end = at - len(gap) // 2
        if rom[end:at].hex() != gap:
            continue
        for length in (2, 4):
            lit = _literal(rom, end - length)
            if lit is not None and lit[1] == length:
                found.add(lit[0])
                break
        if found:
            break
    for start in range(at - 0x100, at, 2):
        lit = _literal(rom, start)
        if lit is not None and _branch_target(rom, start + lit[1]) == at:
            found.add(lit[0])
    return frozenset(found) if found else frozenset({DYN})


def rom_callers(rom: bytes) -> dict[tuple[str, int], frozenset]:
    """`(routine, call address)` -> entries, for every even `jsr`/`jmp`.

    `INLINE` callers are every `jsr GetDialogueByID` outside the routines;
    the census keeps only those inside a scene's documented bytes.
    """
    targets = {name: addr for name, (addr, _, _) in ROUTINES.items()}
    targets["INLINE"] = INLINE
    found: dict[tuple[str, int], frozenset] = {}
    for name, addr in targets.items():
        for opcode in (JSR, JMP) if name != "INLINE" else (JSR,):
            needle = bytes.fromhex(opcode) + addr.to_bytes(4, "big")
            for match in re.finditer(re.escape(needle), rom):
                at = match.start()
                if at % 2 or ROUTINE_SPAN[0] <= at < ROUTINE_SPAN[1]:
                    continue
                found[(name, at)] = d0_entries(rom, at)
    return found


def routine_body(rom: bytes, name: str) -> str:
    """Hex of one routine, from its entry to the next routine (or `$5AE74`)."""
    starts = sorted(addr for addr, _, _ in ROUTINES.values()) + [ROUTINE_SPAN[1]]
    start = ROUTINES[name][0]
    return rom[start : starts[starts.index(start) + 1]].hex()


def exit_problems(rom: bytes) -> list[str]:
    problems = []
    for name, signature in EXITS.items():
        body = routine_body(rom, name)
        label = ROUTINES[name][1]
        if not body.startswith(JSR + "00059164"):
            problems.append(f"{label}: does not open with jsr GetDialogueByID")
        for needle, wanted in signature.items():
            if (needle in body) != wanted:
                problems.append(
                    f"{label}: {'lacks' if wanted else 'has'} {needle}, so its exit is not "
                    f"what DialogueWindow::{ROUTINES[name][2]} stands for"
                )
    return problems


def rust_table(text: str) -> set[Row]:
    """The `DIALOGUE_CALLERS` rows: (scene, routine, address, entry or DYN)."""
    block = re.search(r"const DIALOGUE_CALLERS:[^=]*=\s*&\[(.*?)\n\];", text, re.S)
    if block is None:
        return set()
    return {
        (scene, routine, int(addr, 16), entry if entry == DYN else int(entry, 16))
        for scene, routine, addr, entry in ROW_RE.findall(block.group(1))
    }


def rust_routines(text: str) -> tuple[dict[str, int], dict[str, str]]:
    """The Rust consts (`D1` -> `$5AC66`) and `ROUTINE_WINDOWS` (`D1` -> `Standard`)."""
    consts = {name: int(value.replace("_", ""), 16) for name, value in CONST_RE.findall(text)}
    block = re.search(r"const ROUTINE_WINDOWS:[^=]*=\s*&\[(.*?)\];", text, re.S)
    windows = dict(WINDOW_ROW_RE.findall(block.group(1))) if block else {}
    return consts, windows


def _row_text(row: Row) -> str:
    scene, routine, at, entry = row
    shown = entry if entry == DYN else f"0x{entry:02X}"
    return f'("{scene}", {routine}, 0x{at:06X}, {shown}),'


def census(
    rom: bytes,
    ranges: dict[str, list[tuple[int, int]]],
    table: set[Row],
    doc: str,
    untranscribed: dict | None = None,
) -> list[str]:
    untranscribed = UNTRANSCRIBED if untranscribed is None else untranscribed
    problems: list[str] = []
    expected: set[Row] = set()
    callers = rom_callers(rom)
    for (routine, at), entries in sorted(callers.items(), key=lambda kv: kv[0][1]):
        names = sorted({name for name, rs in ranges.items() for s, e in rs if s <= at < e})
        label = "GetDialogueByID (inline)" if routine == "INLINE" else ROUTINES[routine][1]
        if len(names) > 1:
            problems.append(f"${at:06X} {label}: documented ranges of {names} overlap")
            continue
        if not names:
            if routine == "INLINE":
                continue
            held = untranscribed.get((routine, at))
            if held is None:
                problems.append(
                    f"${at:06X} {label} (entries {sorted(map(str, entries))}): no documented "
                    "scene holds this caller and it is not in UNTRANSCRIBED; transcribe its "
                    "scene or add it to UNTRANSCRIBED and the uncovered-callers table of "
                    "docs/scenes/DIALOGUE_CALLERS.md"
                )
            elif held[2] != entries:
                problems.append(
                    f"${at:06X} {held[0]}: image entries {sorted(map(str, entries))}, "
                    f"UNTRANSCRIBED expects {sorted(map(str, held[2]))}"
                )
            if f"${at:06X}" not in doc:
                problems.append(f"${at:06X} {label}: missing from DIALOGUE_CALLERS.md")
            continue
        scene = names[0]
        if (routine, at) in untranscribed:
            problems.append(
                f"${at:06X} {label}: {scene}'s documented range now holds this caller; "
                "remove it from UNTRANSCRIBED and add its rows to DIALOGUE_CALLERS"
            )
        for entry in entries:
            expected.add((scene, routine, at, entry))
        if f"${at:06X}" not in doc:
            problems.append(
                f"${at:06X} {scene} {label}: missing from DIALOGUE_CALLERS.md; add it to "
                f"{scene}'s row of the census table"
            )
    for key in untranscribed:
        if key not in callers:
            problems.append(f"UNTRANSCRIBED lists ${key[1]:06X}, which the image does not call")
    for row in sorted(expected - table, key=lambda r: (r[2], str(r[3]))):
        problems.append(
            f"${row[2]:06X} {row[0]}: the image calls {row[1]} here; add "
            f"`{_row_text(row)}` to DIALOGUE_CALLERS in {TABLE_REL}"
        )
    for row in sorted(table - expected, key=lambda r: (r[2], str(r[3]))):
        problems.append(
            f"${row[2]:06X} {row[0]}: DIALOGUE_CALLERS has `{_row_text(row)}`, "
            "which the image and the documented ranges do not give; remove or correct it"
        )
    return problems


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class DialogueCallerCensus(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = ROM.read_bytes()
        cls.ranges = documented_ranges()
        cls.rust = TABLE_RS.read_text(encoding="utf-8")
        cls.table = rust_table(cls.rust)
        cls.doc = AUDIT_DOC.read_text(encoding="utf-8")

    def test_every_caller_in_the_image_is_in_the_table(self) -> None:
        problems = census(self.rom, self.ranges, self.table, self.doc)
        self.assertEqual(problems, [], "\n".join(problems))

    def test_each_routine_exit_is_its_window(self) -> None:
        problems = exit_problems(self.rom)
        self.assertEqual(problems, [], "\n".join(problems))

    def test_the_rust_routines_are_these(self) -> None:
        consts, windows = rust_routines(self.rust)
        want = {name: addr for name, (addr, _, _) in ROUTINES.items()}
        want["INLINE"] = INLINE
        self.assertEqual(consts, want)
        self.assertEqual(windows, {name: w for name, (_, _, w) in ROUTINES.items()})

    def test_the_image_caller_counts(self) -> None:
        """Counts from a plain byte search, independent of `rom_callers`."""
        counts = {}
        for name, (addr, _, _) in ROUTINES.items():
            n = 0
            for opcode in (JSR, JMP):
                needle = bytes.fromhex(opcode) + addr.to_bytes(4, "big")
                n += sum(1 for m in re.finditer(re.escape(needle), self.rom) if m.start() % 2 == 0)
            counts[name] = n
        self.assertEqual(counts, {"D1": 155, "D2": 14, "D3": 8, "D4": 3, "D5": 23})
        by_routine: dict[str, int] = {}
        for routine, _ in rom_callers(self.rom):
            by_routine[routine] = by_routine.get(routine, 0) + 1
        self.assertEqual({k: v for k, v in by_routine.items() if k != "INLINE"}, counts)

    def test_zio_nurvus_is_the_scene_h22_was_found_in(self) -> None:
        """Guards the guard: the census must cover the H22 scene itself."""
        self.assertEqual(rom_callers(self.rom)[("D2", 0x06F3D4)], frozenset({0x0B}))
        self.assertIn(("Event_ZioNurvus", "D2", 0x06F3D4, 0x0B), self.table)

    def test_branch_loaded_entries_are_read(self) -> None:
        """`Event_RajaSick` (`ps4.asm:150224-150250`): `$5D`, `$5E`, `$5F` reach
        the `jmp` at `$070C72`, three of them by `bra.s`."""
        self.assertEqual(
            rom_callers(self.rom)[("D1", 0x070C72)], frozenset({0x5D, 0x5E, 0x5F})
        )


class NegativeControl(unittest.TestCase):
    """Each leg of the census fails on its own defect."""

    def setUp(self) -> None:
        if not ROM.exists():
            self.skipTest(f"ROM fixture not present at {ROM}")
        self.rom = ROM.read_bytes()
        self.ranges = documented_ranges()
        self.rust = TABLE_RS.read_text(encoding="utf-8")
        self.table = rust_table(self.rust)
        self.doc = AUDIT_DOC.read_text(encoding="utf-8")

    def run_census(self, **override: object) -> list[str]:
        args = dict(rom=self.rom, ranges=self.ranges, table=self.table, doc=self.doc)
        args.update(override)
        return census(**args)  # type: ignore[arg-type]

    def test_the_census_starts_clean(self) -> None:
        self.assertEqual(self.run_census(), [])

    def test_a_new_image_caller_is_reported(self) -> None:
        patched = bytearray(self.rom)
        # `moveq #1, d0 / jsr Event_GetAndRunDialogue3` over the `$FF` fill at
        # the image's end, which nothing runs.
        at = 0x2FFFC0
        self.assertEqual(set(patched[at : at + 8]), {0xFF})
        patched[at : at + 8] = bytes.fromhex("7001" + JSR + "0005ad32")
        self.assertEqual(len(patched), len(self.rom))
        problems = self.run_census(rom=bytes(patched))
        self.assertTrue(
            any(p.startswith(f"${at + 2:06X} Event_GetAndRunDialogue3") and "UNTRANSCRIBED" in p
                for p in problems),
            problems,
        )

    def test_a_new_caller_in_a_scene_says_which_row_to_add(self) -> None:
        # MeetingKyra's `jsr Event_GetAndRunDialogue3` at `$0777D8` retargeted
        # to `Event_GetAndRunDialogue5`: the row it needs is a D5 row.
        patched = bytearray(self.rom)
        self.assertEqual(patched[0x0777D8:0x0777DE].hex(), JSR + "0005ad32")
        patched[0x0777DA:0x0777DE] = bytes.fromhex("0005adf8")
        problems = self.run_census(rom=bytes(patched))
        self.assertIn(
            "$0777D8 Cutscene_MeetingKyra: the image calls D5 here; add "
            '`("Cutscene_MeetingKyra", D5, 0x0777D8, 0x28),` to DIALOGUE_CALLERS in '
            f"{TABLE_REL}",
            problems,
        )

    def test_a_changed_image_entry_is_reported(self) -> None:
        patched = bytearray(self.rom)
        self.assertEqual(patched[0x06F3D2:0x06F3D4], bytes.fromhex("700b"))
        patched[0x06F3D3] = 0x0C
        problems = self.run_census(rom=bytes(patched))
        self.assertTrue(any("Event_ZioNurvus" in p and "0x0C" in p for p in problems), problems)
        self.assertTrue(any("Event_ZioNurvus" in p and "remove" in p for p in problems), problems)

    def test_a_lost_branch_entry_is_reported(self) -> None:
        # `moveq #$5D, d0 / bra.s loc_70C72` at `$070C68`: point the `bra.s`
        # one word short and `$5D` no longer reaches the call.
        patched = bytearray(self.rom)
        self.assertEqual(patched[0x070C68:0x070C6C].hex(), "705d6006")
        patched[0x070C6B] = 0x04
        problems = self.run_census(rom=bytes(patched))
        self.assertTrue(
            any(p.startswith("$070C72 Event_RajaSick: DIALOGUE_CALLERS has") and "0x5D" in p
                for p in problems),
            problems,
        )

    def test_a_dropped_table_row_is_reported(self) -> None:
        table = {row for row in self.table if row[0] != "Event_Juza"}
        self.assertLess(len(table), len(self.table))
        problems = self.run_census(table=table)
        self.assertTrue(
            any(p.startswith("$06FB42 Event_Juza: the image calls D2") for p in problems),
            problems,
        )

    def test_a_row_under_the_wrong_routine_is_reported(self) -> None:
        row = ("Cutscene_DemiRescue", "D3", 0x074A92, 0x42)
        self.assertIn(row, self.table)
        table = (self.table - {row}) | {("Cutscene_DemiRescue", "D5", 0x074A92, 0x42)}
        problems = self.run_census(table=table)
        self.assertEqual(len([p for p in problems if p.startswith("$074A92")]), 2, problems)

    def test_a_caller_claimed_by_two_scenes_is_reported(self) -> None:
        table = self.table | {("Event_DarkForce2", "D2", 0x06F3D4, 0x0B)}
        problems = self.run_census(table=table)
        self.assertTrue(any(p.startswith("$06F3D4 Event_DarkForce2: DIALOGUE_CALLERS has")
                            for p in problems), problems)

    def test_overlapping_ranges_are_reported(self) -> None:
        ranges = dict(self.ranges)
        ranges["Event_Burstroc"] = ranges["Event_Burstroc"] + ranges["Cutscene_MeetingKyra"]
        problems = self.run_census(ranges=ranges)
        self.assertTrue(any("overlap" in p and "$0777A6" in p for p in problems), problems)

    def test_a_missing_audit_row_is_reported(self) -> None:
        problems = self.run_census(doc=self.doc.replace("$06F3D4", "$------"))
        self.assertTrue(any(p.startswith("$06F3D4 Event_ZioNurvus Event_GetAndRunDialogue2: "
                                         "missing") for p in problems), problems)
        problems = self.run_census(doc=self.doc.replace("$072DDE", "$------"))
        self.assertIn("$072DDE Event_GetAndRunDialogue: missing from DIALOGUE_CALLERS.md", problems)

    def test_a_transcribed_caller_left_in_untranscribed_is_reported(self) -> None:
        untranscribed = dict(UNTRANSCRIBED)
        untranscribed[("D2", 0x06FB42)] = ("Event_Juza", 0x40, frozenset({0x48}), 0)
        problems = self.run_census(untranscribed=untranscribed)
        self.assertTrue(any("remove it from UNTRANSCRIBED" in p for p in problems), problems)

    def test_a_stale_untranscribed_entry_is_reported(self) -> None:
        untranscribed = dict(UNTRANSCRIBED)
        untranscribed[("D1", 0x072DDE)] = ("Event_DyingBoy", 0x86, frozenset({0x32}), 283)
        problems = self.run_census(untranscribed=untranscribed)
        self.assertTrue(any("UNTRANSCRIBED expects" in p for p in problems), problems)

    def test_a_changed_exit_is_reported(self) -> None:
        # Give `Event_GetAndRunDialogue3` the `Panel_DestroyAll` its siblings
        # have, over its `bclr #0, $ECA4` (`$5AD78`): then it is no longer the
        # window that keeps the panels.
        patched = bytearray(self.rom)
        self.assertEqual(patched[0x5AD78:0x5AD7E].hex(), "08b80000eca4")
        patched[0x5AD78:0x5AD7E] = bytes.fromhex(PANEL_DESTROY_ALL)
        problems = exit_problems(bytes(patched))
        self.assertTrue(any(p.startswith("Event_GetAndRunDialogue3: has") for p in problems),
                        problems)

    def test_a_rust_window_mapping_change_is_reported(self) -> None:
        text = self.rust.replace("(D3, DialogueWindow::Cutscene3)", "(D3, DialogueWindow::Cutscene)")
        self.assertNotEqual(text, self.rust)
        _, windows = rust_routines(text)
        self.assertNotEqual(windows, {n: w for n, (_, _, w) in ROUTINES.items()})


if __name__ == "__main__":
    unittest.main()
