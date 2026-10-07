"""Command identity and no-RNG turns are observations, not inferred attacks."""
import unittest

from oracle.fixture.commands import command_entry
from oracle.fixture.commands import inventory_at
from oracle.fixture.errors import FixtureError
from oracle.fixture.logs import Log
from oracle.fixture.observations import action_windows, round_tail_frames
from oracle.fixture.state import round_end_frame


def log(rows, hexadecimal=()):
    return Log(rows, {name: {"name": name, "hex": name in hexadecimal}
                      for name in rows[0] if name != "frame"})


class PartyFixture(unittest.TestCase):
    def test_round_inventory_retains_command_consumption_but_excludes_later_loot(self):
        rows = [dict(self.item_row(), frame=str(frame), battle_routine=routine,
                     menu_inventory_0=str(item))
                for frame, routine, item in ((1, "0000", 57), (2, "0018", 0),
                                              (3, "0018", 128))]
        observed = log(rows, ("battle_routine",))
        frame = round_end_frame(observed, 1, 3)
        self.assertEqual(frame, 2)
        self.assertEqual(inventory_at(observed, frame)[0], 0)
        # Without the observed victory seam, the later inventory write is an
        # in-battle effect and must remain visible, not be masked away.
        for row in rows:
            row["battle_routine"] = "0000"
        observed = log(rows, ("battle_routine",))
        self.assertEqual(round_end_frame(observed, 1, 3), 3)
        self.assertEqual(inventory_at(observed, 3)[0], 128)

    def test_every_command_byte_is_preserved_and_unknown_is_rejected(self):
        for index, name in enumerate(("attack", "technique", "skill", "item", "defend"), 1):
            if name == "item":
                continue  # concrete copy checked separately below
            row = {"frame": "1", "cmd0_index": str(index), "cmd0_id": "7",
                   "cmd0_target": "65535"}
            entry = command_entry(log([row]), 1, 1)
            self.assertEqual(entry["command"], name)
            self.assertEqual(entry["target"], -1)
            if name in ("technique", "skill"):
                self.assertEqual(entry["ability"], 7)
        row["cmd0_index"] = "0"
        with self.assertRaisesRegex(FixtureError, "unknown command"):
            command_entry(log([row]), 1, 1)

    def item_row(self):
        return {"frame": "1", "cmd0_index": "4", "cmd0_id": "57",
                "cmd0_target": "65535", "menu_item_source_0": "1",
                "menu_party_0": "1", "menu_item_cursor_0": "1",
                **{f"menu_equipment_1_{i}": "11" if i == 0 else "0" for i in range(4)},
                **{f"menu_inventory_{i}": "57" if i == 3 else "0" for i in range(40)}}

    def test_item_copy_uses_equipment_first_list_and_preserves_inventory_holes(self):
        row = self.item_row()
        self.assertEqual(command_entry(log([row]), 1, 1)["source"], {"inventory": 3})
        row.update(menu_item_cursor_0="0", cmd0_id="11", menu_item_source_0="0")
        self.assertEqual(command_entry(log([row]), 1, 1)["source"], {"equipment": 0})
        row["menu_item_cursor_0"] = "2"
        with self.assertRaisesRegex(FixtureError, "outside the live item list"):
            command_entry(log([row]), 1, 1)

    def turn_rows(self, status="0", menu="0000"):
        values = [{"frame": str(frame), "battle_actor": "0000",
                   "current_command": "0000", "battle_routine_2": menu,
                   "alys_status": status, "chaz_status": "0", "hahn_status": "0"}
                  for frame in range(1, 5)]
        values[1].update(battle_actor="0001", current_command="0500")
        values[2].update(battle_actor="0002", current_command="0500")
        values[3].update(battle_actor="0002", current_command="0500")
        return values

    def test_consecutive_defends_open_without_rng_but_list_scratch_and_skips_do_not(self):
        hex_fields = ("battle_actor", "current_command", "battle_routine_2")
        self.assertEqual(action_windows(log(self.turn_rows(), hex_fields), 1, 4, cuts=[1]),
                         [(1, 2, 2), (2, 3, 4)])
        self.assertEqual(action_windows(log(self.turn_rows(status="8"), hex_fields),
                                        1, 4, cuts=[1]), [(2, 3, 4)])
        self.assertEqual(action_windows(log(self.turn_rows(menu="0011"), hex_fields),
                                        1, 4, cuts=[1]), [])

    def test_wake_draws_close_the_last_action_and_never_open_a_phantom_attack(self):
        rows = self.turn_rows()
        for row, routine in zip(rows, (0, 0, 0x1C, 6)):
            row.update(battle_routine=str(routine))
        rows[2].update(battle_actor="0007", e2_status="24")
        rows[3].update(battle_actor="0007", e2_status="16")
        fields = ("battle_actor", "current_command", "battle_routine_2")
        observed = log(rows, fields)
        self.assertEqual(round_tail_frames(observed, 1, 4), {4})
        # The final actor wakes in the same sampled frame as the calls. Its
        # clean status must not manufacture a real attack either.
        self.assertEqual(action_windows(observed, 1, 4, cuts=[1], roll_frames=[4]),
                         [(1, 2, 3)])
        # Negative control: without the restoration transition those calls
        # really do open an enemy action, so RNG is not silently discarded.
        rows[2]["battle_routine"] = "0"
        self.assertEqual(action_windows(log(rows, fields), 1, 4, cuts=[1], roll_frames=[4]),
                         [(1, 2, 3), (7, 4, 4)])


if __name__ == "__main__":
    unittest.main()
