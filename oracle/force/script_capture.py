"""Observe menus through a read-only pilot, then freeze an ordinary-input tape.

The host has a finite tape interface. A pilot reads RAM at pad-block boundaries
after the measured formation draw, and supplies joypad inputs. Nothing writes a
command, cursor, technique list or target to RAM. The frozen tape goes through
the existing preview/capture/verify phases and RNG checker unchanged.
"""
from __future__ import annotations

import json
import pathlib
import os
import shutil
import subprocess
import sys

from . import runs, script_menu
from .errors import ForceError
from .script import Script, definitions, party_state, patch_state
from .tape import Step, emit_tape, tape_frames, trim_tape

def prepare(args, facts: dict, base_steps: list[Step]) -> None:
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    frame = facts["battle_first"]
    tape = out / "script-state.tape"
    tape.write_text(emit_tape(trim_tape(base_steps, frame)))
    state = (out / "script-state.bin").resolve()
    run = runs.run_oracle(tape, out / "script-state", "script-state", [],
                          dump_ram=(frame, state))
    if run.status:
        raise ForceError(f"script-state observation failed: {run.stderr}")
    party = party_state(state.read_bytes())
    facts["script_state"] = {"path": str(state), "sha256": runs.sha256(state),
                             "frame": frame, "party": party}
    cache = pathlib.Path(args.scout) if args.scout else out / "scout.json"
    cache.write_text(json.dumps(facts, indent=2, sort_keys=True) + "\n")
    print(f"script state: f{frame}, fighters {sorted(party)}, {state}")
    for slot, member in party.items():
        print(f"  fighter {slot}: tech {member['technique']}, skill {member['skill']}")



