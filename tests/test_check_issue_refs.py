"""The closed-issue deferral check (#159) on fake issue states.

    PYTHONPATH=. python3 -m unittest tests.test_check_issue_refs -v
"""

import unittest
from pathlib import Path

from tools.check_issue_refs import scan_text

STATES = {"3": "CLOSED", "4": "OPEN", "5": "CLOSED"}


class Deferrals(unittest.TestCase):
    def hits(self, text):
        return scan_text(Path("docs/README.md"), text, STATES)

    def test_a_deferral_to_a_closed_issue_is_reported(self):
        hits = self.hits("The remaining work is pending until #3 lands.\n")
        self.assertEqual([(hit["issue"], hit["line"]) for hit in hits], [(3, 1)])

    def test_history_mention_of_a_closed_issue_is_not_reported(self):
        self.assertEqual(self.hits("Fixed in #3 by the rewrite.\n"), [])

    def test_a_deferral_to_an_open_issue_is_not_reported(self):
        self.assertEqual(self.hits("Deferred until #4 lands.\n"), [])

    def test_the_phrase_must_be_close_to_the_reference(self):
        # The window is 80 characters on the same line.
        self.assertEqual(
            self.hits("Deferred until " + "x" * 100 + " #3.\n"), [])
        near = self.hits("Deferred until the rewrite #3.\n")
        self.assertEqual([hit["issue"] for hit in near], [3])

    def test_a_reference_on_a_later_line_is_not_flagged(self):
        # The scan is line-local by design; a section-level "still open"
        # heading is not a deferral phrase.
        self.assertEqual(self.hits("Deferred until the rewrite.\nSee #3.\n"), [])
        self.assertEqual([hit["issue"] for hit in self.hits("Deferred until #5.\n")],
                         [5])

    def test_every_reference_in_the_sentence_is_reported(self):
        hits = self.hits("Tracked in #3 and #5 and #4.\n")
        self.assertEqual([hit["issue"] for hit in hits], [3, 5])

    def test_a_phrase_after_the_reference_is_a_deferral(self):
        hits = self.hits("#3 is still pending the rewrite.\n")
        self.assertEqual([hit["issue"] for hit in hits], [3])

    def test_a_word_containing_a_deferral_word_does_not_flag(self):
        # Whole words only.
        self.assertEqual(self.hits("The translator mentions #3.\n"), [])

    def test_tracked_as_an_adjective_is_not_a_deferral(self):
        self.assertEqual(self.hits("The tracked code mentions #3.\n"), [])
        self.assertEqual(self.hits("Tracked files sat over the limit (#3).\n"), [])
