"""Subprocess glue for the Rust native-observation contract owner.

Local executables are explicit build prerequisites, like the campaign runner;
prepare() provisions the Python suite before its workers start.
The source-text handshake rejects stale local builds; packaged overrides are
explicit dependencies and need neither Cargo nor a build inside a Godot lock.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
INPUTS = (
    "rust/psiv-native-verify/src/lib.rs", "rust/psiv-native-verify/src/main.rs",
    "rust/psiv-native-verify/Cargo.toml", "rust/Cargo.toml", "rust/Cargo.lock",
    "rust-toolchain.toml",
)


def _exchange(binary: Path, request: dict):
    run = subprocess.run([str(binary)], input=json.dumps(request),
                         capture_output=True, text=True, check=False)
    value = json.loads(run.stdout)
    if run.returncode:
        raise ValueError(value.get("error", run.stderr.strip()))
    return value


def _inputs():
    return {name: (ROOT / name).read_text(encoding="utf-8") for name in INPUTS}


def _current(binary, expected):
    try:
        return binary.is_file() and _exchange(binary, {"operation": "identity"}) == expected
    except (OSError, ValueError):
        return False


def _candidates():
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "rust/target"))
    name = "psiv-native-verify.exe" if os.name == "nt" else "psiv-native-verify"
    return [target / profile / name for profile in ("debug", "release")]


def _resolve(expected=None):
    override = os.environ.get("PSIV_NATIVE_VERIFY_BINARY")
    if override:
        return Path(override)
    if expected is None:
        expected = _inputs()
    for binary in _candidates():
        if _current(binary, expected):
            return binary
    return None


def prepare():
    """Provision once before suite workers; verification itself never compiles."""
    binary = _resolve()
    if binary is not None:
        return binary
    expected = _inputs()
    try:
        from tools.heavy_lock import BUILD, heavy_lock_path, hold_heavy_lock
    except ModuleNotFoundError:
        from heavy_lock import BUILD, heavy_lock_path, hold_heavy_lock
    with hold_heavy_lock(heavy_lock_path(ROOT), lock_class=BUILD):
        binary = _resolve(expected)
        if binary is not None:
            return binary
        # Cargo timestamps can call a stale executable fresh after a kept
        # target moved between checkouts. Invalidate only this crate's output.
        if any(path.is_file() for path in _candidates()):
            cleaned = subprocess.run(["cargo", "clean", "--manifest-path",
                                      str(ROOT / "rust/Cargo.toml"), "-p", "psiv-native-verify"],
                                     capture_output=True, text=True, check=False)
            if cleaned.returncode:
                raise ValueError("cannot clean stale Rust native verifier: " + cleaned.stderr.strip())
        built = subprocess.run(["cargo", "build", "--quiet", "--locked", "--manifest-path",
                                str(ROOT / "rust/Cargo.toml"), "-p", "psiv-native-verify",
                                "--message-format=json"], capture_output=True, text=True, check=False)
    if built.returncode:
        raise ValueError("cannot build Rust native verifier: " + built.stderr.strip())
    artifacts = [item["executable"] for item in map(json.loads, built.stdout.splitlines())
                 if item.get("reason") == "compiler-artifact" and item.get("executable")
                 and item.get("target", {}).get("name") == "psiv-native-verify"]
    if not artifacts or not _current(Path(artifacts[-1]), expected):
        raise ValueError("built Rust native verifier does not match current source inputs")
    return Path(artifacts[-1])


def verify(request: dict):
    binary = _resolve()
    if binary is None:
        raise ValueError("Rust native verifier missing or stale: build psiv-native-verify "
                         "under the build lock before entering a Godot or Python holder")
    return _exchange(binary, request)
