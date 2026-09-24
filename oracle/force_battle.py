#!/usr/bin/env python3
"""Force a chosen formation into a battle and capture it with the oracle.

    oracle/force_battle.py --formation 0x5E --out build/forced/helex
    oracle/force_battle.py --formation 0x53 --out build/forced/icedigger \
        --vehicle 2

This is the CLI entry point: it puts the repository root on `sys.path`, hands
its arguments to [`oracle.force`](force/__init__.py) - the package that holds
the mechanism, the citations and the phases - and turns a refused request into
exit status 2. The package docstring is the account of what a capture is; the
ledger is `docs/BATTLE_ORACLE_FORCED.md`.
"""
import pathlib
import sys

if __package__ in (None, ""):
    # `python3 oracle/force_battle.py` runs with `oracle/` on sys.path, and the
    # package is beside it, under the repository root.
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

from oracle.force import ForceError, main


if __name__ == "__main__":
    try:
        sys.exit(main())
    except ForceError as error:
        print(f"force_battle: {error}", file=sys.stderr)
        sys.exit(2)
