"""Capture scene-load observations for the Rust psiv-scene-loads analyzer.

    nix develop -c python3 -m oracle.scene_loads --out build/scene-loads

Python owns fixture/host orchestration and provenance only. Rust verifies
ROM identities, selects watches, analyzes traces and validates every result.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import subprocess
import time
from pathlib import Path

from oracle import host_binary

ROOT = Path(__file__).resolve().parents[1]
ROM = ROOT / "Phantasy Star IV (USA).md"
CORE = ROOT / "oracle/core/genesis_plus_gx_libretro.so"


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_tool(arguments: list[str], out: Path, name: str) -> None:
    command = ["python3", "tools/heavy_lock.py", "run", "--class", "build", "--",
               "cargo", "run", "--quiet", "--manifest-path", "rust/Cargo.toml",
               "-p", "psiv-scene-loads", "--", *arguments]
    started = time.monotonic()
    print(f"{name}: progress in {out / (name + '.stderr')}", flush=True)
    with (out / f"{name}.stdout").open("wb") as output, (out / f"{name}.stderr").open("wb") as errors:
        result = subprocess.run(command, cwd=ROOT, stdout=output, stderr=errors)
    (out / f"{name}-receipt.json").write_text(json.dumps(
        {"command": command, "exit_status": result.returncode,
         "elapsed_seconds": time.monotonic() - started}, indent=2) + "\n")
    if result.returncode:
        raise RuntimeError(f"{name} exited {result.returncode}; see {out / (name + '.stderr')}")


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open() as handle:
        return list(csv.DictReader(line for line in handle if not line.startswith("#")))


def capture(scene: int, recipe: dict, out: Path, config: dict) -> None:
    directory = out / f"tape-{scene}"
    directory.mkdir()
    tape = directory / "input.tape"
    tape.write_bytes((ROOT / recipe["tape"]).read_bytes())
    ram_map = directory / "ram_map.tsv"
    ram_map.write_text((ROOT / "oracle/ram_map.tsv").read_text()
                       + "\npanel_index\tFFFFED00\t2\tscene_load\thex\n")
    trace, frames = directory / "pc.csv", directory / "frames.csv"
    command = [str(host_binary.ensure()), "--core", str(CORE), "--rom", str(ROM),
               "--map", str(ram_map), "--tape", str(tape), "--groups", "core,rng,scene_load",
               "--out", str(frames), "--pc-trace", str(trace), "--pc-trace-from", "7200"]
    for pc in config["watches"]:
        command += ["--pc-watch", f"{pc:06X}"]
    for patch in recipe["patches"]:
        command += ["--ram-patch", patch]
    inputs = {"rom": ROM, "core": CORE, "host": Path(command[0]),
              "ram_map": ram_map, "tape": tape, "certify_recipes": ROOT / "tools/certify.py"}
    sources = [ROOT / "oracle/scene_loads.py",
               ROOT / "rust/psiv-pack/src/presentation/load_work.rs",
               ROOT / "rust/psiv-pack/src/decode/nemesis.rs",
               ROOT / "rust/psiv-pack/src/decode/enigma.rs",
               *sorted((ROOT / "rust/psiv-scene-loads/src").rglob("*.rs"))]
    receipt = {"command": command, "cwd": str(ROOT),
               "input_sha256": {key: sha(path) for key, path in inputs.items()},
               "source_revision": subprocess.check_output(
                   ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
               "source_status": subprocess.check_output(
                   ["git", "status", "--porcelain"], cwd=ROOT, text=True),
               "measurement_source_sha256": {str(path.relative_to(ROOT)): sha(path) for path in sources},
               "measurement": "explicit_scene_fixture", **config}
    started = time.monotonic()
    with (directory / "stdout.log").open("wb") as output:
        result = subprocess.run(command, cwd=ROOT, stdout=output, stderr=subprocess.STDOUT)
    receipt.update(exit_status=result.returncode, elapsed_seconds=time.monotonic() - started)
    (directory / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    if result.returncode:
        raise RuntimeError(f"oracle exited {result.returncode}; see {directory / 'stdout.log'}")
    # Format adaptation only: the Rust program owns field parsing, routine
    # matching, frame/seed checks, coverage and decoded-stream attribution.
    (directory / "analysis-input.json").write_text(json.dumps(
        {"scene": scene, "trace": read_csv(trace), "frames": read_csv(frames)}) + "\n")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True, help="new evidence directory")
    parser.add_argument("--scenes", type=int, nargs="+", choices=(46, 43, 38), default=[46, 43])
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    run_tool(["verify", str(ROM)], out, "verify")
    config = json.loads((out / "verify.stdout").read_bytes())
    from tools.certify import TAPE_38, TAPE_43, TAPE_46
    recipes = {38: TAPE_38, 43: TAPE_43, 46: TAPE_46}
    for scene in args.scenes:
        capture(scene, recipes[scene], out, config)
    run_tool(["analyze", str(ROM), str(out), *map(str, args.scenes)], out, "analysis")
    print((out / "analysis.stdout").read_text(), end="")


if __name__ == "__main__":
    main()
