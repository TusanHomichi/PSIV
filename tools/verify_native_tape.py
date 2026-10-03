"""Replay one campaign tape through Godot's ordinary input and compare a slot.

Run this entry point under the shared heavy flock. Tape parsing and its FNV
save-start check live in psiv-runtime's TapeFeed; this launcher does neither.
Every invocation creates a fresh save directory and retains raw logs/receipts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
GODOT = Path(os.environ.get("PSIV_GODOT", Path.home() / ".local/bin/psiv-godot-4.7.1"))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def parser() -> argparse.ArgumentParser:
    command = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    command.add_argument("--tape", type=Path, required=True)
    command.add_argument("--from-save", type=Path)
    command.add_argument("--expect-save", type=Path)
    command.add_argument("--expect-written-save", type=Path,
                         help="ordinary camp SAVE file to match in isolated PSIV_SAVE_DIR")
    command.add_argument("--expect-map", type=lambda value: int(value, 0))
    command.add_argument("--expect-cell", help="x,y in the endpoint's cell grid")
    command.add_argument("--pack", type=Path, default=ROOT / "runtime-pack")
    command.add_argument("--out", type=Path, required=True)
    command.add_argument("--timeout", type=int, default=600)
    command.add_argument("--render-capture", action="store_true")
    command.add_argument("--drop-pad-at", type=int, help="negative control: replace one byte with neutral")
    return command


def main() -> int:
    args = parser().parse_args()
    tape = args.tape.resolve(strict=True)
    source = args.from_save.resolve(strict=True) if args.from_save else None
    expected = args.expect_save.resolve(strict=True) if args.expect_save else None
    written = args.expect_written_save.resolve(strict=True) if args.expect_written_save else None
    pack = args.pack.resolve(strict=True)
    if not (pack / "manifest.json").is_file():
        parser().error(f"pack lacks manifest.json: {pack}")
    if not GODOT.is_file():
        parser().error(f"Godot executable absent: {GODOT}")
    if args.timeout <= 0:
        parser().error("--timeout must be positive")
    if args.expect_cell:
        parts = args.expect_cell.split(",")
        if len(parts) != 2 or any(not part.isdecimal() for part in parts):
            parser().error("--expect-cell is decimal x,y")
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    saves = out / "saves"
    saves.mkdir()
    native_report = out / "native.json"
    snapshot = out / "slot_1.snapshot.sram"
    run_log = out / "godot-run.log"
    godot_log = out / "godot.log"
    capture = out / "selected-1280x800.png"
    source_before = sha256(source) if source else None
    expected_sha = sha256(expected) if expected else None
    written_sha = sha256(written) if written else None
    # No inherited selector, autostart, retail-pace or load-slot override may
    # turn an ordinary-input replay into a debug fixture.
    removed_psiv_keys = sorted(key for key in os.environ if key.startswith("PSIV_"))
    env = {key: value for key, value in os.environ.items()
           if not key.startswith("PSIV_")}
    env.update({
        "PSIV_RUNTIME_PACK": str(pack),
        "PSIV_SAVE_DIR": str(saves),
        "PSIV_NATIVE_TAPE": "1",
        "PSIV_DEBUG_ROUTE": "1",  # endpoint observation only
        "PSIV_TAPE_FILE": str(tape),
        "PSIV_TAPE_SOURCE_SAVE": str(source) if source else "",
        "PSIV_TAPE_EXPECT_SAVE": str(expected) if expected else "",
        "PSIV_TAPE_EXPECT_MAP": str(args.expect_map) if args.expect_map is not None else "",
        "PSIV_TAPE_EXPECT_CELL": args.expect_cell or "",
        "PSIV_TAPE_REQUIRE_SAVE_SLOT": "0" if written else "",
        "PSIV_TAPE_REPORT": str(native_report),
        "PSIV_TAPE_SNAPSHOT_OUT": str(snapshot),
        "PSIV_TAPE_CAPTURE": str(capture) if args.render_capture else "",
        "LIBGL_ALWAYS_SOFTWARE": "1",
    })
    if args.drop_pad_at is not None:
        env["PSIV_TAPE_DROP_AT"] = str(args.drop_pad_at)
    # A fresh checkout has no Godot extension index. The editor's one-shot
    # scan creates it locally; no game or save is started by this step.
    extension_index = ROOT / "godot/.godot/extension_list.cfg"
    extension_entry = "res://psiv.gdextension"
    import_command = None
    import_code = None
    import_log = out / "godot-import.log"
    if not extension_index.is_file() or extension_entry not in extension_index.read_text():
        import_command = [str(GODOT), "--headless", "--path", "godot", "--editor", "--quit"]
        with import_log.open("w") as handle:
            try:
                import_code = subprocess.run(import_command, cwd=ROOT, env=env,
                                             stdout=handle, stderr=subprocess.STDOUT,
                                             timeout=args.timeout).returncode
            except subprocess.TimeoutExpired:
                import_code = 124
        if (import_code != 0 or not extension_index.is_file()
                or extension_entry not in extension_index.read_text()):
            print(f"native tape: Godot extension import failed; log={import_log}")
            return 1
    if args.render_capture:
        command = ["xvfb-run", "-a", str(GODOT), "--display-driver", "x11",
                   "--rendering-method", "gl_compatibility", "--rendering-driver", "opengl3",
                   "--audio-driver", "Dummy", "--fixed-fps", "60"]
    else:
        command = [str(GODOT), "--headless", "--disable-render-loop",
                   "--audio-driver", "Dummy", "--fixed-fps", "60"]
    command.extend(["--log-file", str(godot_log), "--path", "godot", "--script",
                    str(ROOT / "tools/native/native_tape.gd")])
    start = time.monotonic()
    with run_log.open("w") as handle:
        try:
            code = subprocess.run(command, cwd=ROOT, env=env, stdout=handle,
                                  stderr=subprocess.STDOUT, timeout=args.timeout).returncode
        except subprocess.TimeoutExpired:
            code = 124
    elapsed = time.monotonic() - start
    source_after = sha256(source) if source else None
    live_slot = saves / "slot_1.sram"
    result = json.loads(native_report.read_text()) if native_report.is_file() else None
    save_acks = result.get("camp_save_acks") if isinstance(result, dict) else None
    acknowledged = (isinstance(save_acks, list) and len(save_acks) == 3
                    and isinstance(save_acks[0], int) and save_acks[0] > 0)
    receipt = {
        "command": command, "exit_code": code, "elapsed_s": round(elapsed, 3),
        "effective_psiv_env": {key: env[key] for key in sorted(env) if key.startswith("PSIV_")},
        "removed_inherited_psiv_keys": removed_psiv_keys,
        "godot_import_command": import_command,
        "godot_import_exit_code": import_code,
        "godot_import_log": str(import_log) if import_command else None,
        "tape": str(tape), "tape_sha256": sha256(tape),
        "source_save": str(source) if source else None,
        "source_sha256_before": source_before, "source_sha256_after": source_after,
        "live_slot_sha256": sha256(live_slot) if live_slot.is_file() else None,
        "expected_save": str(expected) if expected else None,
        "expected_sha256": expected_sha,
        "expected_written_save": str(written) if written else None,
        "expected_written_sha256": written_sha,
        "ordinary_save_matches": bool(written and live_slot.is_file()
                                      and sha256(live_slot) == written_sha),
        "ordinary_save_acknowledged": acknowledged,
        "snapshot_sha256": sha256(snapshot) if snapshot.is_file() else None,
        "pack": str(pack), "pack_manifest_sha256": sha256(pack / "manifest.json"),
        "native_report": result,
        "godot_log": str(godot_log), "run_log": str(run_log),
        "capture": str(capture) if capture.is_file() else None,
    }
    receipt["pass"] = (
        code == 0 and result is not None and result.get("result") == "pass"
        and source_before == source_after
        and (source is None or (result.get("source_hash_verified_before_copy") is True
                                and result.get("source_copied") is True))
        and snapshot.is_file()
        and (expected is None or receipt["snapshot_sha256"] == expected_sha)
        and (written is None or (receipt["ordinary_save_acknowledged"]
                                and receipt["ordinary_save_matches"]))
    )
    receipt_path = out / "receipt.json"
    receipt_path.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    print(f"native tape: {'PASS' if receipt['pass'] else 'FAIL'}; "
          f"{elapsed:.2f}s; receipt={receipt_path}")
    return 0 if receipt["pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
