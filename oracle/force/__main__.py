#!/usr/bin/env python3
"""Force a chosen formation into a battle and capture it with the oracle.

    python3 -m oracle.force --formation 0x5E --out build/forced/helex
    python3 -m oracle.force --formation 0x53 --out build/forced/icedigger \
        --vehicle 2

The entry point of [`oracle.force`](__init__.py) - the package that holds the
mechanism, the citations and the phases - run from the repository root. It
hands its arguments to that package's `main` and turns a refused request into
exit status 2. The package docstring is the account of what a capture is; the
ledger is `docs/oracle/BATTLE_ORACLE_FORCED.md`.
"""
import sys

from oracle.force import ForceError, main


if __name__ == "__main__":
    try:
        sys.exit(main())
    except ForceError as error:
        print(f"force_battle: {error}", file=sys.stderr)
        sys.exit(2)
