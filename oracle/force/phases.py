"""The five runs a capture is: plan, probe, preview, capture, verify.

* **plan** - compose the tape (the base tape's field prefix, an optional delay,
  the policy), write it and the selector-only patch list.
* **probe** - force the group and leave the seed alone: its log must show the
  formation the group table's own entry names, its trace says where the draw is
  and what `hv + frame_count` was, and its log has to prove that nothing moves
  the seed at the frame the seed patch will land on.
* **preview** - the untrimmed run with the seed patch: the battle's window, its
  outcome and the ability ids the capture exists for. It may outlive the fight
  into the game-over sequence, so its exit status is reported, not required.
* **capture** and **verify** - the same tape and patches, trimmed just past the
  battle, in two output directories: the evidence, byte-compared.

Nothing here decides what the cartridge does; every phase is one
`oracle/bin/psiv_oracle` run and a reading of its two CSVs, and the checks are
the ones the ledger cites.
"""
from __future__ import annotations

import json
import pathlib
import subprocess
import sys

from . import runs
from .capture import Capture, enemy_slots, matches, read_capture
from .draw import Draw, find_draw, patch_specs
from .errors import ForceError
from .pack import Pack, describe, field_layout, parse_formation
from .runs import ORACLE, battle_window, by_frame, read_rows, seed_of, sha256
from .scout import scout
from .selectors import Selector, choose_selector
from .tape import (PROBE_REPEATS, TAIL_FRAMES, Step, compose, emit_tape,
                   expand_tape, tape_frames, trim_tape)


def plan(args, pack: Pack, layout: dict, selector: Selector, formation: int,
         facts: dict, base_steps: list[Step]) -> dict:
    """The composed tape, its header and the selector-only patch list."""
    cut = facts["battle_first"]
    stem = f"forced_{formation:02X}_{args.policy}" \
        + (f"_d{args.delay}" if args.delay else "") \
        + (f"_v{args.vehicle}" if args.vehicle else "")
    header = (
        f"# Forced battle capture written by oracle/force_battle.py\n"
        f"# {describe(pack, formation)}\n"
        f"# group {selector.group} ({selector.kind}) forced via "
        f"{selector.label}\n"
        f"# base tape {args.base_tape}, frames 1..{cut}: its encounter fires at "
        f"f{cut}, the formation is drawn a few frames later\n"
        f"# policy {args.policy}"
        + (f", delay {args.delay} idle frames at the seam" if args.delay else "")
        + f", {args.repeats} block(s)\n")
    steps = compose(base_steps, cut, args.delay, args.repeats, args.policy)
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    tape = out / f"{stem}.tape"
    full_tape = out / f"{stem}.full.tape"
    full_tape.write_text(emit_tape(steps, header))
    return {"cut": cut, "stem": stem, "header": header, "steps": steps,
            "out": out, "tape": tape, "full_tape": full_tape,
            "selector_specs": patch_specs(selector, facts, layout)}


