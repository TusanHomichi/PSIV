"""Ensure the asset-free CI lane executes the extract freshness regressions."""
import re
import unittest
from pathlib import Path


def platform_modules(flake):
    block = flake.split('platform-tests =', 1)[1].split('touch $out', 1)[0]
    command = re.search(r"python3 -m unittest ([^\n]+)", block)
    if command is None or command[1] != '${lib.concatStringsSep " " platformTests}':
        raise AssertionError("platform check must execute the shared platformTests list")
    declared = re.search(r'platformTests = \[(.*?)\];', flake, re.S)
    if declared is None:
        raise AssertionError("missing platformTests list")
    return re.findall(r'"([^"\n]+)"', declared[1])


class ExtractCoverage(unittest.TestCase):
    def setUp(self):
        self.flake = (Path(__file__).resolve().parents[1] / "flake.nix").read_text()

    def assert_extract_executes(self, flake):
        suite = unittest.defaultTestLoader.loadTestsFromNames(platform_modules(flake))

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

    def test_platform_check_executes_extract_stamp_suite(self):
        self.assert_extract_executes(self.flake)

    def test_omitting_extract_stamp_fails(self):
        with self.assertRaises(AssertionError):
            self.assert_extract_executes(self.flake.replace('"tests.test_extract_stamp"', ''))

    def test_not_executing_shared_list_fails(self):
        for command in ('tests.test_release', '${lib.concatStringsSep " " anotherList}'):
            with self.subTest(command=command), self.assertRaises(AssertionError):
                self.assert_extract_executes(self.flake.replace(
                    '${lib.concatStringsSep " " platformTests}', command,
                ))

    def test_gate_coverage_names_direct_modules_and_adapter_suites_only(self):
        block = self.flake.split('gate-tests =', 1)[1].split("} ''", 1)[0]
        declared = re.search(r'passthru.testFiles = \[(.*?)\];', block, re.S)
        self.assertIsNotNone(declared)
        self.assertEqual(set(re.findall(r'"([^"\n]+)"', declared[1])), {
            'tests/test_gate*.py',
            'tests/test_heavy_lock.py',
            'tests/test_heavy_lock_runner_wait.py',
            'tests/test_pack_parity.py',
        })
