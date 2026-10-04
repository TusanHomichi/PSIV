"""Structural census: every `Event_GetAndRunDialogue2` caller is a retained window.

    PYTHONPATH=. python3 -m unittest tests.test_dialogue2_callers -v

`Event_ZioNurvus` ends its presentation with `moveq #$B, d0 / jsr
Event_GetAndRunDialogue2` (`ps4.asm:148622`). The port transcribed that as a
`RunDialogueResume`, which reopens `Saved_Dialogue_Addr`: in play that is
whatever dialogue ran last (the Zio Fort barrier's `$45`, so the resume opened
`$46` and its text fired event 0), and after a load there is nothing to resume
(runner log H22). Every other caller sat on the standard window variant, which
destroys the panels the cartridge leaves up.

This test pins the class, not the instance. It re-derives from the US image
every `jsr $5ACDC.l` (`Event_GetAndRunDialogue2`) with the dialogue entry its
`d0` carries, ties each address to the scene whose documented range holds it,
and requires that scene to run exactly that entry through the `retained(..)`
op (`DialogueWindow::Retained`). A caller whose scene is not transcribed has to
be named in `UNTRANSCRIBED`, so transcribing it later forces the entry to move
into the registry instead of silently dropping out of the census.

Three tables must agree: the image, the registry (`rust/psiv-core/src/scenes`)
and the audit document (`docs/scenes/DIALOGUE2_CALLERS.md`); the Rust test
`every_dialogue2_caller_is_a_retained_window` keeps its own copy, which this
census also compares.

The negative controls at the end show each leg reports its own defect, so a
green run means agreement rather than a vacuous pass.
"""
from __future__ import annotations

import re
import unittest
from pathlib import Path

from tests.test_scene_trees import (
    NAME_RE,
    OPS_FN_RE,
    OPS_STATIC_RE,
    OPS_SYMBOL_RE,
    ROM,
    SCENES_DIR,
    STATIC_RE,
    documented_ranges,
)

ROOT = Path(__file__).resolve().parents[1]
AUDIT_DOC = ROOT / "docs" / "scenes" / "DIALOGUE2_CALLERS.md"
SCENES_MOD = SCENES_DIR / "mod.rs"

# `jsr $0005ACDC.l`: `Event_GetAndRunDialogue2` (`ps4.asm:121634`).
DIALOGUE2 = 0x5ACDC
JSR_DIALOGUE2 = bytes.fromhex("4eb9") + DIALOGUE2.to_bytes(4, "big")

# Callers whose scene is not in the registry: `jsr address` -> (label, event).
# Event ids are the `EventPtrs` slots (`ps4.asm:120650..120720`).
UNTRANSCRIBED = {
    0x071538: ("Event_AngerTowerAlys", 0x62, 0x08),
    0x072B2E: ("Event_FractOozeFound", 0x7D, 0x2F),
    0x072FEA: ("Event_KingRappy", 0x88, 0x36),
    0x0731B4: ("Event_DaughterTerminal", 0x8F, 0x04),
}

RETAINED_RE = re.compile(r"\bretained\(\s*(0x[0-9A-Fa-f]+|\d+)\s*\)")
RESUME_RE = re.compile(r"SceneOp::RunDialogueResume\b")
RUST_ROW_RE = re.compile(
    r'\(\s*"([A-Za-z_0-9]+)"\s*,\s*(0x[0-9A-Fa-f]+)\s*,\s*(0x[0-9A-Fa-f]+)\s*\)'
)


def rom_callers(rom: bytes) -> dict[int, int]:
    """`jsr` address -> the entry in `d0`, read from the loader before it.

    Accepts `moveq #n, d0` (`70nn`), `move.b #n, d0` (`103C 00nn`) and
    `move.w #n, d0` (`303C nnnn`) within twelve bytes before the jsr; the
    unaligned bytes in between are another instruction's operands, so the walk
    is by word.
    """
    callers: dict[int, int] = {}
    for match in re.finditer(re.escape(JSR_DIALOGUE2), rom):
        at = match.start()
        if at % 2:
            continue
        entry = None
        for back in range(2, 14, 2):
            word = int.from_bytes(rom[at - back : at - back + 2], "big")
            if 0x7000 <= word <= 0x707F:
                entry = word & 0xFF
                break
            prior = int.from_bytes(rom[at - back - 2 : at - back], "big")
            if prior in (0x103C, 0x303C) and word <= 0xFF:
                entry = word
                break
        if entry is None:
            raise AssertionError(f"jsr ${at:06X}: no d0 loader within twelve bytes")
        callers[at] = entry
    return callers


