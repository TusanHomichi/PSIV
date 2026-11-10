"""Subprocess glue for the Rust native-observation contract owner.

Local executables are explicit build prerequisites, like the campaign runner.
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


def verify(request: dict):
    override = os.environ.get("PSIV_NATIVE_VERIFY_BINARY")
    if override:
        return _exchange(Path(override), request)
    expected = {name: (ROOT / name).read_text(encoding="utf-8") for name in INPUTS}
    for profile in ("debug", "release"):
        binary = ROOT / "rust/target" / profile / (
            "psiv-native-verify.exe" if os.name == "nt" else "psiv-native-verify")
        if binary.is_file() and _exchange(binary, {"operation": "identity"}) == expected:
            return _exchange(binary, request)
    raise ValueError("Rust native verifier missing or stale: build psiv-native-verify "
                     "under the build lock before entering a Godot or Python holder")
