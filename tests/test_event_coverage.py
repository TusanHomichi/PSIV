"""Census guard: every event the cartridge can fire has a scene or a listed reason.

    PYTHONPATH=. python3 -m unittest tests.test_event_coverage -v

The campaign runner kept halting on events the port never transcribed:
`$71` sandworm, the Garuberk doors, the penguin feed (#56), the Canceller
reminder, four `Event_GetAndRunDialogue2` callers (#71), then H28's
`$43 Event_OutsideRajaTemple`, which fires on every arrival outside Raja
Temple. Each was found by a halt. This test finds them without one.

`tests/event_census.py` derives the set from the US image (its docstring
cites how each source is read): every operand `$ECA8` below `$80000` is
classified, the `RunEventsJmpTbl` routines are read against the maps that
list them, the interaction table and every `$F6` in every dialogue tree are
joined, and every direct call onto an event routine is found. An event is
covered when the registry holds its scene, when its routine is a bare `rts`,
or when it is only ever called from inside a registered scene. Anything else
must be in the allowlist with a tracking issue, and the allowlist may only
shrink.

`rust/psiv-core/tests/event_census.rs` carries the same table against the
live registry (`scene_for`); this test compares that table and
`docs/scenes/EVENT_COVERAGE.md` to the census, so all three stay one list.
Regenerate the generated sections with
`python3 -m tests.event_census --rust` and `--doc`.

The negative controls at the end show each leg fails on its own defect, the
first being the one the brief asks for: unregister a scene and the guard
fails, naming it.
"""
from __future__ import annotations

import re
import unittest

from tests import event_census as ec

RUST_TABLE, DOC, between = ec.RUST_TABLE, ec.DOC, ec.between
DOC_BEGIN, DOC_END = ec.DOC_BEGIN, ec.DOC_END
COUNTS_BEGIN, COUNTS_END = ec.COUNTS_BEGIN, ec.COUNTS_END
RUST_BEGIN, RUST_END = ec.RUST_BEGIN, ec.RUST_END


