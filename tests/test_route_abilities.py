"""The route census, `oracle/sweep/route_abilities.py`: constructed controls.

The derivation joins generated tables, so the controls hand it constructed ones
(`Data`) and never a cartridge record. The two scope readers (a named stretch
and an explicit map pattern) share one derivation; the ledger the classes come
from is the committed `docs/battle/ENEMY_ABILITIES.md`, whose counts a test
recomputes.
"""
import pathlib
import re
import tempfile
import unittest

from oracle.sweep import route_abilities as ra
from oracle.sweep.route_abilities import (Data, Scope, classes, derive,
                                          ledger_classes, scene_records)


def data() -> Data:
    skill = lambda i: {"id": i, "display_name": f"S{i}", "effect_id": 1}
    return Data(
        encounters={
            5: {"map_id": 5, "mode": "position_grid",
                "groups_available": [7], "vehicle_groups": [8]},
            6: {"map_id": 6, "mode": "none"},
        },
        groups={7: [10] * 32, 8: [11] * 32},
        formations={10: {"enemies": [{"enemy": {"id": 1}}]},
                    11: {"enemies": [{"enemy": {"id": 2}}]}},
        bosses={9: {"enemies": [{"enemy": {"id": 3}}]}},
        enemies={
            1: {"symbol": "A", "ai": {"regular_ability_ids": [1, 0],
                                      "condition_ids": [4],
                                      "conditional_ability_ids": [2]}},
            2: {"symbol": "B", "ai": {"regular_ability_ids": [3]}},
            3: {"symbol": "C", "ai": {"regular_ability_ids": [4]}},
        },
        skills={i: skill(i) for i in range(1, 5)},
        map_symbols={5: "world", 6: "event"})


def scope() -> Scope:
    found = Scope()
    found.add_map(5, "test")
    found.add_map(6, "test")
    found.add_battle(9, "SCENE")
    return found


LEDGER = {i: {"class": "damage", "status": "unsupported", "section": "2",
              "gated": False} for i in range(1, 5)}


class DerivationTests(unittest.TestCase):
    def test_conditional_vehicle_and_event_only_carriers_all_count(self):
        result = derive(data(), scope(), LEDGER)
        self.assertEqual(result["groups"], [7, 8])
        self.assertEqual([a["ability"] for a in result["abilities"]], [1, 2, 3, 4])
        last = result["abilities"][-1]
        self.assertEqual((last["random_carriers"], last["event_carriers"]),
                         ([], [3]))
        conditional = result["abilities"][1]
        self.assertEqual(conditional["carriers"], {"1": "conditional:4"})
        # A vehicle-only carrier is listed apart from the on-foot groups.
        vehicle = result["abilities"][2]
        self.assertEqual((vehicle["foot_maps"], vehicle["vehicle_maps"]),
                         ([], [5]))

    def test_negative_controls_each_half_of_the_input_matters(self):
        baseline = derive(data(), scope(), LEDGER)["abilities"]
        # A conditional-only ability vanishes if the reader omits that half of
        # the AI table, even with identical formations.
        broken = data()
        broken.enemies[1]["ai"]["conditional_ability_ids"] = []
        self.assertNotEqual(derive(broken, scope(), LEDGER)["abilities"], baseline)
        # On-foot-only enumeration loses the vehicle carrier.
        broken = data()
        broken.encounters[5]["vehicle_groups"] = []
        self.assertNotEqual(derive(broken, scope(), LEDGER)["abilities"], baseline)
        # A scope without the event battle loses the event-only carrier.
        narrow = scope()
        narrow.battles.clear()
        self.assertNotEqual(derive(data(), narrow, LEDGER)["abilities"], baseline)

    def test_unclassified_ability_is_an_error(self):
        with self.assertRaisesRegex(ValueError, "no inventory classification"):
            derive(data(), scope(), {1: LEDGER[1]})


