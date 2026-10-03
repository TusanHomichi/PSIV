"""Replay one campaign tape through Godot's ordinary input and compare a slot.

Run this entry point under the shared heavy flock. Tape parsing and its FNV
save-start check live in psiv-runtime's TapeFeed; this launcher does neither.
Every invocation creates a fresh save directory and retains raw logs/receipts.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
GODOT = Path(os.environ.get("PSIV_GODOT", Path.home() / ".local/bin/psiv-godot-4.7.1"))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def file_identity(path: Path) -> dict:
    path = path.resolve(strict=True)
    stat = path.stat()
    return {"path": str(path), "sha256": sha256(path), "device": stat.st_dev,
            "inode": stat.st_ino, "size": stat.st_size,
            "mtime_ns": stat.st_mtime_ns, "ctime_ns": stat.st_ctime_ns}


def untracked_identity(path: Path) -> dict:
    """Identify a Git-untracked entry without traversing a linked asset tree."""
    info = path.lstat()
    identity = {"mode": info.st_mode, "device": info.st_dev, "inode": info.st_ino,
                "size": info.st_size, "mtime_ns": info.st_mtime_ns,
                "ctime_ns": info.st_ctime_ns}
    if stat.S_ISLNK(info.st_mode):
        identity["kind"] = "symlink"
        identity["link_sha256"] = hashlib.sha256(os.fsencode(os.readlink(path))).hexdigest()
    elif stat.S_ISREG(info.st_mode):
        identity["kind"] = "file"
        identity["sha256"] = sha256(path)
    else:
        identity["kind"] = "special"
    return identity


def source_identity(root: Path = ROOT) -> dict:
    def git(*args: str) -> bytes:
        return subprocess.check_output(["git", *args], cwd=root)

    status = git("status", "--porcelain=v1", "--untracked-files=all").decode().splitlines()
    untracked = git("ls-files", "--others", "--exclude-standard", "-z").split(b"\0")
    untracked_entries = {os.fsdecode(name): untracked_identity(root / os.fsdecode(name))
                         for name in untracked if name}
    return {"head": git("rev-parse", "HEAD").decode().strip(),
            "dirty_paths": status,
            "diff_sha256": hashlib.sha256(git("diff", "--no-ext-diff", "--binary", "HEAD")).hexdigest(),
            "untracked_entries": untracked_entries}


def terminate_process_group(process: subprocess.Popen, grace_s: float = 2.0) -> None:
    """Stop only this invocation's new session, including xvfb-run children."""
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    # Keep the group leader unreaped until after SIGKILL, so its PGID cannot
    # be reused while an uncooperative Godot/Xvfb child remains in the group.
    time.sleep(grace_s)
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait()


def mapped_extension(pid: int, library: Path) -> dict | None:
    """Observe the actual Linux process map while Godot has the extension loaded."""
    try:
        lines = Path(f"/proc/{pid}/maps").read_text().splitlines()
    except OSError:
        return None
    for line in lines:
        fields = line.split(maxsplit=5)
        if len(fields) == 6 and fields[5] == str(library):
            return {"pid": pid, "path": fields[5], "device_in_proc_maps": fields[3],
                    "inode": int(fields[4]), "segment": fields[0],
                    "observed_at_utc": datetime.now(timezone.utc).isoformat()}
    return None


def mapped_extension_in_process_tree(pid: int, library: Path) -> dict | None:
    """`xvfb-run` is a shell parent; Godot loads the library in its child."""
    seen = set()
    pending = [pid]
    while pending:
        current = pending.pop()
        if current in seen:
            continue
        seen.add(current)
        mapped = mapped_extension(current, library)
        if mapped is not None:
            return mapped
        try:
            children = Path(f"/proc/{current}/task/{current}/children").read_text()
        except OSError:
            continue
        pending.extend(int(child) for child in children.split())
    return None


