"""The round-end state reports the fighters the battle has, not stale stats."""
import unittest

from oracle.fixture.logs import Log
from oracle.fixture.state import round_state


def log(rows):
    return Log(rows, {name: {"name": name, "hex": False}
                      for name in rows[0] if name != "frame"})


def row(objects, **values):
    """One frame: party slot 1 (Alys) and enemy slots 6 and 7, with the
    `Obj_Fighters` words `objects` gives for 6 and 7."""
    base = {"frame": "1", "menu_party_0": "1", "alys_hp": "50", "alys_status": "0",
            "alys_tp": "4", "e1_hp": "400", "e1_status": "128", "e1_id": "87",
            "e1_mdfs_bat": "10", "e2_hp": "232", "e2_status": "0", "e2_id": "84",
            "e2_mdfs_bat": "15", "menu_object_6": str(objects[0]),
            "menu_object_7": str(objects[1])}
    base.update(values)
    return base


class RoundState(unittest.TestCase):
    def test_an_emptied_enemy_slot_is_not_reported(self):
        # COMBINE's reload left slot 6 holding TwinArms and slot 7 empty: its
        # stats struct still reads the BladeRight that stood there.
        state = round_state(log([row((8, 0))]), 1, {1, 6, 7})
        self.assertEqual([entry["id"] for entry in state], [1, 6])
        self.assertEqual(state[1]["enemy_id"], 87)
        self.assertEqual(state[1]["status"], 0, "bit 7 is masked")

    def test_an_occupied_slot_is_reported_and_a_log_without_the_words_keeps_all(self):
        self.assertEqual([e["id"] for e in round_state(log([row((8, 9))]), 1, {1, 6, 7})],
                         [1, 6, 7])
        bare = row((8, 0))
        del bare["menu_object_6"], bare["menu_object_7"]
        self.assertEqual([e["id"] for e in round_state(log([bare]), 1, {1, 6, 7})],
                         [1, 6, 7])


if __name__ == "__main__":
    unittest.main()
