#!/usr/bin/env python3
"""Sweep every Motavia formation: capture it, extract its fixture, record it.

    python3 oracle/sweep.py --out build/lane-evidence/sweep
    python3 oracle/sweep.py --out build/lane-evidence/sweep --jobs 3
    python3 oracle/sweep.py --out build/lane-evidence/sweep --only 0x05,0x06

This is the CLI entry point: it puts the repository root on `sys.path`, hands
its arguments to [`oracle.sweep`](sweep/__init__.py) - the package that holds
the list, the per-formation run and the record - and turns a refused request
into exit status 2. Re-running is the resume: a formation whose fixture is on
disk and whose recorded hash still matches is skipped, and `--force` runs them
all again. The record is `<out>/sweep_motavia.json`; the ledger of what this
sweep found is `docs/oracle/BATTLE_ORACLE_SWEEP.md`.
"""
import pathlib
import sys

if __package__ in (None, ""):
    # `python3 oracle/sweep.py` runs with `oracle/` on sys.path, and the
    # package is beside it, under the repository root.
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

from oracle.sweep import main


if __name__ == "__main__":
    sys.exit(main())