class ScopeTests(unittest.TestCase):
    def test_scene_boundary_does_not_import_an_unselected_battle(self):
        text = """pub static ONE: Scene = Scene { ops: &[
            SceneOp::LoadMap { map: 0x15F, prev_map: 1 },
            SceneOp::StartBattle { index: 8 }, ] };
            pub static TWO: Scene = Scene { ops: &[
            SceneOp::LoadFlightMap { map: 0x20 },
            SceneOp::StartBattle { index: 9 }, ] };"""
        records = scene_records(text)
        self.assertEqual(records["ONE"], ([0x15F], [8]))
        self.assertEqual(records["TWO"], ([0x20], [9]))

    def test_the_named_stretch_reads_the_route_and_the_scenes(self):
        found = ra.stretch_scope(ra.STRETCHES["zelan-kuran"])
        self.assertEqual(sorted(found.battles), [8, 9])
        self.assertTrue(set(range(0x190, 0x198)) <= set(found.maps))
        self.assertTrue(any(where.startswith("chapter ")
                            for reasons in found.maps.values() for where in reasons))
        self.assertTrue(any(where.startswith("scene ")
                            for reasons in found.maps.values() for where in reasons))

    def test_a_chapter_appended_to_the_route_does_not_widen_a_stretch(self):
        import json
        original = json.loads(ra.ROUTE.read_text())
        extended = json.loads(ra.ROUTE.read_text())
        extended["chapters"].append({
            "id": "an-appended-chapter", "title": "t", "source": "s",
            "random_battle_policy": "run_then_win",
            "objectives": [{"do": "go_to_map", "map": 0x1A0}],
        })
        scopes = []
        with tempfile.TemporaryDirectory() as root:
            for name, route in (("original", original), ("extended", extended)):
                path = pathlib.Path(root, f"{name}.json")
                path.write_text(json.dumps(route))
                saved, ra.ROUTE = ra.ROUTE, path
                try:
                    scopes.append(sorted(
                        ra.stretch_scope(ra.STRETCHES["zelan-kuran"]).maps))
                finally:
                    ra.ROUTE = saved
        self.assertEqual(scopes[0], scopes[1])
        self.assertNotIn(0x1A0, scopes[1])
        # Negative control: a stretch with no named end would have taken it.
        open_ended = ra.Stretch(first_chapter="zelan-wren-canceller",
                                last_chapter="an-appended-chapter",
                                first_scene=None, last_scene=None)
        with tempfile.TemporaryDirectory() as root:
            path = pathlib.Path(root, "extended.json")
            path.write_text(json.dumps(extended))
            saved, ra.ROUTE = ra.ROUTE, path
            try:
                self.assertIn(0x1A0, ra.stretch_scope(open_ended).maps)
            finally:
                ra.ROUTE = saved

    def test_the_c7_stretch_names_its_chapters_and_its_event_battles(self):
        found = ra.stretch_scope(ra.STRETCHES["dezolis-air-castle"])
        self.assertEqual(sorted(found.battles), [10, 14])
        self.assertIn(0x184, found.maps)
        self.assertTrue(any("dezolis-ice-digger" in why
                            for reasons in found.maps.values() for why in reasons))
        # The route's first chapters are not in it.
        self.assertFalse(any("kuran-dark-force-1" in why
                             for reasons in found.maps.values() for why in reasons))

    def test_a_map_outside_the_encounter_table_draws_nothing(self):
        outside = data()
        outside.encounters[7] = {"map_id": 7, "mode": "outside_table",
                                 "in_table": False, "group": None}
        outside.map_symbols[7] = "space"
        found = scope()
        found.add_map(7, "test")
        result = derive(outside, found, LEDGER)
        self.assertEqual(result["groups"], [7, 8], "the same groups as without it")
        self.assertEqual(result["abilities"], derive(data(), scope(), LEDGER)["abilities"])
        # Negative control: an unknown mode is still an error, not a silent skip.
        outside.encounters[7]["mode"] = "mystery"
        with self.assertRaises(KeyError):
            derive(outside, found, LEDGER)

    def test_a_pattern_selects_by_symbol_and_an_empty_one_is_an_error(self):
        found = ra.pattern_scope(data(), "^wor", [])
        self.assertEqual(sorted(found.maps), [5])
        with self.assertRaisesRegex(ValueError, "selects no maps"):
            ra.pattern_scope(data(), "^nothing", [])

    def test_a_scene_document_without_a_data_row_is_an_error(self):
        with tempfile.TemporaryDirectory() as root:
            document = pathlib.Path(root, "scene.md")
            document.write_text("# a scene with no Data row\n")
            with self.assertRaisesRegex(ValueError, "no literal scene Data row"):
                ra.pattern_scope(data(), "^wor", [document])


