"""Ensure the asset-free CI lane executes the extract freshness regressions."""
import re
import unittest
from pathlib import Path


class ExtractCoverage(unittest.TestCase):
    def test_platform_check_executes_extract_stamp_suite(self):
        flake = (Path(__file__).resolve().parents[1] / "flake.nix").read_text()
        block = flake.split('platform-tests =', 1)[1].split('touch $out', 1)[0]
        command = re.search(r"python3 -m unittest ([^\n]+)", block)
        self.assertIsNotNone(command)
        suite = unittest.defaultTestLoader.loadTestsFromNames(command[1].split())

        def ids(tests):
            for test in tests:
                if isinstance(test, unittest.TestSuite):
                    yield from ids(test)
                else:
                    yield test.id()

        self.assertIn(
            "tests.test_extract_stamp.Loader."
            "test_an_interrupted_extract_refuses_a_brand_new_table_name",
            set(ids(suite)),
            "CI must execute the extract regressions, rather than only read their files",
        )
