#!/usr/bin/env python3
"""Turn a tape's RNG trace and RAM log into a replay fixture for psiv-core.

    python3 oracle/battle_fixture.py --trace build/tape07_rolls.csv \
                                     --log   build/tape07_battle.csv \
                                     --out   rust/psiv-core/src/battle/replay_fixtures/tape07_first_battle.json

This is the CLI; [`oracle/fixture/`](fixture/__init__.py) is the extractor, and
its package docstring is the account of the two inputs, the roll derivation,
what the log decides and what it does not, and the fixture's shape. Nothing in
this file decides anything about the battle: it parses the arguments, reads the
two CSVs, calls `oracle.fixture.build_fixture` and writes what comes back.
"""
import argparse
import json
import os
import pathlib
import sys

if __package__ in (None, ""):
    # `python3 oracle/battle_fixture.py` runs with `oracle/` on sys.path, and
    # the extractor is a package beside it, under the repository root.
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

from oracle.fixture import (BATTLE_FIRST, BATTLE_LAST, FixtureError,
                            build_fixture, compact_leaf_arrays, load_ram_map,
                            load_rows, Log, provenance_lines, sha256)


def default_ram_map():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)),
                        "ram_map.json")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--trace", required=True,
                        help="CSV from psiv_oracle --rng-trace")
    parser.add_argument("--log", required=True,
                        help="RAM log CSV from the same run")
    parser.add_argument("--out", required=True, help="fixture JSON to write")
    parser.add_argument("--ram-map", default=default_ram_map())
    parser.add_argument("--tape", default="oracle/tapes/07_first_battle.tape",
                        help="the tape the capture replayed; recorded by its "
                             "file name, so a fixture does not depend on "
                             "where the sweep kept it")
    parser.add_argument("--core", default="Genesis Plus GX 2d7131c "
                                         "(libretro/Genesis-Plus-GX)")
    parser.add_argument("--patch", default="oracle/patches/0001-rng-hv-trace.patch")
    parser.add_argument("--battle-first", type=int, default=BATTLE_FIRST)
    parser.add_argument("--battle-last", type=int, default=BATTLE_LAST)
    parser.add_argument("--max-rounds", type=int, default=0,
                        help="cap the extraction at this many rounds: the "
                             "fixture holds rounds 1..N and its outcome says "
                             "`truncated` (0 = every round the log holds)")
    parser.add_argument("--minified", action="store_true",
                        help="write the fixture as one line with no padding "
                             "spaces: a sweep's 83 fixtures are ~40%% smaller "
                             "that way, and the two hand-checked ones keep the "
                             "layout meant for reading")
    parser.add_argument("--hp-patch", type=int, default=0,
                        help="the HP the capture patched every living "
                             "fighter's two HP cells to (oracle/force/"
                             "durable.py); recorded in the provenance so a "
                             "replay knows the start state is the capture's")
    arguments = parser.parse_args(argv)

    trace_rows = load_rows(arguments.trace)
    log = Log(load_rows(arguments.log), load_ram_map(arguments.ram_map))
    # Every name a fixture records is the file's own, never the path this run
    # was given: the same capture extracted into another directory, or from
    # another directory, has to yield the same bytes (`oracle/fixture/logs.py`
    # `provenance_lines`, and `oracle/host/provenance.h` on the host's side).
    meta = {
        "tape": os.path.basename(arguments.tape),
        "core": arguments.core,
        "patch": arguments.patch,
        "trace": os.path.basename(arguments.trace),
        "trace_sha256": sha256(arguments.trace),
        "log_sha256": sha256(arguments.log),
        "trace_header": provenance_lines(arguments.trace),
        "log_header": provenance_lines(arguments.log),
    }
    fixture = build_fixture(trace_rows, log, load_ram_map(arguments.ram_map),
                            arguments.battle_first, arguments.battle_last, meta,
                            max_rounds=arguments.max_rounds,
                            hp_patch=arguments.hp_patch or None)
    with open(arguments.out, "w") as handle:
        if arguments.minified:
            handle.write(json.dumps(fixture, separators=(",", ":"),
                                    sort_keys=False))
        else:
            handle.write(compact_leaf_arrays(
                json.dumps(fixture, indent=1, sort_keys=False)))
        handle.write("\n")
    # Re-read what was written: the layout pass must not have changed the data.
    with open(arguments.out) as handle:
        if json.load(handle) != fixture:
            raise FixtureError(f"{arguments.out} does not read back as the "
                               f"fixture that was built")

    print(f"wrote {arguments.out}")
    print(f"  battle frames   {fixture['provenance']['battle_frames']}")
    print(f"  rounds          {len(fixture['rounds'])} "
          f"at {fixture['provenance']['round_frames']}")
    print(f"  rolls           {len(fixture['rolls']['rows'])} kept, "
          f"{len(fixture['outside_rolls']['rows'])} outside the battle, "
          f"{fixture['provenance']['roll_column']['agrees']} row(s) whose "
          f"roll column agrees with the cartridge's derivation")
    if fixture["provenance"]["vehicle_battle"]:
        vehicle = fixture["vehicle"]
        print(f"  vehicle         {vehicle['name']} (index "
              f"{vehicle['index']}), fighter id {vehicle['fighter_id']}, "
              f"{vehicle['hp']} HP at the battle's start")
    outcome = fixture["outcome"]
    print(f"  outcome         "
          + ("truncated at round " + str(outcome["rounds_captured"])
             if outcome["truncated"]
             else "victory" if outcome["victory"]
             else "defeat" if outcome["defeat"] else "the battle did not end")
          + f", {outcome['experience_total']} exp, {outcome['meseta']} meseta")
    for round_ in fixture["rounds"]:
        print(f"    round {round_['round']}: order {round_['order']} "
              f"({round_['roll_count']} rolls) "
              f"{len(round_['actions'])} action(s)")
        for action in round_["actions"]:
            shape = action["kind"]
            if shape != "attack":
                shape += f" ${action['ability']:02X}"
            print(f"      actor {action['actor']} at f{action['start_frame']}"
                  f"-{action['end_frame']}: {shape}, "
                  f"{action['roll_count']} roll(s), "
                  f"{len(action['targets'])} target(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
