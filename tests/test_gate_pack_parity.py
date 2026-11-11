"""Run the pack parity gate's ROM-free regressions in the gate-tests check.

Keep the suite in its owning module; this adapter also lets the base flake's
name-discovered gate-tests check execute new parity tests in the negative run.
"""
from tests import test_pack_parity


def load_tests(loader, tests, pattern):
    return loader.loadTestsFromModule(test_pack_parity)
