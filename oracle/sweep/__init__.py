"""The Motavia sweep: every formation, captured and extracted in one run.

    python3 -m oracle.sweep --out build/lane-evidence/sweep
    python3 -m oracle.sweep --out build/lane-evidence/sweep --jobs 3

`python3 -m oracle.force` captures *one* formation on request; this runs it over
a list of them - the sweep's own list is the distinct formation ids in
`generated/formation_indexes.json` groups 0-7 (Motavia on foot) and 8, 9, 10
(Motavia's vehicle tables), computed from the data rather than typed in - and
turns each capture into a fixture under
`rust/psiv-core/src/battle/replay_fixtures/sweep_motavia/`.

The shape of one formation's run, and the reason for each part:

* `--durable --max-rounds 5` - tape 07's party is level 1, so a formation whose
  enemies hit harder than 21 HP would end the fight inside round 1 and show
  only each enemy's first action. The durable patch (`oracle/force/durable.py`)
  keeps the party standing and the round cap ends the capture at a round
  boundary, so every capture is the same shape of evidence: up to five rounds
  of the cartridge's own actions against a known start state;
* **at most three captures in parallel** (the machine's memory cap - each
  capture is an emulator run plus a ~30MB RAM log, and the extractor parses
  that log in Python);
* **a formation that fails is recorded, not fatal.** The record names the
  stage (`capture` or `extract`), the exact command and the error, so the run
  accounts for every formation it was given;
* **the run resumes.** A formation already captured with its fixture on disk is
  skipped, which is what makes a sweep interruptible;
* one shared scout cache: the base tape's own run is 36k frames and every
  capture needs the same facts from it, so it is warmed once and read after.

`oracle/sweep_motavia.json` (or `--out`'s own name) is the record: per
formation the group, the selector, the capture's hashes, the outcome, the
rounds and the abilities observed, and the fixture's own hash - plus a summary
census. `docs/oracle/BATTLE_ORACLE_SWEEP.md` is where the divergences that record
produces are worked up.
"""
from .plan import (FOOT_GROUPS, MOTAVIA_GROUPS, VEHICLE_GROUPS, Formation,
                   list_formations, write_list)
from .jobs import Options, extractor_argv, force_argv, run_formation
from .batch import Sweep, record_path
from .cli import main

__all__ = [
    "FOOT_GROUPS", "MOTAVIA_GROUPS", "Options", "Sweep", "VEHICLE_GROUPS",
    "Formation", "extractor_argv", "force_argv", "list_formations",
    "main",
    "record_path", "run_formation", "write_list",
]