def probe_phase(plan_facts: dict, args, selector: Selector, pack: Pack,
                base_steps: list[Step]) -> Draw:
    """Run the probe and hand back the draw, with the model's own checks.

    The probe forces the group and leaves the seed alone, so its log says which
    formation the group's draw produced - which must be the group table's entry
    - and its trace says where the draw is and what `hv + frame_count` was.
    """
    out, stem = plan_facts["out"], plan_facts["stem"]
    steps = compose(base_steps, plan_facts["cut"], args.delay,
                    min(args.repeats, PROBE_REPEATS), args.policy)
    tape = out / f"{stem}.probe.tape"
    tape.write_text(emit_tape(steps, plan_facts["header"]))
    run = runs.run_oracle(tape, out / "probe", stem, plan_facts["selector_specs"])
    if run.status != 0:
        raise ForceError(f"the probe run failed (exit {run.status}):\n"
                         f"{run.stderr.strip()}")
    rows = read_rows(run.log)
    window = battle_window(rows)
    if window is None:
        raise ForceError("the probe never entered a battle: the forced group "
                         "patch did not survive to the load")
    draw = find_draw(read_rows(run.trace), rows, plan_facts["cut"])
    drawn = pack.group_entries(selector.group)[draw.index]
    print(f"probe: the formation is drawn at f{draw.frame} "
          f"({draw.rows_in_frame} call(s) that frame), roll ${draw.roll:04X} "
          f"& 31 = {draw.index} -> formation {drawn}, K = {draw.k}")
    if tape_frames(steps) < draw.frame:
        raise ForceError(f"the probe tape ({tape_frames(steps)} frames) does "
                         f"not reach the draw at f{draw.frame}")
    built = enemy_slots(rows, window)
    if built != pack.enemies_of(drawn):
        raise ForceError(
            f"the probe built {built}, but group {selector.group}"
            f"[{draw.index}] is formation {drawn} = {pack.enemies_of(drawn)}: "
            "the forcing model does not explain this run")
    # The seed patch must land where nothing else moves the seed, or the trace
    # it produces cannot be checked.
    before = by_frame(rows).get(draw.frame_before)
    if before is None or seed_of(before) != draw.seed_before:
        raise ForceError(
            f"the seed at the draw (${draw.seed_before:08X}) is not the seed "
            f"the probe log holds at f{draw.frame_before} "
            f"(${seed_of(before) if before else 0:08X}): a one-frame-earlier "
            "patch would not fix the draw")
    if any(int(row["frame"]) == draw.frame_before
           for row in read_rows(run.trace)):
        raise ForceError(
            f"f{draw.frame_before} has an UpdateRNGSeed2 call of its own, so "
            "the seed patch would break the trace's chain")
    return draw


def capture_phase(plan_facts: dict, specs: list[str], draw: Draw, pack: Pack,
                  selector: Selector, formation: int) -> tuple[Capture, int]:
    """Preview, trim, capture and re-run; hand back the capture and the trim.

    The preview is the untrimmed run: it says when the battle ended, which is
    what the trim needs, and it may walk into the game-over sequence after a
    defeat (its exit status is reported, not required). The capture and its
    re-run are the evidence, and the tool has already proved their inputs are
    identical apart from their output directory.
    """
    out, stem, tape = plan_facts["out"], plan_facts["stem"], plan_facts["tape"]
    preview = runs.run_oracle(plan_facts["full_tape"], out / "preview", stem,
                             specs)
    if preview.status != 0:
        print(f"note: the untrimmed preview run ended with exit "
              f"{preview.status} (post-battle game-over sequence); the trimmed "
              "capture is the evidence", file=sys.stderr)
    preview_capture = read_capture(preview, draw.frame,
                                  selector.kind == "vehicle")
    window = preview_capture.window
    if not matches(preview_capture, pack, formation):
        raise ForceError(
            f"the capture built {preview_capture.enemies}, not formation "
            f"{formation} ({pack.enemies_of(formation)}); the seed patch missed")
    print(f"capture: f{window[0]}-{window[1]}, outcome "
          f"{preview_capture.outcome}, enemies "
          f"{[(e['slot'], e['id'], e['maxhp']) for e in preview_capture.enemies]}")
    print("         abilities used: " + (", ".join(
        f"{key} at f{value}" for key, value in
        sorted(preview_capture.abilities.items())) or "none"))
    if preview_capture.outcome == "unfinished":
        raise ForceError("the battle had not ended when the tape ran out; "
                         "re-run with more --repeats")

    trim_to = min(tape_frames(plan_facts["steps"]), window[1] + TAIL_FRAMES)
    tape.write_text(emit_tape(trim_tape(plan_facts["steps"], trim_to),
                              plan_facts["header"]))
    capture = runs.run_oracle(tape, out / "capture", stem, specs)
    rerun = runs.run_oracle(tape, out / "verify", stem, specs)
    for label, run in (("capture", capture), ("verify", rerun)):
        if run.status != 0:
            raise ForceError(f"the {label} run failed (exit {run.status}):\n"
                             f"{run.stderr.strip()}")
    final = read_capture(capture, draw.frame, selector.kind == "vehicle")
    if final.window != window:
        raise ForceError(f"trimming the tape moved the battle, {window} -> "
                         f"{final.window}")
    if not matches(final, pack, formation):
        raise ForceError(f"the trimmed capture built {final.enemies}, not "
                         f"{pack.enemies_of(formation)}")
    if final.log_sha256 != sha256(rerun.log) \
            or final.trace_sha256 != sha256(rerun.trace):
        raise ForceError("two runs of the same tape and patches differ")
    return final, preview.status


