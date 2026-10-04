"""Verify that a map-update pack differs from an accepted pack only additively."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

MAP_ADDITIONS = {"map_updates", "map_update_palette", "map_update_images"}


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def compare(accepted: Path, candidate: Path) -> dict:
    """Check every original file, map JSON field and manifest declaration."""
    old = json.loads((accepted / "manifest.json").read_text())
    new = json.loads((candidate / "manifest.json").read_text())
    original = {str(p.relative_to(accepted)) for p in accepted.rglob("*") if p.is_file()}
    current = {str(p.relative_to(candidate)) for p in candidate.rglob("*") if p.is_file()}
    errors = [f"removed {p}" for p in sorted(original - current)]
    maps = {entry["json"] for entry in old["maps"]}
    declared = set()
    for name in sorted(maps & current):
        baseline = json.loads((accepted / name).read_text())
        updated = json.loads((candidate / name).read_text())
        if not MAP_ADDITIONS.intersection(updated):
            errors.append(f"map additions missing: {name}")
        for image in updated.get("map_update_images", {}).values():
            declared.add(image)
        if {k: v for k, v in updated.items() if k not in MAP_ADDITIONS} != baseline:
            errors.append(f"non-additive map change: {name}")
    normalized = json.loads(json.dumps(new))
    normalized.pop("map_updates", None)
    old_entries = {entry["id"]: entry for entry in old["maps"]}
    for entry in normalized["maps"]:
        baseline = old_entries.get(entry["id"], {})
        if "json_sha256" in entry:
            path = candidate / entry["json"]
            if not path.is_file() or entry["json_sha256"] != digest(path):
                errors.append(f"invalid map JSON digest: {entry['json']}")
            entry["json_sha256"] = baseline.get("json_sha256")
    if normalized != old:
        errors.append("non-additive manifest change")
    unchanged = 0
    for name in sorted((original & current) - maps - {"manifest.json"}):
        if digest(accepted / name) != digest(candidate / name):
            errors.append(f"changed original bytes: {name}")
        else:
            unchanged += 1
    additions = current - original
    if additions != declared:
        errors.append(f"undeclared/missing additions: {sorted(additions ^ declared)}")
    return {"accepted": str(accepted.resolve()), "candidate": str(candidate.resolve()),
            "accepted_manifest_sha256": digest(accepted / "manifest.json"),
            "candidate_manifest_sha256": digest(candidate / "manifest.json"),
            "original_files": len(original), "unchanged_binary_and_other_files": unchanged,
            "additive_map_json_files": len(maps), "added_index_images": len(additions),
            "errors": errors, "passed": not errors}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("accepted", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    result = compare(args.accepted, args.candidate)
    args.report.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
