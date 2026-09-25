#!/usr/bin/env python3
"""Sweep every Motavia formation: capture it, extract its fixture, record it.

    python3 -m oracle.sweep --out build/lane-evidence/sweep
    python3 -m oracle.sweep --out build/lane-evidence/sweep --jobs 3
    python3 -m oracle.sweep --out build/lane-evidence/sweep --only 0x05,0x06

The entry point of [`oracle.sweep`](__init__.py) - the package that holds the
list, the per-formation run and the record - run from the repository root. It
hands its arguments to that package's `main` and turns a refused request into
exit status 2. Re-running is the resume: a formation whose fixture is on disk
and whose recorded hash still matches is skipped, and `--force` runs them all
again. The record is `<out>/sweep_motavia.json`; the ledger of what this sweep
found is `docs/oracle/BATTLE_ORACLE_SWEEP.md`.
"""
import sys

from oracle.sweep import main


if __name__ == "__main__":
    sys.exit(main())