def build(plan: dict, args, specs: list[str], script: Script) -> None:
    """Compile the stock host plus a read-only pad hook, then freeze its inputs."""
    out = plan["out"] / "script-pilot"
    sources = out / "host"
    sources.mkdir(parents=True, exist_ok=True)
    for source in (runs.ORACLE / "host").iterdir():
        if source.suffix in (".c", ".h"):
            name = "stock_host.c" if source.name == "psiv_oracle.c" else source.name
            shutil.copyfile(source, sources / name)
    shutil.copyfile(pathlib.Path(__file__).with_name("pilot_host.c"), sources / "psiv_oracle.c")
    binary = out / "pilot-host"
    built = subprocess.run([sys.executable, "-m", "oracle.build_host", "--host-dir", str(sources),
                            "--binary", str(binary)], capture_output=True, text=True)
    if built.returncode:
        raise ForceError(f"pilot host build failed: {built.stderr}")
    (out / "host-build.txt").write_text(built.stdout + built.stderr)
    cache = pathlib.Path(args.scout) if args.scout else plan["out"] / "scout.json"
    facts = json.loads(cache.read_text())
    observed = pathlib.Path(facts["script_state"]["path"]).read_bytes()
    party = party_state(patch_state(observed, args.ram_patch, facts["battle_first"] + 1))
    ram_map = script_menu.write_map(runs.DEFAULT_RAM_MAP_TSV, plan["out"] / "script-map.tsv", party)
    plan["run_options"] = {"ram_map": ram_map, "groups": runs.GROUPS + ",menu"}
    steps = trim_tape(plan["steps"], plan["script_start"])
    start = tape_frames(steps)
    budget = args.repeats * 16
    tape = out / "pilot.tape"
    tape.write_text(emit_tape(steps + [Step(budget, ".")], plan["header"]))
    argv = [str(binary), "--core", str(runs.CORE), "--rom", str(runs.ROM),
            "--map", str(ram_map), "--tape", str(tape), "--groups", runs.GROUPS + ",menu",
            "--out", str(out / "pilot.csv")]
    for spec in specs:
        argv += ["--ram-patch", spec]
    env = dict(os.environ, PSIV_FORCE_START_FRAME=str(start))
    receipt, round_count, player_round = [], 0, -1
    records = definitions(pathlib.Path(args.runtime_pack))
    queue = None
    new_phase = True
    last_frame = start
    last_row = {}
    layout = [(line.split("\t")[0], int(line.split("\t")[1], 16) & 0xFFFF,
               int(line.split("\t")[2]), line.split("\t")[4].strip() == "hex")
              for line in ram_map.read_text().splitlines() if line and not line.startswith("#")]
    stderr = (out / "pilot-stderr.txt").open("w")
    proc = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=stderr, text=True, env=env)
    try:
        while True:
            line = proc.stdout.readline()
            if not line:
                status = proc.wait()
                detail = (f"f{last_frame}, {round_count} queue(s), "
                          f"routine ${last_row.get('battle_routine_2', '?')}, "
                          f"command ${last_row.get('current_command', '?')}")
                raise ForceError(f"pilot host stopped before script boundary: {status}; {detail}; "
                                 "inspect pilot.csv and pilot-stderr.txt for a stalled animation/menu")
            frame, raw = line.split()
            frame, raw = int(frame), bytes.fromhex(raw)
            row = {name: (f"{int.from_bytes(raw[address:address + size], 'big'):0{size * 2}X}"
                          if hex_ else str(int.from_bytes(raw[address:address + size], "big")))
                   for name, address, size, hex_ in layout}
            last_frame, last_row = frame, row
            current_queue = tuple(row[f"turn_{i:02d}"] for i in range(18))
            ids = [int(value, 16) for value in current_queue[::2]]
            if any(ids) and all(0 <= value <= 9 for value in ids):
                if current_queue != queue:
                    round_count += 1
                    if (not args.max_rounds or round_count <= args.max_rounds) \
                            and any(1 <= fighter <= 5 for fighter in ids):
                        script_menu.verify_commands(raw, script.round(player_round), records, ids)
                queue = current_queue
            if (args.max_rounds and round_count > args.max_rounds) or row["game_mode"] not in ("0010", "0014"):
                proc.stdin.write("0 0\n")
                proc.stdin.flush()
                break
            if frame - start >= budget:
                raise ForceError("script frame budget exhausted; increase --repeats")
            routine = int(row["battle_routine_2"], 16)
            if routine == 2:
                new_phase = True
                buttons = script_menu.horizontal(int(row["battle_main_option"]), 0, 3)
                buttons = ["D" if b == "R" else "U" for b in buttons] + ["C"]
            elif routine in (5, 8, 0x11, 0x17, 0xC, 0xD):
                if new_phase:
                    if routine != 5:
                        raise ForceError("script reached a list/target before a character menu")
                    player_round += 1
                    new_phase = False
                fighter = int(row["battle_total_comd"]) + 1
                # Finish the capped boundary by building (but never executing)
                # the next queue. A finite script needs no extra command round
                # merely to close its final captured turn.
                command = script.command(fighter, player_round, round_count, args.max_rounds)
                buttons = script_menu.command_buttons(row, command)
            elif routine in script_menu.READY or script_menu.confirmation_ready(routine):
                buttons = ["C"]
            else:
                buttons = []
            if buttons:
                receipt.append({"frame": frame, "routine": routine,
                                "player_round": player_round + 1,
                                "fighter": int(row["battle_total_comd"]) + 1,
                                "buttons": buttons})
                print(f"script: f{frame} menu ${routine:02X} player-round {player_round + 1}: "
                      f"{' '.join(buttons)}", flush=True)
            block = script_menu.presses(buttons) if buttons else [Step(4, ".")]
            for index, step in enumerate(block):
                mask = {".": 0, "U": 1, "D": 2, "L": 4, "R": 8, "C": 64}[step.buttons]
                proc.stdin.write(f"{step.frames} {mask}\n")
                proc.stdin.flush()
                steps.append(step)
                if index < len(block) - 1:
                    # Every block is acknowledged at its boundary. Only the
                    # last snapshot is needed to choose the next menu command.
                    if not proc.stdout.readline():
                        raise ForceError("pilot stopped inside an input block")
        status = proc.wait()
        if status:
            raise ForceError(f"pilot host failed (exit {status})")
    finally:
        if proc.poll() is None:
            proc.terminate()
            proc.wait()
        stderr.close()
        (plan["out"] / "script-inputs.json").write_text(json.dumps(receipt, indent=2) + "\n")
    plan["steps"] = steps
    plan["full_tape"].write_text(emit_tape(steps, plan["header"]))
    print(f"script frozen: {len(receipt)} menu decisions, {tape_frames(steps)} frames")
