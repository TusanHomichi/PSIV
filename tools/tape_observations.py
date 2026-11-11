"""Python launch glue for the Rust frame-addressed observation contract."""
from __future__ import annotations

import json
from pathlib import Path

try:
    from tools.native_verifier import verify
except ModuleNotFoundError:
    from native_verifier import verify


def load_requests(path: Path, tape_sha256: str | None = None) -> list[dict]:
    return verify({"operation": "requests", "manifest": json.loads(path.read_text()),
                   "tape_sha256": tape_sha256})


def passed(requests: list[dict], report: dict | None, directory: Path) -> bool:
    try:
        return verify({"operation": "receipt", "requests": requests,
                       "report": report, "directory": str(directory)})["pass"]
    except (OSError, ValueError, KeyError):
        return False