def registry_retained(sources: dict[str, str] | None = None) -> dict[str, list[int]]:
    """Scene name -> the entries its `retained(..)` ops run; also `Resume` count."""
    sources = sources if sources is not None else {
        path.name: path.read_text(encoding="utf-8") for path in SCENES_DIR.glob("*.rs")
    }
    bodies: dict[str, str] = {}
    for text in sources.values():
        for name, body in OPS_STATIC_RE.findall(text):
            bodies[name] = body
        for name, body in OPS_FN_RE.findall(text):
            bodies[name] = body
    found: dict[str, list[int]] = {}
    for text in sources.values():
        for _, body in STATIC_RE.findall(text):
            name = NAME_RE.search(body)
            if name is None:
                continue
            symbol = OPS_SYMBOL_RE.search(body)
            ops = bodies.get(symbol.group(1), "") if symbol else body
            found[name.group(1)] = [int(v, 0) for v in RETAINED_RE.findall(ops)]
    return found


def rust_table(text: str) -> dict[str, tuple[int, int]]:
    """The `DIALOGUE2_CALLERS` rows of `scenes/mod.rs`: name -> (address, entry)."""
    block = re.search(
        r"const DIALOGUE2_CALLERS:[^=]*=\s*&\[(.*?)\n\s*\];", text, re.S
    )
    if block is None:
        return {}
    return {
        name: (int(addr, 16), int(entry, 16))
        for name, addr, entry in RUST_ROW_RE.findall(block.group(1))
    }


