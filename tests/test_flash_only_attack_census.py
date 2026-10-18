"""No enemy attack in the runtime pack takes the flash-only presentation branch.

    PYTHONPATH=. python3 -m unittest tests.test_flash_only_attack_census -v

`EnemySprite::begin_attack` (rust/psiv-godot/src/battle/attack.rs) draws an
attack on a sprite layer when the enemy has a usable sheet attack, and only
tints the body otherwise: the flash-only branch. That branch once allocated a
Sprite2D it never parented or freed, one orphan node per attack (#92). The fix
is in the type (a flash-only attack holds no layer), but no native run
exercises the branch, because today's pack never reaches it: every attack
record has exact attack art whose frames match its timing.

If this test fails, the branch is reachable: some record would now flash only.
Before relaxing it, add a native check that drives that enemy's attack through
`PSIV_DEBUG_BATTLE` and asserts no orphan node remains (the memory bracket's
orphan check in tools/native_rss.py is the run-wide guard).
"""

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACK = ROOT / "runtime-pack"


def flash_only_attacks(animations: list, attacks: list, enemies: list) -> list:
    """The (enemy_id, symbol) of every attack record that would take the
    flash-only branch: a flash-timed record with frame timing whose enemy has
    body art but no exact sheet attack matching that timing."""
    by_id = {entry["id"]: entry for entry in attacks}
    with_art = {entry["id"] for entry in enemies}
    found = []
    for record in animations:
        sequence = record.get("frame_sequence") or {}
        count = sequence.get("frame_count")
        durations = sequence.get("frame_durations") or [sequence.get("frame_duration")] * (count or 0)
        attack = by_id.get(record["enemy_id"])
        sheet = bool(attack is not None and attack.get("status") == "exact" and attack.get("frames")
                     and len(attack["frames"]) == count and len(durations) == count
                     and all(durations))
        if (record.get("flash_timing_proven") and sequence.get("total_frames")
                and record["enemy_id"] in with_art
                and not (record.get("sprite_sheet_proven") and sheet)):
            found.append((record["enemy_id"], record.get("symbol")))
    return found


def load(relative: str, key: str) -> list:
    return json.loads((PACK / relative).read_text())[key]


class FlashOnlyAttackCensus(unittest.TestCase):
    def test_no_pack_attack_takes_the_flash_only_branch(self):
        if not (PACK / "manifest.json").is_file():
            self.skipTest("runtime pack not present")
        animations = load("battle/enemy_animations.json", "animations")
        found = flash_only_attacks(animations, load("battle/art/enemy_attacks.json", "enemies"),
                                   load("battle/art/enemies.json", "enemies"))
        self.assertGreater(len(animations), 0)
        self.assertEqual(found, [], "the flash-only branch is reachable; see this module's docstring")

    def test_a_record_without_exact_attack_art_is_reported(self):
        # Negative control: one record whose attack art is not exact.
        animations = [{"enemy_id": 7, "symbol": "Synthetic", "flash_timing_proven": True,
                       "sprite_sheet_proven": True,
                       "frame_sequence": {"frame_count": 2, "frame_duration": 3,
                                          "total_frames": 6}},
                      {"enemy_id": 8, "symbol": "Sheeted", "flash_timing_proven": True,
                       "sprite_sheet_proven": True,
                       "frame_sequence": {"frame_count": 2, "frame_duration": 3,
                                          "total_frames": 6}}]
        attacks = [{"id": 7, "status": "pending", "frames": []},
                   {"id": 8, "status": "exact", "frames": [{}, {}]}]
        enemies = [{"id": 7}, {"id": 8}]
        self.assertEqual(flash_only_attacks(animations, attacks, enemies), [(7, "Synthetic")])
        # A frame-count mismatch takes the branch too.
        attacks[1]["frames"] = [{}]
        self.assertEqual(flash_only_attacks(animations, attacks, enemies),
                         [(7, "Synthetic"), (8, "Sheeted")])


if __name__ == "__main__":
    unittest.main()
