"""A requested ability must occur inside the extracted capture, not the probe."""
import unittest

from oracle.sweep.capture_route import observed_rounds, out_of_lane


class CaptureRouteTests(unittest.TestCase):
    def test_observation_is_a_used_ability_in_a_captured_round(self):
        fixture = {"rounds": [{"round": 1, "actions": [
            {"kind": "attack", "ability": 0x5A, "targets": []}]}, {"round": 2, "actions": [
            {"kind": "ability", "ability": 0x5A, "targets": [{"damage": 10}]}]}]}
        self.assertEqual(observed_rounds(fixture, 0x5A), [2])
        fixture["rounds"].pop()
        self.assertEqual(observed_rounds(fixture, 0x5A), [])

    def test_an_animation_only_or_wasted_turn_is_not_damage_evidence(self):
        fixture = {"rounds": [{"round": 1, "actions": [
            {"kind": "ability", "ability": 0x5A, "targets": []},
            {"kind": "no_effect", "ability": 0x5A, "targets": []}]}]}
        self.assertEqual(observed_rounds(fixture, 0x5A), [])

    def test_status_turn_cannot_be_smuggled_into_a_damage_only_receipt(self):
        fixture = {"rounds": [{"round": 1, "actions": [
            {"kind": "ability", "ability": 0x5A},
            {"kind": "ability", "ability": 0x4B}]}]}
        self.assertEqual(out_of_lane(fixture, {0x5A}), [0x4B])
        self.assertEqual(out_of_lane(fixture, {0x5A, 0x4B}), [])
