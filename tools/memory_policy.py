"""Interpreted bootstrap for a Rust executable that may be absent or stale.

The current checkout must own the input manifest, candidate selection and
freshness check before invoking the compiled policy. An old executable cannot
choose the inputs against which it declares itself current. Exact input-text
comparison and executable/source cache invalidation establish freshness, not
protection against a hostile executable.

When no candidate matches, compilation needs a build-class reservation. A
second Rust bootstrap would itself need a current executable or compilation;
that repeats the same prerequisite and cannot run inside an inherited Godot
or Python hold. This interpreted boundary therefore owns discovery, reuse and
reserved rebuild decisions. Counter availability and verdict acceptance stay
in Rust; the generic heavy-lock module owns reservation rules.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
BUILD_INPUTS = (
    "rust/psiv-memory-verdict/src/identity.rs",
    "rust/psiv-memory-verdict/src/lib.rs",
    "rust/psiv-memory-verdict/src/main.rs",
    "rust/psiv-memory-verdict/Cargo.toml",
    "rust/Cargo.toml", "rust/Cargo.lock", "rust-toolchain.toml",
)
_policy_binary: Path | None = None
_policy_inputs: dict | None = None
_policy_stat: os.stat_result | None = None


def _exchange(binary: Path, request: dict) -> dict:
    result = subprocess.run([str(binary)], input=json.dumps(request),
                            capture_output=True, text=True, check=False)
    output = json.loads(result.stdout)
    if result.returncode or not isinstance(output, dict):
        raise ValueError("Rust memory policy failed: " + result.stderr.strip())
    return output


def _inputs(root: Path) -> dict:
    return {name: (root / name).read_text(encoding="utf-8") for name in BUILD_INPUTS}


def _current(binary: Path, expected: dict) -> bool:
    try:
        return _exchange(binary, {"operation": "identity"}) == expected
    except (OSError, ValueError):
        return False


def _resolve(root: Path) -> Path:
    global _policy_binary, _policy_inputs, _policy_stat
    # A supplied derivation/release binary is an explicit dependency, not an
    # implicitly reused local artifact. It needs neither Cargo nor a build slot.
    override = os.environ.get("PSIV_MEMORY_VERDICT_BINARY")
    if override:
        return Path(override)
    expected = _inputs(root)
    if (_policy_binary is not None and _policy_inputs == expected
            and _policy_binary.is_file() and _policy_binary.stat() == _policy_stat):
        return _policy_binary
    manifest = str(root / "rust/Cargo.toml")
    metadata = subprocess.run(["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version=1",
                               "--manifest-path", manifest], capture_output=True,
                              text=True, check=False)
    if metadata.returncode:
        raise ValueError("cannot locate Rust memory policy: " + metadata.stderr.strip())
    target = Path(json.loads(metadata.stdout)["target_directory"])
    name = "psiv-memory-verdict.exe" if os.name == "nt" else "psiv-memory-verdict"
    for profile in ("debug", "release"):
        candidate = target / profile / name
        if candidate.is_file() and _current(candidate, expected):
            _policy_binary, _policy_inputs, _policy_stat = candidate, expected, candidate.stat()
            return candidate
    try:
        from tools.heavy_lock import BUILD, heavy_lock_path, hold_heavy_lock
    except ModuleNotFoundError:
        from heavy_lock import BUILD, heavy_lock_path, hold_heavy_lock
    try:
        with hold_heavy_lock(heavy_lock_path(root), lock_class=BUILD):
            built = subprocess.run(["cargo", "build", "--quiet", "--locked", "--manifest-path",
                                    manifest, "-p", "psiv-memory-verdict", "--message-format=json"],
                                   capture_output=True, text=True, check=False)
    except SystemExit as error:
        raise ValueError("the Rust memory policy is missing or stale; build it under the build "
                         "class before entering a Godot or Python holder") from error
    executables = [item["executable"] for item in map(json.loads, built.stdout.splitlines())
                   if item.get("reason") == "compiler-artifact" and item.get("executable")
                   and item.get("target", {}).get("name") == "psiv-memory-verdict"]
    if built.returncode or not executables:
        raise ValueError("cannot build Rust memory policy: " + built.stderr.strip())
    binary = Path(executables[-1])
    if not _current(binary, expected):
        raise ValueError("the built Rust memory policy does not match its current source inputs")
    _policy_binary, _policy_inputs, _policy_stat = binary, expected, binary.stat()
    return binary


def memory_policy(request: dict) -> dict:
    """Reuse an attested current artifact or build under its own reservation."""
    try:
        return _exchange(_resolve(ROOT), request)
    except (OSError, ValueError, KeyError) as error:
        return {"error": str(error)}