def build_report(args, pack: Pack, selector: Selector, formation: int,
                 forced_index: int, facts: dict, plan_facts: dict, draw: Draw,
                 specs: list[str], final: Capture, preview_status: int,
                 cut: int, missing: list[str]) -> dict:
    """Everything a reader needs to re-run the capture and judge it."""
    entries = pack.entries_for(formation)
    return {
        "formation": formation,
        "formation_hex": f"0x{formation:02X}",
        "formation_enemies": pack.enemies_of(formation),
        "formation_ability_ids": sorted(
            {value for entry in pack.enemies_of(formation)
             for value in pack.ability_ids(entry["id"])}),
        "action": describe(pack, formation),
        "policy": args.policy,
        "delay": args.delay,
        "repeats": args.repeats,
        "base_tape": str(args.base_tape),
        "base_battle_first": cut,
        "selector": {"group": selector.group, "kind": selector.kind,
                     "label": selector.label, "entry": forced_index,
                     "entries": entries[selector.group],
                     "cells": selector.cells, "restore": selector.restore,
                     "cells_before": facts["cells"]},
        "vehicle": {"index": selector.vehicle, "name": selector.vehicle_name,
                    "table": selector.group,
                    "fighter_hp_at_end": final.vehicle_hp}
        if selector.vehicle else None,
        "draw": {"frame": draw.frame, "hv": f"0x{draw.hv:04X}",
                 "frame_count": draw.frame_count,
                 "seed_before": f"0x{draw.seed_before:08X}",
                 "roll": f"0x{draw.roll:04X}", "index_before": draw.index,
                 "index_forced": forced_index, "k": draw.k},
        "patches": specs,
        "tape": str(plan_facts["tape"]),
        "tape_frames": tape_frames(expand_tape(plan_facts["tape"].read_text())),
        "battle_first": final.window[0], "battle_last": final.window[1],
        "outcome": final.outcome, "enemies": final.enemies,
        "party": final.party, "vehicle_fighter_hp": final.vehicle_hp,
        "abilities": final.abilities, "rewards": final.rewards,
        "trace": str(final.trace_path), "trace_sha256": final.trace_sha256,
        "log": str(final.log_path), "log_sha256": final.log_sha256,
        "rerun_log": str(pathlib.Path(final.log_path).parent.parent
                         / "verify" / pathlib.Path(final.log_path).name),
        "rerun_log_sha256": final.log_sha256,
        "preview_status": preview_status,
        "rng_trace_check": "passed",
        "require_ability": [f"0x{value:02X}" for value in args.require_ability],
        "require_ability_met": not missing,
    }


