"""Frame-addressed read-only observations shared by native tape entry points."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re


def load_requests(path: Path, tape_sha256: str | None = None) -> list[dict]:
    """Reject ambiguous frame addresses and unsafe artifact names before launch."""
    entries = json.loads(path.read_text())
    if isinstance(entries, dict):
        if (set(entries) != {"tape_sha256", "requests"}
                or not isinstance(entries["tape_sha256"], str)
                or not re.fullmatch(r"[0-9a-f]{64}", entries["tape_sha256"])
                or entries["tape_sha256"] != tape_sha256):
            raise ValueError("observation manifest does not name this tape SHA-256")
        entries = entries["requests"]
    if not isinstance(entries, list) or not entries:
        raise ValueError("observations must be a non-empty JSON array")
    previous = 0
    labels = set()
    for item in entries:
        if not isinstance(item, dict) or set(item) - {"frame", "label", "capture"}:
            raise ValueError("observation needs frame, label and optional capture boolean")
        frame, label = item.get("frame"), item.get("label")
        if type(frame) is not int or frame < previous or frame < 1:
            raise ValueError("observation frames must be positive ascending integers")
        if not isinstance(label, str) or not re.fullmatch(r"[A-Za-z0-9_-]+", label) or label in labels:
            raise ValueError("observation labels must be unique safe artifact names")
        if type(item.get("capture", False)) is not bool:
            raise ValueError("observation capture must be a boolean")
        previous = frame
        labels.add(label)
    return entries


def passed(requests: list[dict], report: dict | None, directory: Path) -> bool:
    """A successful process cannot hide an omitted or misaddressed observation."""
    if not requests:
        return True
    results = (report or {}).get("observations")
    if not isinstance(results, list) or len(results) != len(requests):
        return False
    for want, actual in zip(requests, results):
        if not isinstance(actual, dict) or any(actual.get(key) != want[key] for key in ("label", "frame")):
            return False
        state = actual.get("state")
        if (type(actual.get("frame")) is not int or not isinstance(state, dict)
                or not {"map", "cell"} <= state.keys()):
            return False
        if want.get("capture", False):
            path = directory / f"{want['label']}.png"
            if actual.get("capture") != str(path) or not path.is_file() or path.stat().st_size == 0:
                return False
            if actual.get("capture_file_sha256") != hashlib.sha256(path.read_bytes()).hexdigest():
                return False
    return True