def identities_stable(code_before: dict, code_after: dict, git_before: dict,
                      git_after: dict, pack_before: str, pack_after: str,
                      tape_before: str, tape_after: str) -> bool:
    return (code_before == code_after and git_before == git_after
            and pack_before == pack_after and tape_before == tape_after)


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
    library = ROOT / "rust/target/debug/libpsiv_godot.so"
    if not library.is_file():
        parser().error(f"native extension is missing: {library}; build it before replay")
    code_paths = {"extension": library, "driver": ROOT / "tools/native/native_tape.gd",
                  "verifier": Path(__file__), "godot": GODOT}
    code_before = {name: file_identity(path) for name, path in code_paths.items()}
    git_before = source_identity()
    pack_manifest_before = sha256(pack / "manifest.json")
    tape_sha_before = sha256(tape)
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
    started_at_utc = datetime.now(timezone.utc).isoformat()
    mapped = None
    with run_log.open("w") as handle:
        process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=handle,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        deadline = start + args.timeout
        # A positive receipt must bind the code on disk to the library mapped
        # by this Godot process, not just to a later file with the same name.
        while process.poll() is None and time.monotonic() < deadline:
            mapped = mapped_extension_in_process_tree(process.pid, library.resolve())
            if mapped is not None:
                break
            time.sleep(0.02)
        try:
            code = process.wait(timeout=max(0, deadline - time.monotonic()))
        except subprocess.TimeoutExpired:
            terminate_process_group(process)
            code = 124
    elapsed = time.monotonic() - start
    ended_at_utc = datetime.now(timezone.utc).isoformat()
    code_after = {name: file_identity(path) for name, path in code_paths.items()}
    git_after = source_identity()
    pack_manifest_after = sha256(pack / "manifest.json")
    tape_sha_after = sha256(tape)
    identity_stable = identities_stable(code_before, code_after, git_before,
                                        git_after, pack_manifest_before,
                                        pack_manifest_after, tape_sha_before,
                                        tape_sha_after)
    # /proc/maps may report the underlying mount's device number while stat
    # sees its bind-mounted number; path and inode are the stable join here.
    mapped_library_matches = (mapped is not None
                              and mapped["path"] == code_before["extension"]["path"]
                              and mapped["inode"] == code_before["extension"]["inode"])
    source_after = sha256(source) if source else None
    live_slot = saves / "slot_1.sram"
    result = json.loads(native_report.read_text()) if native_report.is_file() else None
    save_acks = result.get("camp_save_acks") if isinstance(result, dict) else None
    acknowledged = (isinstance(save_acks, list) and len(save_acks) == 3
                    and isinstance(save_acks[0], int) and save_acks[0] > 0)
    receipt = {
        "command": command, "exit_code": code, "elapsed_s": round(elapsed, 3),
        "started_at_utc": started_at_utc, "ended_at_utc": ended_at_utc,
        "code_identity_before": code_before, "code_identity_after": code_after,
        "source_identity_before": git_before, "source_identity_after": git_after,
        "identity_stable": identity_stable,
        "mapped_extension": mapped, "mapped_library_matches_preflight": mapped_library_matches,
        "pack_manifest_sha256_before": pack_manifest_before,
        "tape_sha256_before": tape_sha_before,
        "effective_psiv_env": {key: env[key] for key in sorted(env) if key.startswith("PSIV_")},
        "removed_inherited_psiv_keys": removed_psiv_keys,
        "godot_import_command": import_command,
        "godot_import_exit_code": import_code,
        "godot_import_log": str(import_log) if import_command else None,
        "tape": str(tape), "tape_sha256": tape_sha_after,
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
        "pack": str(pack), "pack_manifest_sha256": pack_manifest_after,
        "native_report": result,
        "godot_log": str(godot_log), "run_log": str(run_log),
        "capture": str(capture) if capture.is_file() else None,
    }
    receipt["pass"] = (
        code == 0 and result is not None and result.get("result") == "pass"
        and identity_stable and mapped_library_matches
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
