"""Run the gate's lock regressions in the flake's discovered gate tests.

Keeping this entry point under test_gate* also lets the negative check run
these tests with the base flake, before it knows any new check names.
"""


def load_tests(loader, tests, pattern):
    return loader.loadTestsFromNames([
        "tests.test_heavy_lock",
        "tests.test_heavy_lock_runner_wait",
    ])
