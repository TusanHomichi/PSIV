"""The flash-only attack verifier's verdict and cases, without Godot.

    PYTHONPATH=. python3 -m unittest tests.test_verify_native_flash_attack -v

tools/verify_native_flash_attack.py runs the native check; these pin what it
accepts. Every control below is one way a native run can go wrong, built by
hand from a passing report.
"""

import copy
import json
import unittest
from pathlib import Path

from tools import verify_native_flash_attack as flash

ROOT = Path(__file__).resolve().parents[1]
PACK = ROOT / "runtime-pack"

WHITE = [255, 255, 255, 255]
PINK = [255, 224, 224, 255]
#: Locusta and Sand Newt: receipt routine bodies 0x010C7C and 0x00FCCA in
#: oracle/fixtures/battle_animation_remainder.json, typed here by hand.
RECEIPT = {14, 56}


def body(fighter, enemy_id=14, attack=None, tint=WHITE, children=0):
    return {"fighter": fighter, "enemy_id": enemy_id, "attack": attack, "tint": tint,
            "children": children}


def attack(tick, fighter, enemy_id=14, kind="flash", tint=PINK, children=0, orphans=0):
    return {"tick": tick, "fighter": fighter, "enemy_id": enemy_id, "attack": kind,
            "tint": tint, "children": children, "orphans": orphans}


def passing():
    """What the driver reports for formation $12 (two Locustas), as floats the
    way Godot's JSON writes numbers."""
    return {"complete": True, "reason": "", "orphan_monitor_alive": True,
            "idle": [body(6.0), body(7.0)],
            "attacks": [attack(152, 6), attack(224, 7), attack(524, 7)],
            "orphans_battle_start": 0, "orphans_max": 0, "orphans_end": 0,
            "final": [body(6.0), body(7.0)]}


class Verdict(unittest.TestCase):
    def failures(self, change):
        report = passing()
        change(report)
        return flash.verdict(report, RECEIPT, attacks=3)

    def test_a_clean_run_passes(self):
        self.assertEqual(flash.verdict(passing(), RECEIPT, attacks=3), [])

    def test_a_leaked_node_per_attack_fails(self):
        # The #92 defect: each flash attack leaves one unparented Sprite2D.
        def leak(report):
            report["orphans_max"] = 3
            report["orphans_end"] = 3
        self.assertEqual(self.failures(leak), ["orphans_max = 3, not 0", "orphans_end = 3, not 0"])

    def test_an_orphan_freed_before_the_end_still_fails(self):
        def transient(report):
            report["orphans_max"] = 1
        self.assertEqual(self.failures(transient), ["orphans_max = 1, not 0"])

    def test_an_orphan_before_the_battle_fails(self):
        def early(report):
            report["orphans_battle_start"] = 2
        self.assertEqual(self.failures(early), ["orphans_battle_start = 2, not 0"])

    def test_a_dead_orphan_monitor_fails(self):
        def dead(report):
            report["orphan_monitor_alive"] = False
        self.assertEqual(self.failures(dead),
                         ["the orphan-node monitor did not count a test orphan"])

    def test_a_sheet_enemy_fails(self):
        # The fixture hits a sheet enemy: tape 07's Zoran Bult (10), whose
        # sheet layer is one child of its body.
        def sheet(report):
            report["idle"] = [body(6, 10), body(7, 10)]
            report["attacks"] = [attack(188, 6, 10, "sheet", WHITE, 1)] * 3
            report["final"] = [body(6, 10), body(7, 10)]
        self.assertEqual(self.failures(sheet), [
            "0 flash attacks, not 3",
            *["tick 188 fighter 6 enemy 10: drew a sheet attack, not the flash branch",
              "tick 188 fighter 6 enemy 10: enemy 10 is not a receipt enemy",
              "tick 188 fighter 6 enemy 10: 1 children, idle 0"] * 3])

    def test_a_flash_attack_outside_the_receipts_fails(self):
        def stranger(report):
            report["attacks"][0]["enemy_id"] = 2
        self.assertEqual(self.failures(stranger),
                         ["tick 152 fighter 6 enemy 2: enemy 2 is not a receipt enemy"])

    def test_an_untinted_flash_fails(self):
        def untinted(report):
            report["attacks"][1]["tint"] = WHITE
        self.assertEqual(self.failures(untinted),
                         ["tick 224 fighter 7 enemy 14: the flash attack left the body untinted"])

    def test_a_flash_that_parents_a_node_fails(self):
        def parented(report):
            report["attacks"][2]["children"] = 1
        self.assertEqual(self.failures(parented),
                         ["tick 524 fighter 7 enemy 14: 1 children, idle 0"])

    def test_too_few_attacks_fail(self):
        def short(report):
            report["attacks"] = report["attacks"][:2]
        self.assertEqual(self.failures(short), ["2 flash attacks, not 3"])

    def test_an_unfinished_run_fails_and_still_names_what_it_saw(self):
        def unfinished(report):
            report.update(complete=False, reason="timed out", orphans_end=-1, final=None)
            report["attacks"][0]["kind"] = "sheet"
            report["attacks"][0]["attack"] = "sheet"
        self.assertEqual(self.failures(unfinished), [
            "incomplete: timed out", "2 flash attacks, not 3",
            "tick 152 fighter 6 enemy 14: drew a sheet attack, not the flash branch"])

    def test_a_body_left_tinted_or_mid_attack_fails(self):
        def stuck(report):
            report["final"][0] = body(6, attack="flash", tint=PINK)
            report["final"][1] = body(7, children=1)
        self.assertEqual(self.failures(stuck), [
            "after cleanup fighter 6: still in a flash attack",
            f"after cleanup fighter 6: tint {PINK}, not {WHITE}",
            "after cleanup fighter 7: 1 children, idle 0"])

    def test_a_complete_run_with_no_final_bodies_fails(self):
        def empty(report):
            report["final"] = []
        self.assertEqual(self.failures(empty), ["no enemy body observed after cleanup"])

    def test_the_passing_report_is_not_mutated(self):
        report = passing()
        before = copy.deepcopy(report)
        flash.verdict(report, RECEIPT, attacks=3)
        self.assertEqual(report, before)


