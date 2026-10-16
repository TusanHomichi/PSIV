"""The endgame capture recipe, `oracle/sweep/endgame.py`, and the shared
`oracle/sweep/forced_cases.py`: constructed controls."""
import unittest

from oracle.sweep import endgame as recipe
from oracle.sweep import forced_cases


class Recipe(unittest.TestCase):
    def test_every_case_is_a_formation_or_an_event_with_distinct_names(self):
        names = [case.name for case in recipe.CASES]
        self.assertEqual(len(names), len(set(names)))
        for case in recipe.CASES:
            self.assertNotEqual(case.formation is None, case.event is None, case.name)
            self.assertLessEqual(case.kept, case.rounds, case.name)
            for ability in case.required:
                self.assertTrue(0 < ability < 0x71, case.name)

    def test_every_script_names_each_party_slot_every_round(self):
        for case in recipe.CASES:
            if case.script is None:
                continue
            for commands in case.script["rounds"]:
                self.assertEqual(sorted(commands), ["1", "2", "3"], case.name)


class Observed(unittest.TestCase):
    FIXTURE = {"formation": {"enemies": [{"id": 6, "enemy_id": 23}]},
               "rounds": [
                   {"round": 1, "actions": [{"actor": 6, "ability": 0x0C},
                                            {"actor": 2, "ability": 0x0C}]},
                   {"round": 2, "actions": [{"actor": 7, "ability": 0x0D}],
                    "state_after": [{"id": 6, "enemy_id": 26}]}]}

    def test_every_required_ability_must_be_in_an_enemy_action(self):
        both = forced_cases.Case("c", "n", 0x0C, 2, formation=1, also=(0x0D,))
        self.assertEqual(forced_cases.observed(both, self.FIXTURE),
                         {0x0C: [1], 0x0D: [2]})
        # Negative control: an ability only a party action shows is missing.
        absent = forced_cases.Case("c", "n", 0x0C, 2, formation=1, also=(0x3A,))
        self.assertIsNone(forced_cases.observed(absent, self.FIXTURE))

    def test_a_seated_or_ability_free_case_requires_no_ability(self):
        seated = forced_cases.Case("c", "n", 0x0C, 2, formation=1, seated=26)
        self.assertEqual(seated.required, ())
        self.assertEqual(forced_cases.observed(seated, self.FIXTURE), {26: [2]})
        latch = forced_cases.Case("c", "n", None, 2, event=10)
        self.assertEqual(latch.required, ())
        self.assertEqual(forced_cases.observed(latch, self.FIXTURE), {})


if __name__ == "__main__":
    unittest.main()