def run(args) -> int:
    """The whole capture: scout, plan, probe, preview, capture, verify."""
    out = pathlib.Path(args.out)
    layout = field_layout(pathlib.Path(args.ram_map))
    pack = Pack.load(pathlib.Path(args.data_dir))
    formation = parse_formation(args.formation, pack)
    selector = choose_selector(pack, formation, args.vehicle)
    if args.delay < 0 or args.repeats < 1:
        raise ForceError("--delay must be >= 0 and --repeats >= 1")
    base_tape = pathlib.Path(args.base_tape)
    try:
        base_text = base_tape.read_text()
    except OSError as error:
        raise ForceError(str(error)) from error
    base_steps = expand_tape(base_text)
    forced_index = pack.entries_for(formation)[selector.group][0]
    facts = scout(base_tape, base_text, out,
                  pathlib.Path(args.scout) if args.scout else out / "scout.json",
                  args.refresh_scout, args.dry_run, layout)
    plan_facts = plan(args, pack, layout, selector, formation, facts, base_steps)

    print(describe(pack, formation))
    print(f"group {selector.group} via {selector.label}; entry {forced_index} of"
          f" {len(pack.group_entries(selector.group))}")
    print(f"base tape {base_tape}: first battle at f{plan_facts['cut']}, so the "
          f"tape is {tape_frames(plan_facts['steps'])} frames ({args.policy}"
          + (f", delay {args.delay}" if args.delay else "") + ")")
    if selector.kind == "vehicle":
        print("note: the vehicle tables are only reachable with Vehicle_Index "
              "set, so the vehicle fights this battle alone (loc_78EE, "
              "ps4.asm:11408)")
    stem = plan_facts["stem"]
    if args.dry_run:
        patches = plan_facts["out"] / f"{stem}.patches.txt"
        patches.write_text(
            "# selector patches only: --dry-run stops before the probe that "
            "measures\nthe draw frame the seed patch is placed against\n"
            + "\n".join(plan_facts["selector_specs"]) + "\n")
        print(f"dry run: wrote {plan_facts['full_tape']} and {patches}")
        print(f"  selector patches: "
              f"{' '.join(plan_facts['selector_specs'])}")
        return 0

    draw = probe_phase(plan_facts, args, selector, pack, base_steps)
    specs = patch_specs(selector, facts, layout, draw, forced_index)
    (plan_facts["out"] / f"{stem}.patches.txt").write_text(
        f"# --ram-patch list for {stem}\n"
        f"# f{facts['battle_first'] + 1}: the group selector, one frame after "
        f"the encounter fires\n"
        f"# f{draw.frame_before}: RNG_Seed's high word, one frame before the "
        f"formation draw (K = {draw.k}, entry {forced_index})\n"
        f"# f{draw.frame + 1}: the selector cells written back\n"
        + "\n".join(specs) + "\n")
    print("patches: " + "  ".join(specs))

    final, preview_status = capture_phase(plan_facts, specs, draw, pack,
                                         selector, formation)
    checked = subprocess.run(
        [sys.executable, str(ORACLE / "rng_trace.py"), "check",
         str(final.trace_path), str(final.log_path)],
        capture_output=True, text=True)
    sys.stdout.write(checked.stdout)
    if checked.returncode != 0:
        sys.stderr.write(checked.stderr)
        print("force_battle: rng_trace.py check failed", file=sys.stderr)
        return 1

    missing = [f"0x{value:02X}" for value in args.require_ability
               if not any(key.endswith(f"=0x{value:02X}")
                          for key in final.abilities)]
    report = build_report(args, pack, selector, formation, forced_index, facts,
                          plan_facts, draw, specs, final, preview_status,
                          plan_facts["cut"], missing)
    (plan_facts["out"] / "report.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"capture: f{report['battle_first']}-{report['battle_last']}, "
          f"outcome {report['outcome']}, {report['tape_frames']} tape frames, "
          "two runs byte-identical")
    print(f"  trace {report['trace_sha256']}")
    print(f"  log   {report['log_sha256']}")
    print(f"  report {plan_facts['out'] / 'report.json'}")
    if missing:
        print(f"force_battle: required abilit"
              + ("y " if len(missing) == 1 else "ies ")
              + f"{', '.join(missing)} never fired; try another --delay",
              file=sys.stderr)
        return 1
    return 0