class Cases(unittest.TestCase):
    def test_the_receipt_ids_are_the_census_set(self):
        ids = flash.receipt_enemy_ids()
        self.assertEqual(len(ids), 32)
        self.assertTrue(RECEIPT <= ids)
        self.assertNotIn(10, ids)  # Zoran Bult draws a sheet attack

    def test_each_case_fields_only_receipt_enemies_of_two_routines(self):
        if not (PACK / "battle/formations.json").is_file():
            self.skipTest("runtime pack not present")
        formations = {entry["id"]: [slot["enemy_id"] for slot in entry["enemies"]]
                      for entry in json.loads((PACK / "battle/formations.json").read_text())["formations"]}
        receipts = json.loads(flash.RECEIPTS.read_text())["receipts"]
        routines = set()
        for name, formation in flash.CASES:
            members = formations[formation]
            # Two bodies at least, so a body attacks again after its cleanup.
            self.assertGreaterEqual(len(members), 2, name)
            for enemy_id in members:
                (routine,) = [receipt["routine_offset"] for receipt in receipts
                              if enemy_id in receipt["enemy_ids"]]
                routines.add(routine)
        self.assertEqual(routines, {"0x010C7C", "0x00FCCA"})

    def test_a_case_names_a_formation_in_hex(self):
        self.assertEqual(flash.parse_case("zoran=8a"), ("zoran", 0x8A))
        self.assertEqual(flash.parse_case("x=0x105"), ("x", 0x105))

    def test_the_case_environment_drops_other_selectors(self):
        import os
        from unittest import mock
        with mock.patch.dict(os.environ, {"PSIV_DEBUG_CAMP": "1", "PSIV_LOAD_SLOT": "2",
                                          "PSIV_RUNTIME_PACK": "/elsewhere"}):
            env = flash.case_env(0x12, Path("/s"), Path("/r.json"), Path("/pack"))
        self.assertNotIn("PSIV_DEBUG_CAMP", env)
        self.assertNotIn("PSIV_LOAD_SLOT", env)
        self.assertEqual(env["PSIV_RUNTIME_PACK"], "/pack")
        self.assertEqual(env["PSIV_DEBUG_BATTLE_FORMATION"], "12")
        self.assertEqual(env["PSIV_DEBUG_BATTLE_WINDOW"], "top")
        self.assertEqual(env["PSIV_SAVE_DIR"], "/s")


if __name__ == "__main__":
    unittest.main()