def census(
    rom: bytes,
    registry: dict[str, list[int]],
    ranges: dict[str, list[tuple[int, int]]],
    table: dict[str, tuple[int, int]],
    doc: str,
) -> list[str]:
    problems: list[str] = []
    callers = rom_callers(rom)
    transcribed: dict[str, tuple[int, int]] = {}
    for at, entry in sorted(callers.items()):
        names = sorted(
            {name for name, rs in ranges.items() for s, e in rs if s <= at < e}
        )
        if not names:
            if at not in UNTRANSCRIBED:
                problems.append(
                    f"${at:06X} (entry 0x{entry:X}): no documented scene holds this "
                    "caller and it is not in UNTRANSCRIBED"
                )
            else:
                label, _, want = UNTRANSCRIBED[at]
                if entry != want:
                    problems.append(
                        f"${at:06X} {label}: image entry 0x{entry:X}, census expects 0x{want:X}"
                    )
            if f"${at:06X}" not in doc:
                problems.append(f"${at:06X}: missing from DIALOGUE2_CALLERS.md")
            continue
        if len(names) > 1:
            problems.append(f"${at:06X}: documented ranges of {names} overlap")
            continue
        name = names[0]
        transcribed[name] = (at, entry)
        ran = registry.get(name)
        if ran is None:
            problems.append(f"{name}: documented range holds ${at:06X} but the scene is not registered")
        elif ran != [entry]:
            problems.append(
                f"{name}: image runs entry 0x{entry:X} through Event_GetAndRunDialogue2 at "
                f"${at:06X}, registry's retained ops run {[hex(v) for v in ran]}"
            )
        if f"${at:06X}" not in doc:
            problems.append(f"${at:06X} {name}: missing from DIALOGUE2_CALLERS.md")
    for at in UNTRANSCRIBED:
        if at not in callers:
            problems.append(f"UNTRANSCRIBED lists ${at:06X}, which the image does not call")
    for name, ran in registry.items():
        if ran and name not in transcribed:
            problems.append(f"{name}: registry runs retained entries {ran} the image does not")
    if table != transcribed:
        problems.append(
            "scenes/mod.rs DIALOGUE2_CALLERS disagrees with the image: "
            f"{sorted(set(table.items()) ^ set(transcribed.items()))}"
        )
    return problems


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class Dialogue2Census(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = ROM.read_bytes()
        cls.registry = registry_retained()
        cls.ranges = documented_ranges()
        cls.table = rust_table(SCENES_MOD.read_text(encoding="utf-8"))
        cls.doc = AUDIT_DOC.read_text(encoding="utf-8")

    def test_every_caller_in_the_image_is_a_retained_window(self) -> None:
        problems = census(self.rom, self.registry, self.ranges, self.table, self.doc)
        self.assertEqual(problems, [], "\n".join(problems))

    def test_the_image_has_fourteen_callers(self) -> None:
        self.assertEqual(len(rom_callers(self.rom)), 14)

    def test_zio_nurvus_is_the_scene_h22_was_found_in(self) -> None:
        """Guards the guard: the census must cover the H22 scene itself."""
        self.assertEqual(rom_callers(self.rom)[0x06F3D4], 0x0B)
        self.assertEqual(self.registry["Event_ZioNurvus"], [0x0B])
        zio = (SCENES_DIR / "post_rika_events.rs").read_text(encoding="utf-8")
        body = STATIC_RE.search(zio[zio.index("pub static ZIO_NURVUS") :]).group(2)
        self.assertEqual(RESUME_RE.findall(body), [], "Event_ZioNurvus must not resume")


class NegativeControl(unittest.TestCase):
    """Each leg of the census fails on its own defect."""

    def setUp(self) -> None:
        if not ROM.exists():
            self.skipTest(f"ROM fixture not present at {ROM}")
        self.rom = ROM.read_bytes()
        self.registry = registry_retained()
        self.ranges = documented_ranges()
        self.table = rust_table(SCENES_MOD.read_text(encoding="utf-8"))
        self.doc = AUDIT_DOC.read_text(encoding="utf-8")

    def run_census(self, **override: object) -> list[str]:
        args = dict(
            rom=self.rom,
            registry=self.registry,
            ranges=self.ranges,
            table=self.table,
            doc=self.doc,
        )
        args.update(override)
        return census(**args)  # type: ignore[arg-type]

    def test_a_scene_back_on_resume_is_reported(self) -> None:
        sources = {
            path.name: path.read_text(encoding="utf-8")
            for path in SCENES_DIR.glob("*.rs")
        }
        text = sources["post_rika_events.rs"]
        broken = text.replace("        retained(0x0B),\n", "        SceneOp::RunDialogueResume,\n", 1)
        self.assertNotEqual(broken, text)
        sources["post_rika_events.rs"] = broken
        problems = self.run_census(registry=registry_retained(sources))
        self.assertTrue(
            any(p.startswith("Event_ZioNurvus: image runs entry 0xB") for p in problems),
            problems,
        )

    def test_a_wrong_entry_is_reported(self) -> None:
        sources = {
            path.name: path.read_text(encoding="utf-8")
            for path in SCENES_DIR.glob("*.rs")
        }
        text = sources["post_zio_cutscenes.rs"]
        broken = text.replace("retained(0x48)", "retained(0x47)", 1)
        self.assertNotEqual(broken, text)
        sources["post_zio_cutscenes.rs"] = broken
        problems = self.run_census(registry=registry_retained(sources))
        self.assertTrue(any(p.startswith("Event_Juza:") for p in problems), problems)

    def test_a_new_image_caller_is_reported(self) -> None:
        patched = bytearray(self.rom)
        # A fifteenth `moveq #1, d0 / jsr $5ACDC.l` over padding the image never runs.
        at = 0x300000
        patched[at : at + 8] = bytes.fromhex("7001") + JSR_DIALOGUE2
        problems = self.run_census(rom=bytes(patched))
        self.assertTrue(
            any(p.startswith(f"${at + 2:06X}") and "UNTRANSCRIBED" in p for p in problems),
            problems,
        )

    def test_a_changed_image_entry_is_reported(self) -> None:
        patched = bytearray(self.rom)
        self.assertEqual(patched[0x06F3D2:0x06F3D4], bytes.fromhex("700b"))
        patched[0x06F3D3] = 0x0C
        problems = self.run_census(rom=bytes(patched))
        self.assertTrue(any(p.startswith("Event_ZioNurvus:") for p in problems), problems)

    def test_a_transcribed_caller_left_in_untranscribed_is_reported(self) -> None:
        table = dict(self.table)
        del table["Event_Juza"]
        problems = self.run_census(table=table)
        self.assertTrue(any("DIALOGUE2_CALLERS disagrees" in p for p in problems), problems)

    def test_a_missing_audit_row_is_reported(self) -> None:
        problems = self.run_census(doc=self.doc.replace("$06F3D4", "$------"))
        self.assertIn("$06F3D4 Event_ZioNurvus: missing from DIALOGUE2_CALLERS.md", problems)


if __name__ == "__main__":
    unittest.main()