@unittest.skipUnless(ec.ROM.exists(), f"ROM fixture not present at {ec.ROM}")
class EventCensus(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = ec.ROM.read_bytes()
        cls.census = ec.build(cls.rom)
        cls.registry = ec.registry_events()

    def test_every_event_index_operand_is_classified(self) -> None:
        """`build` raises on any unknown `$ECA8` shape; pin what it found."""
        shapes: dict[str, int] = {}
        for _, shape, _ in self.census.sites:
            shapes[shape] = shapes.get(shape, 0) + 1
        self.assertEqual(shapes["reg"], 2, "Interaction_GetEvent and GetEventFromDialogue")
        self.assertGreaterEqual(shapes["imm"], 120)
        self.assertGreaterEqual(shapes["read"], 10)

    def test_every_reachable_event_has_a_scene_or_a_listed_reason(self) -> None:
        found = ec.problems(self.census, self.registry)
        self.assertEqual(found, [], "\n".join(found))

    def test_the_allowlist_names_a_tracked_issue_and_never_grows(self) -> None:
        self.assertLessEqual(len(ec.ALLOWLIST), ec.ALLOWLIST_CEILING)
        for event, (issue, reason) in ec.ALLOWLIST.items():
            self.assertIn(issue, ec.ISSUES, f"${event:04X}")
            self.assertTrue(reason)
        by_issue = {i: [e for e, (n, _) in ec.ALLOWLIST.items() if n == i] for i in ec.ISSUES}
        # #56 and #71 keep exactly what is still untranscribed of their lists.
        self.assertEqual(sorted(by_issue[56]), [0x96])
        self.assertEqual(sorted(by_issue[71]), [0x62])

    def test_the_census_sees_every_source_kind(self) -> None:
        kinds = {k for ev in self.census.events.values() for k in ev.kinds()}
        self.assertEqual(kinds, {"trigger", "interaction", "dialogue", "other", "direct", "chain"})

    def test_the_events_the_issues_name_are_in_the_census(self) -> None:
        for event in (0x71, 0x37, 0x38, 0x96, 0x2A, 0x62, 0x7D, 0x88, 0x8F, 0x43):
            self.assertIn(event, self.census.events, f"${event:04X}")
        trigger = {s.kind for s in self.census.events[0x43].sources}
        self.assertEqual(trigger, {"trigger"})
        self.assertIn("Dezolis", self.census.events[0x43].sources[0].detail)
        for event in (0x62, 0x7D, 0x88, 0x8F):
            self.assertEqual(self.census.events[event].kinds(), ["dialogue"])

    def test_null_routines_are_read_from_the_image(self) -> None:
        self.assertTrue(self.census.events[0].null)
        self.assertFalse(self.census.events[0x43].null)

    def test_chained_events_are_called_from_inside_a_registered_scene(self) -> None:
        for event, caller in ((0x07, 0x800B), (0x11, 0x8010)):
            self.assertEqual(ec.chain_callers(self.census.events[event]), [caller])
            self.assertIn(caller, self.registry)
            self.assertEqual(ec.disposition(self.census, self.registry, ec.ALLOWLIST, event)[0], "chained")

    def test_unreferenced_trigger_slots_fire_nothing(self) -> None:
        """A slot no map lists cannot fire; `RunEvent_Null4D` is one of them."""
        self.assertIn(0x4D, self.census.unreferenced_slots)
        self.assertIn(0x01, self.census.unreferenced_slots)

    def test_the_rust_test_shares_the_ceiling_and_the_issues(self) -> None:
        text = RUST_TABLE.read_text(encoding="utf-8")
        ceiling = re.search(r"const ALLOWLIST_CEILING: usize = (\d+);", text)
        self.assertEqual(int(ceiling.group(1)), ec.ALLOWLIST_CEILING)
        issues = re.search(r"const ISSUES: \[u32; \d+\] = \[([^\]]*)\];", text)
        self.assertEqual([int(v) for v in issues.group(1).split(",")], list(ec.ISSUES))

    @unittest.skipUnless(ec.ASM.exists(), "reference/ps4disasm not present (labels are navigation only)")
    def test_the_rust_table_is_the_census(self) -> None:
        text = RUST_TABLE.read_text(encoding="utf-8")
        self.assertEqual(
            between(text, RUST_BEGIN, RUST_END),
            ec.rust_rows(self.census, self.registry),
            "regenerate with `python3 -m tests.event_census --rust`",
        )

    @unittest.skipUnless(ec.ASM.exists(), "reference/ps4disasm not present (labels are navigation only)")
    def test_the_document_is_the_census(self) -> None:
        text = DOC.read_text(encoding="utf-8")
        self.assertEqual(
            between(text, DOC_BEGIN, DOC_END),
            ec.doc_rows(self.census, self.registry),
            "regenerate with `python3 -m tests.event_census --doc`",
        )
        self.assertEqual(between(text, COUNTS_BEGIN, COUNTS_END), ec.issue_counts())


@unittest.skipUnless(ec.ROM.exists(), f"ROM fixture not present at {ec.ROM}")
class NegativeControl(unittest.TestCase):
    """Each leg of the census fails on its own defect."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = ec.ROM.read_bytes()
        cls.census = ec.build(cls.rom)
        cls.registry = ec.registry_events()

    def test_an_unregistered_scene_is_named(self) -> None:
        registry = dict(self.registry)
        del registry[0x43]
        found = ec.problems(self.census, registry)
        self.assertEqual(len(found), 1, found)
        self.assertIn("$0043 Event_OutsideRajaTemple", found[0])

    def test_a_scene_removed_from_the_sources_is_named(self) -> None:
        sources = {p.name: p.read_text(encoding="utf-8") for p in ec.SCENES_DIR.glob("*.rs")}
        text = sources["dezolis_route.rs"]
        broken = text.replace("event: EventIndex(0x0043),", "event: EventIndex(0x7FFF),")
        self.assertNotEqual(broken, text)
        sources["dezolis_route.rs"] = broken
        found = ec.problems(self.census, ec.registry_events(sources))
        self.assertTrue(any("$0043 Event_OutsideRajaTemple" in f for f in found), found)

    def test_a_registered_scene_left_in_the_allowlist_is_reported(self) -> None:
        allowlist = dict(ec.ALLOWLIST)
        allowlist[0x43] = (ec.ISSUE_DEZOLIS, "stale")
        found = ec.problems(self.census, self.registry, allowlist, ec.ALLOWLIST_CEILING + 1)
        self.assertTrue(any("$0043" in f and "only shrinks" in f for f in found), found)

    def test_a_longer_allowlist_is_reported(self) -> None:
        found = ec.problems(self.census, self.registry, ec.ALLOWLIST, len(ec.ALLOWLIST) - 1)
        self.assertTrue(any("over its ceiling" in f for f in found), found)

    def test_an_entry_without_an_issue_is_reported(self) -> None:
        allowlist = dict(ec.ALLOWLIST)
        allowlist[0x37] = (0, "no issue")
        found = ec.problems(self.census, self.registry, allowlist)
        self.assertTrue(any("$0037" in f and "tracked issue" in f for f in found), found)

    def test_a_new_event_without_a_scene_is_reported(self) -> None:
        allowlist = dict(ec.ALLOWLIST)
        del allowlist[0x96]
        found = ec.problems(self.census, self.registry, allowlist)
        self.assertEqual(len(found), 1, found)
        self.assertIn("$0096 Event_PenguinFeedStolen", found[0])

    def test_a_new_immediate_writer_in_the_image_is_reported(self) -> None:
        patched = bytearray(self.rom)
        patched[0x7C000 : 0x7C006] = bytes.fromhex("31FC00AAECA8")
        with self.assertRaisesRegex(AssertionError, r"\$07C004: unclassified `Event_Index` immediate write 0XAA"):
            ec.build(bytes(patched))

    def test_an_unknown_operand_shape_is_reported(self) -> None:
        patched = bytearray(self.rom)
        patched[0x7C000 : 0x7C004] = bytes.fromhex("4278ECA8")  # clr.w Event_Index.w
        with self.assertRaisesRegex(AssertionError, r"\$07C002: an `Event_Index` operand"):
            ec.build(bytes(patched))

    def test_a_new_direct_call_onto_an_event_is_reported(self) -> None:
        patched = bytearray(self.rom)
        routine = ec.routine_for(self.rom, 0x4C)
        patched[0x7C000 : 0x7C006] = bytes.fromhex("4EB9") + routine.to_bytes(4, "big")
        with self.assertRaisesRegex(AssertionError, r"\$07C000: an unclassified direct call"):
            ec.build(bytes(patched))

    def test_a_stale_document_row_is_reported(self) -> None:
        if not ec.ASM.exists():
            self.skipTest("reference/ps4disasm not present")
        text = DOC.read_text(encoding="utf-8")
        broken = text.replace("| `$0043` |", "| `$0143` |", 1)
        self.assertNotEqual(broken, text)
        self.assertNotEqual(
            between(broken, DOC_BEGIN, DOC_END), ec.doc_rows(self.census, self.registry)
        )


if __name__ == "__main__":
    unittest.main()
