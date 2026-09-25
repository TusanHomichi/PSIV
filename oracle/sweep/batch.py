"""The batch: every formation, at most N captures at once, recorded as it goes.

The record is `sweep_motavia.json` and it is written after *every* formation, so
an interrupted sweep still says which formations are done and why the rest are
not. Resuming is the same command again: a formation whose fixture is on disk
and whose recorded hash still matches is skipped (`jobs.is_captured`).

Concurrency is `--jobs` threads, each running one formation's subprocesses
(`jobs.run_formation`), and it is 3 by default because that is what this
machine's memory allows: three emulator runs plus three Python log parses. The
first formation - or the first one after a `--force` - is run *before* the pool
starts, which is what makes the shared scout cache safe: the base tape's own
run is the one write to `work/scout.json` and every later formation only reads
it.
"""
from __future__ import annotations

import concurrent.futures
import json
import pathlib
import threading

from . import jobs
from .plan import Formation


def record_path(out: pathlib.Path) -> pathlib.Path:
    """`sweep_motavia.json`, beside the sweep's own working directory."""
    return out / "sweep_motavia.json"


class Sweep:
    """A sweep's state: the list, the record and how they are written."""

    def __init__(self, out: pathlib.Path, options: jobs.Options,
                 formations: list[Formation], source: dict):
        self.out = out
        self.options = options
        self.formations = formations
        self.source = source
        self.path = record_path(out)
        self.lock = threading.Lock()
        self.records: dict[int, dict] = {}
        self.load()

    def load(self) -> None:
        """The previous run's record, if there is one and it is this list's."""
        if not self.path.exists():
            return
        try:
            document = json.loads(self.path.read_text())
        except (OSError, ValueError):
            return
        for record in document.get("formations", []):
            if isinstance(record.get("formation"), int):
                self.records[record["formation"]] = record

    def pending(self, force: bool = False) -> list[Formation]:
        out = []
        for entry in self.formations:
            record = self.records.get(entry.formation)
            if not force and record and jobs.is_captured(record, self.options):
                continue
            out.append(entry)
        return out

    def run(self, jobs_count: int = 3, force: bool = False,
            limit: int = 0, only: set[int] | None = None) -> dict:
        """Run what is pending, then write the record back."""
        self.options.fixtures.mkdir(parents=True, exist_ok=True)
        todo = self.pending(force)
        if only:
            todo = [entry for entry in todo if entry.formation in only]
        if limit:
            todo = todo[:limit]
        skipped = len(self.formations) - len(self.pending(force))
        print(f"sweep: {len(self.formations)} formation(s) in the list, "
              f"{skipped} already captured, {len(todo)} to run")
        if todo:
            # Serial warm-up: the shared scout cache is written once, before
            # any thread reads it.
            self.one(todo[0])
            if len(todo) > 1:
                with concurrent.futures.ThreadPoolExecutor(
                        max_workers=jobs_count) as pool:
                    futures = [pool.submit(self.one, entry)
                               for entry in todo[1:]]
                    for future in concurrent.futures.as_completed(futures):
                        future.result()
        return self.write()

    def one(self, entry: Formation) -> dict:
        """One formation, recorded whatever it does."""
        record = jobs.run_formation(entry, self.options)
        with self.lock:
            self.records[entry.formation] = record
            self.write()
        print(f"  {entry.hex:>4} {record['status']:<8} "
              + (f"{record['outcome']}, {record['rounds']} round(s), "
                 f"abilities {record['abilities'] or 'none'}"
                 if record["status"] == "captured"
                 else f"{record['stage']} failed: {record['error']}"))
        return record

    def summary(self) -> dict:
        """The census the worklist's coverage section reads."""
        captured = [r for r in self.records.values()
                    if r.get("status") == "captured"]
        failed = [r for r in self.records.values()
                  if r.get("status") != "captured"]
        enemies, abilities, outcomes = {}, {}, {}
        for record in captured:
            for entry in record.get("enemies", []):
                enemies[entry["id"]] = enemies.get(entry["id"], 0) + 1
            for key in record.get("abilities", {}):
                abilities[key] = abilities.get(key, 0) + 1
            outcomes[record["outcome"]] = outcomes.get(record["outcome"], 0) + 1
        return {
            "formations": len(self.formations),
            "recorded": len(self.records),
            "captured": len(captured),
            "failed": len(failed),
            "truncated": sum(1 for r in captured if r.get("truncated")),
            "outcomes": dict(sorted(outcomes.items())),
            "enemies_seen": dict(sorted(enemies.items())),
            "abilities_seen": dict(sorted(abilities.items())),
            "failures": [{"formation": r["formation"], "stage": r["stage"],
                          "error": r["error"]} for r in failed],
        }

    def document(self) -> dict:
        return {
            "generated_by": "oracle/sweep/__main__.py",
            "source": self.source,
            "options": {
                "base_tape": str(self.options.base_tape),
                "policy": self.options.policy,
                "repeats": self.options.repeats,
                "max_rounds": self.options.max_rounds,
                "durable": self.options.durable,
                "fixtures": jobs.relative(self.options.fixtures),
            },
            "formations": [self.records[entry.formation]
                           for entry in self.formations
                           if entry.formation in self.records],
            "summary": self.summary(),
        }

    def write(self) -> dict:
        """The record as it stands: what happened to each formation so far."""
        document = self.document()
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(json.dumps(document, indent=1) + "\n")
        return document