class LedgerTests(unittest.TestCase):
    def test_inventory_class_is_not_inferred_from_ability_name(self):
        text = "\n".join((
            "| `$02` (2) damage-ish name | record | chain | effect | carrier | status/stat effect | unsupported |",
            "| `$03` (3) harmless name | record | chain | effect | carrier | damage † | unsupported |",
            "| `$04` (4) name | record | chain | effect | carrier | — | implemented enemy_damage::resolve_damage_skill |"))
        self.assertEqual(classes(text),
                         {2: "status/stat effect", 3: "damage", 4: "damage"})
        self.assertTrue(ledger_classes(text)[3]["gated"])

    def test_rows_of_other_sections_are_not_read_as_inventory(self):
        text = "\n".join((
            "## 2. Inventory",
            "| `$02` (2) A | s | d | h | c | damage | unsupported |",
            "## 3. Conditional",
            "| `$05` (5) B | s | carriers | 1 Cond | scripted/custom | implemented |",
            "## 4. Where they appear",
            "| `$02` FLAME | damage | 5 | map |"))
        self.assertEqual(classes(text), {2: "damage", 5: "scripted/custom"})

    def test_the_committed_ledger_counts_match_its_rows(self):
        """The summary line is computed from section 2's status cells."""
        rows = {a: row for a, row in ledger_classes().items()
                if row["section"] == "2"}
        count = {word: sum(row["status"].startswith(word) for row in rows.values())
                 for word in ("implemented", "partial", "unsupported")}
        self.assertEqual(sum(count.values()), len(rows), "a row has no status word")
        text = ra.LEDGER.read_text()
        summary = re.search(
            r"\*\*(\d+) distinct nonzero regular ability ids\.\*\* (\d+) implemented, "
            r"(\d+) partial", text)
        self.assertEqual((int(summary[1]), int(summary[2]), int(summary[3])),
                         (len(rows), count["implemented"], count["partial"]))
        table = re.search(r"\| unsupported \| (\d+) \|", text)
        self.assertEqual(int(table[1]), count["unsupported"])
        self.assertEqual(int(re.search(r"\| implemented \| (\d+)", text)[1]),
                         count["implemented"])


class CommittedDocBlock(unittest.TestCase):
    """The route doc's tables are the tool's output, never a hand-kept copy."""

    DOC = ra.ROOT / "docs" / "battle" / "ENEMY_ABILITIES_ROUTE.md"

    def test_the_route_doc_block_matches_the_derivation(self):
        try:
            data = Data.load(ra.GENERATED)
        except FileNotFoundError as error:
            self.skipTest(f"generated tables absent (local input): {error}")
        scope = ra.stretch_scope(ra.STRETCHES["zelan-kuran"])
        expected = ra.markdown(derive(data, scope, ra.ledger_classes()))
        text = self.DOC.read_text()
        self.assertEqual(
            text, ra.with_doc_block(text, expected),
            "docs/battle/ENEMY_ABILITIES_ROUTE.md is stale: run "
            "python3 -m oracle.sweep.route_abilities --update-doc "
            "docs/battle/ENEMY_ABILITIES_ROUTE.md")

    def test_a_stale_block_is_detected_and_a_missing_one_is_an_error(self):
        text = "intro\n<!-- route_abilities:begin -->\n\nold\n\n<!-- route_abilities:end -->\n"
        self.assertNotEqual(text, ra.with_doc_block(text, "new"))
        self.assertIn("\n\nnew\n\n", ra.with_doc_block(text, "new"))
        with self.assertRaises(ValueError):
            ra.with_doc_block("no markers here", "new")


if __name__ == "__main__":
    unittest.main()
