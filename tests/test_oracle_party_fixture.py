"""Command identity and no-RNG turns are observations, not inferred attacks."""
import unittest

from oracle.fixture.commands import command_entry
from oracle.fixture.errors import FixtureError
from oracle.fixture.logs import Log
from oracle.fixture.observations import action_windows


def log(rows, hexadecimal=()):
    return Log(rows, {name: {"name": name, "hex": name in hexadecimal}
                      for name in rows[0] if name != "frame"})


class PartyFixture(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
