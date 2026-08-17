import unittest

from psiv_tools.dialogue_census import census
from psiv_tools.presentation_pack import DIALOGUE_ACTION_PANEL_IDS


class DialogueCensusTests(unittest.TestCase):
    def test_action_payloads_and_locations_are_counted(self):
        document = {
            "trees": [
                {
                    "tree": 7,
                    "label": "DialogueTree7",
                    "entries": [
                        {
                            "id": 3,
                            "segments": [
                                {
                                    "ctrl": "0xF2",
                                    "action_id": 0,
                                    "action": "load_panel",
                                    "operands": [0, 0x30],
                                },
                                {
                                    "ctrl": "action",
                                    "action_id": 3,
                                    "action": "load_sound",
                                    "operands": [0xFE],
                                },
                            ],
                        }
                    ],
                }
            ]
        }

        result = census(document)
        self.assertEqual(result["entries"], 1)
        self.assertEqual(result["action_occurrences"], 2)
        self.assertEqual(result["panel_ids"], [0x30])
        self.assertEqual(
            result["actions"]["load_panel"]["payloads"]["0x30"]["entries"],
            {"DialogueTree7:3": 1},
        )
        self.assertEqual(
            result["actions"]["load_sound"]["payloads"]["0xFE"]["entries"],
            {"DialogueTree7:3": 1},
        )

    def test_retail_extract_has_all_action_kinds_and_known_totals(self):
        from pathlib import Path

        path = Path(__file__).parents[1] / "generated/dialogue.json"
        if not path.is_file():
            self.skipTest("generated dialogue extract is not present")
        import json

        result = census(json.loads(path.read_text(encoding="utf-8")))
        self.assertEqual(result["entries"], 2736)
        self.assertEqual(result["action_occurrences"], 266)
        self.assertEqual(result["panel_count"], 163)
        self.assertEqual(set(result["panel_ids"]), set(DIALOGUE_ACTION_PANEL_IDS))
        self.assertEqual(
            {
                action: item["occurrences"]
                for action, item in result["actions"].items()
            },
            {
                "load_panel": 165,
                "destroy_last_panel": 35,
                "destroy_all_panels": 7,
                "load_sound": 35,
                "load_sound_2": 14,
                "update_palette": 4,
                "zio_eyes_red": 1,
                "pause_music": 1,
                "resume_music": 1,
                "sabotage_alarm_red_palette": 1,
                "set_event_flag": 1,
                "elsydeon_broken": 1,
            },
        )
