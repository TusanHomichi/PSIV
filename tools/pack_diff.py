#!/usr/bin/env python3
"""
tools/pack_diff.py - CLI tool to diff two runtime-pack directories.

Compares files between two directory trees:
1) Reports files present only in one side (grouped, sorted).
2) SHA-256 compares shared files and reports differing paths.
3) For differing .json files, prints up to 20 leaf-level structural diffs
   as lines like: maps/013.json: $.npcs[3].interactable: true -> false
   (arrays element-wise; length mismatch as $.path.length: N -> M), then a
   remaining-differences count.
4) Exit codes:
   - 0: Identical packs (no differences)
   - 1: Differences found
   - 2: Usage or I/O error
5) Summary footer: compared/identical/differing/only-in-A/only-in-B counts.
6) --quiet prints only the summary.
7) --self-test builds temporary fixtures, asserts behavior and exit codes,
   prints SELF-TEST OK, and exits 0.

Stdlib-only, Python 3 with type hints.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from typing import Any, Dict, List, Optional, Set, Tuple


class _MissingSentinel:
    """Sentinel representing a missing key or value in JSON diff."""

    def __repr__(self) -> str:
        return "<missing>"

    def __str__(self) -> str:
        return "<missing>"


MISSING = _MissingSentinel()


def json_repr(val: Any) -> str:
    """Format a JSON value or sentinel into a readable string."""
    if val is MISSING:
        return "<missing>"
    return json.dumps(val, ensure_ascii=False)


def diff_json(a: Any, b: Any, path: str = "$") -> list[str]:
    """Compute leaf-level structural differences between two JSON-compatible objects.

    - Objects/dicts are diffed key-by-key (sorted).
    - Arrays are diffed element-wise; length mismatches are reported as $.path.length: N -> M.
    - Type mismatches and primitive inequalities are reported as old -> new.
    """
    diffs: list[str] = []

    # Case 1: Both are dictionaries / JSON objects
    if isinstance(a, dict) and isinstance(b, dict):
        all_keys = sorted(set(a.keys()) | set(b.keys()))
        for k in all_keys:
            child_path = f"{path}.{k}"
            if k not in b:
                diffs.append(f"{child_path}: {json_repr(a[k])} -> <missing>")
            elif k not in a:
                diffs.append(f"{child_path}: <missing> -> {json_repr(b[k])}")
            else:
                diffs.extend(diff_json(a[k], b[k], child_path))
        return diffs

    # Case 2: Both are lists / JSON arrays
    if isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            diffs.append(f"{path}.length: {len(a)} -> {len(b)}")
        min_len = min(len(a), len(b))
        for i in range(min_len):
            child_path = f"{path}[{i}]"
            diffs.extend(diff_json(a[i], b[i], child_path))
        return diffs

    # Case 3: Type mismatch or primitive value inequality
    # In Python, isinstance(True, int) is True, so compare exact types
    if type(a) is not type(b) or a != b:
        diffs.append(f"{path}: {json_repr(a)} -> {json_repr(b)}")

    return diffs


def collect_pack_files(pack_dir: Path) -> dict[str, Path]:
    """Recursively find all regular files in pack_dir, mapping relative posix paths to Path objects."""
    pack_dir = pack_dir.resolve()
    files: dict[str, Path] = {}
    for root, _, filenames in os.walk(pack_dir):
        for fname in filenames:
            full_path = Path(root) / fname
            if full_path.is_file():
                rel_path = full_path.relative_to(pack_dir).as_posix()
                files[rel_path] = full_path
    return files


def compute_sha256(file_path: Path) -> str:
    """Compute the SHA-256 hash of a file."""
    hasher = hashlib.sha256()
    with open(file_path, "rb") as f:
        while chunk := f.read(65536):
            hasher.update(chunk)
    return hasher.hexdigest()


class PackDiffResult:
    """Stores the diff results between two pack directories."""

    def __init__(
        self,
        pack_a: Path,
        pack_b: Path,
        only_in_a: list[str],
        only_in_b: list[str],
        identical_files: list[str],
        differing_files: list[str],
        json_diffs: dict[str, list[str]],
        non_json_diffs: dict[str, str],
        invalid_json_diffs: dict[str, str],
    ) -> None:
        self.pack_a = pack_a
        self.pack_b = pack_b
        self.only_in_a = only_in_a
        self.only_in_b = only_in_b
        self.identical_files = identical_files
        self.differing_files = differing_files
        self.json_diffs = json_diffs
        self.non_json_diffs = non_json_diffs
        self.invalid_json_diffs = invalid_json_diffs

    @property
    def compared_count(self) -> int:
        return len(self.identical_files) + len(self.differing_files)

    @property
    def identical_count(self) -> int:
        return len(self.identical_files)

    @property
    def differing_count(self) -> int:
        return len(self.differing_files)

    @property
    def only_in_a_count(self) -> int:
        return len(self.only_in_a)

    @property
    def only_in_b_count(self) -> int:
        return len(self.only_in_b)

    @property
    def has_differences(self) -> bool:
        return bool(self.only_in_a or self.only_in_b or self.differing_files)


def compare_packs(pack_a: Path, pack_b: Path) -> PackDiffResult:
    """Compare two pack directories and return structured diff results."""
    files_a = collect_pack_files(pack_a)
    files_b = collect_pack_files(pack_b)

    keys_a = set(files_a.keys())
    keys_b = set(files_b.keys())

    only_in_a = sorted(keys_a - keys_b)
    only_in_b = sorted(keys_b - keys_a)
    shared = sorted(keys_a & keys_b)

    identical_files: list[str] = []
    differing_files: list[str] = []
    json_diffs: dict[str, list[str]] = {}
    non_json_diffs: dict[str, str] = {}
    invalid_json_diffs: dict[str, str] = {}

    for rel_path in shared:
        path_a = files_a[rel_path]
        path_b = files_b[rel_path]

        sha_a = compute_sha256(path_a)
        sha_b = compute_sha256(path_b)

        if sha_a == sha_b:
            identical_files.append(rel_path)
        else:
            differing_files.append(rel_path)
            if rel_path.lower().endswith(".json"):
                try:
                    with open(path_a, "r", encoding="utf-8") as fa:
                        obj_a = json.load(fa)
                    with open(path_b, "r", encoding="utf-8") as fb:
                        obj_b = json.load(fb)
                    diffs = diff_json(obj_a, obj_b, "$")
                    json_diffs[rel_path] = diffs
                except Exception as exc:
                    invalid_json_diffs[rel_path] = str(exc)
            else:
                non_json_diffs[rel_path] = f"binary/content differs (SHA-256: {sha_a[:8]}... -> {sha_b[:8]}...)"

    return PackDiffResult(
        pack_a=pack_a,
        pack_b=pack_b,
        only_in_a=only_in_a,
        only_in_b=only_in_b,
        identical_files=identical_files,
        differing_files=differing_files,
        json_diffs=json_diffs,
        non_json_diffs=non_json_diffs,
        invalid_json_diffs=invalid_json_diffs,
    )


def print_report(result: PackDiffResult, quiet: bool = False, max_diffs: int = 20) -> None:
    """Print the diff report and summary footer."""
    if not quiet:
        if result.only_in_a:
            print(f"Files only in pack A ({result.pack_a}):")
            for rel in result.only_in_a:
                print(f"  {rel}")
            print()

        if result.only_in_b:
            print(f"Files only in pack B ({result.pack_b}):")
            for rel in result.only_in_b:
                print(f"  {rel}")
            print()

        if result.differing_files:
            print("Differing shared files:")
            for rel in result.differing_files:
                if rel in result.json_diffs:
                    diffs = result.json_diffs[rel]
                    if diffs:
                        shown = diffs[:max_diffs]
                        for d in shown:
                            print(f"{rel}: {d}")
                        remaining = len(diffs) - len(shown)
                        if remaining > 0:
                            plural = "difference" if remaining == 1 else "differences"
                            print(f"{rel}: ... and {remaining} more {plural}")
                    else:
                        print(f"{rel}: formatting/whitespace differs (structural JSON is identical)")
                elif rel in result.invalid_json_diffs:
                    print(f"{rel}: JSON decode error: {result.invalid_json_diffs[rel]}")
                elif rel in result.non_json_diffs:
                    print(f"{rel}: {result.non_json_diffs[rel]}")
                else:
                    print(f"{rel}: content differs")
            print()

    print("============================================================")
    print("Pack Diff Summary:")
    print(f"  Compared:       {result.compared_count}")
    print(f"  Identical:      {result.identical_count}")
    print(f"  Differing:      {result.differing_count}")
    print(f"  Only in A:      {result.only_in_a_count}")
    print(f"  Only in B:      {result.only_in_b_count}")
    print("============================================================")


def run_self_test() -> int:
    """Build temporary fixtures, assert CLI & API behavior, print SELF-TEST OK, and return 0."""
    # 1. Direct unit tests on diff_json
    d1 = {"a": 1, "b": {"c": 2, "d": [10, 20]}}
    d2 = {"a": 1, "b": {"c": 3, "d": [10, 20, 30]}}
    diffs = diff_json(d1, d2, "$")
    assert "$.b.c: 2 -> 3" in diffs, f"Expected $.b.c diff, got {diffs}"
    assert "$.b.d.length: 2 -> 3" in diffs, f"Expected $.b.d.length diff, got {diffs}"

    # Array element changes
    a1 = {"npcs": [{"id": 0}, {"id": 1}, {"id": 2}, {"interactable": True}]}
    a2 = {"npcs": [{"id": 0}, {"id": 1}, {"id": 2}, {"interactable": False}]}
    diffs_npc = diff_json(a1, a2, "$")
    assert diffs_npc == ["$.npcs[3].interactable: true -> false"], f"Unexpected diffs: {diffs_npc}"

    # Missing & added keys
    k1 = {"x": 1, "del": "foo"}
    k2 = {"x": 1, "add": "bar"}
    diffs_k = diff_json(k1, k2, "$")
    assert '$.add: <missing> -> "bar"' in diffs_k
    assert '$.del: "foo" -> <missing>' in diffs_k

    # Type mismatch (bool vs int)
    t1 = {"flag": True}
    t2 = {"flag": 1}
    diffs_t = diff_json(t1, t2, "$")
    assert diffs_t == ["$.flag: true -> 1"]

    # 2. Filesystem fixture testing
    with tempfile.TemporaryDirectory(prefix="pack_diff_test_") as tmp_dir_str:
        tmp_dir = Path(tmp_dir_str)
        pack1 = tmp_dir / "pack1"
        pack2 = tmp_dir / "pack2"
        pack1_clone = tmp_dir / "pack1_clone"

        (pack1 / "maps").mkdir(parents=True)
        (pack2 / "maps").mkdir(parents=True)
        (pack1 / "sprites").mkdir(parents=True)
        (pack2 / "sprites").mkdir(parents=True)

        # Identical JSON (exact same bytes)
        manifest_data = {"version": 1, "title": "PSIV", "flags": [1, 2, 3]}
        manifest_bytes = json.dumps(manifest_data, indent=2).encode("utf-8")
        (pack1 / "manifest.json").write_bytes(manifest_bytes)
        (pack2 / "manifest.json").write_bytes(manifest_bytes)

        # Identical Binary (exact same bytes)
        bin_data = bytes([0x00, 0x11, 0x22, 0x33, 0xFF])
        (pack1 / "sprites" / "shared.bin").write_bytes(bin_data)
        (pack2 / "sprites" / "shared.bin").write_bytes(bin_data)

        # Differing JSON with nested changes
        map_a = {
            "map_id": "013",
            "npcs": [
                {"id": 0, "name": "Alys"},
                {"id": 1, "name": "Chaz"},
                {"id": 2, "name": "Hahn"},
                {"id": 3, "interactable": True},
            ],
            "items": [1, 2, 3],
            "old_key": "to_remove",
            "count": 42,
        }
        map_b = {
            "map_id": "013",
            "npcs": [
                {"id": 0, "name": "Alys"},
                {"id": 1, "name": "Chaz"},
                {"id": 2, "name": "Hahn"},
                {"id": 3, "interactable": False},
            ],
            "items": [1, 2, 3, 4, 5],
            "new_key": "was_added",
            "count": "forty-two",
        }
        (pack1 / "maps" / "013.json").write_text(json.dumps(map_a), encoding="utf-8")
        (pack2 / "maps" / "013.json").write_text(json.dumps(map_b), encoding="utf-8")

        # Differing JSON with >20 diffs (25 diffs)
        many_a = {f"k{i}": i for i in range(25)}
        many_b = {f"k{i}": i + 100 for i in range(25)}
        (pack1 / "many_diffs.json").write_text(json.dumps(many_a), encoding="utf-8")
        (pack2 / "many_diffs.json").write_text(json.dumps(many_b), encoding="utf-8")

        # Differing Binary
        (pack1 / "sprites" / "diff.png").write_bytes(b"PNG_DATA_A_12345")
        (pack2 / "sprites" / "diff.png").write_bytes(b"PNG_DATA_B_67890")

        # Whitespace/Formatting difference only
        fmt_data = {"test": [1, 2, 3]}
        (pack1 / "formatting.json").write_text(json.dumps(fmt_data, indent=2), encoding="utf-8")
        (pack2 / "formatting.json").write_text(json.dumps(fmt_data), encoding="utf-8")

        # Only in Pack 1
        (pack1 / "only_in_a.txt").write_text("Hello Pack A", encoding="utf-8")

        # Only in Pack 2
        (pack2 / "only_in_b.txt").write_text("Hello Pack B", encoding="utf-8")

        # Clone pack1
        shutil.copytree(pack1, pack1_clone)

        script_path = str(Path(__file__).resolve())

        # Test Case A: Identical packs (pack1 vs pack1_clone)
        proc_identical = subprocess.run(
            [sys.executable, script_path, str(pack1), str(pack1_clone)],
            capture_output=True,
            text=True,
        )
        assert proc_identical.returncode == 0, f"Expected exit code 0, got {proc_identical.returncode}\n{proc_identical.stderr}"
        assert "Compared:       7" in proc_identical.stdout
        assert "Identical:      7" in proc_identical.stdout
        assert "Differing:      0" in proc_identical.stdout
        assert "Only in A:      0" in proc_identical.stdout
        assert "Only in B:      0" in proc_identical.stdout

        # Test Case B: Differing packs (pack1 vs pack2)
        proc_diff = subprocess.run(
            [sys.executable, script_path, str(pack1), str(pack2)],
            capture_output=True,
            text=True,
        )
        assert proc_diff.returncode == 1, f"Expected exit code 1, got {proc_diff.returncode}\n{proc_diff.stderr}"
        assert "Files only in pack A" in proc_diff.stdout
        assert "only_in_a.txt" in proc_diff.stdout
        assert "Files only in pack B" in proc_diff.stdout
        assert "only_in_b.txt" in proc_diff.stdout
        assert "maps/013.json: $.npcs[3].interactable: true -> false" in proc_diff.stdout
        assert "maps/013.json: $.items.length: 3 -> 5" in proc_diff.stdout
        assert 'maps/013.json: $.old_key: "to_remove" -> <missing>' in proc_diff.stdout
        assert 'maps/013.json: $.new_key: <missing> -> "was_added"' in proc_diff.stdout
        assert 'maps/013.json: $.count: 42 -> "forty-two"' in proc_diff.stdout
        assert "many_diffs.json: ... and 5 more differences" in proc_diff.stdout
        assert "formatting.json: formatting/whitespace differs" in proc_diff.stdout
        assert "sprites/diff.png: binary/content differs" in proc_diff.stdout
        assert "Compared:       6" in proc_diff.stdout
        assert "Identical:      2" in proc_diff.stdout
        assert "Differing:      4" in proc_diff.stdout
        assert "Only in A:      1" in proc_diff.stdout
        assert "Only in B:      1" in proc_diff.stdout

        # Test Case C: --quiet mode
        proc_quiet = subprocess.run(
            [sys.executable, script_path, str(pack1), str(pack2), "--quiet"],
            capture_output=True,
            text=True,
        )
        assert proc_quiet.returncode == 1, f"Expected exit code 1 in quiet mode, got {proc_quiet.returncode}"
        assert "Pack Diff Summary:" in proc_quiet.stdout
        assert "$.npcs[3].interactable" not in proc_quiet.stdout
        assert "only_in_a.txt" not in proc_quiet.stdout

        # Test Case D: Error handling (exit code 2)
        # 1. Missing args
        proc_noargs = subprocess.run(
            [sys.executable, script_path],
            capture_output=True,
            text=True,
        )
        assert proc_noargs.returncode == 2, f"Expected exit code 2 for missing args, got {proc_noargs.returncode}"

        # 2. Non-existent directory
        proc_nodir = subprocess.run(
            [sys.executable, script_path, str(pack1), str(tmp_dir / "nonexistent")],
            capture_output=True,
            text=True,
        )
        assert proc_nodir.returncode == 2, f"Expected exit code 2 for non-existent dir, got {proc_nodir.returncode}"

        # 3. File path instead of directory
        proc_file = subprocess.run(
            [sys.executable, script_path, str(pack1), str(pack1 / "manifest.json")],
            capture_output=True,
            text=True,
        )
        assert proc_file.returncode == 2, f"Expected exit code 2 for file path, got {proc_file.returncode}"

    print("SELF-TEST OK")
    return 0


def main(argv: Optional[list[str]] = None) -> int:
    """CLI entry point."""
    if argv is None:
        argv = sys.argv[1:]

    # Intercept --self-test early if present
    if "--self-test" in argv:
        return run_self_test()

    parser = argparse.ArgumentParser(
        description="Diff two runtime-pack directories (SHA-256 & structural JSON diff).",
        add_help=True,
    )
    parser.add_argument(
        "pack_a",
        nargs="?",
        help="Path to pack directory A",
    )
    parser.add_argument(
        "pack_b",
        nargs="?",
        help="Path to pack directory B",
    )
    parser.add_argument(
        "-q",
        "--quiet",
        action="store_true",
        help="Print only the summary footer",
    )
    parser.add_argument(
        "--max-diffs",
        type=int,
        default=20,
        help="Max structural diffs to display per JSON file (default: 20)",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run embedded self-tests and exit",
    )

    try:
        args = parser.parse_args(argv)
    except SystemExit as exc:
        return exc.code if isinstance(exc.code, int) else 2

    if args.self_test:
        return run_self_test()

    if not args.pack_a or not args.pack_b:
        sys.stderr.write("Error: Both <pack_a> and <pack_b> directory arguments are required.\n\n")
        parser.print_usage(sys.stderr)
        return 2

    path_a = Path(args.pack_a)
    path_b = Path(args.pack_b)

    if not path_a.exists():
        sys.stderr.write(f"Error: Pack A directory '{path_a}' does not exist.\n")
        return 2
    if not path_a.is_dir():
        sys.stderr.write(f"Error: Pack A path '{path_a}' is not a directory.\n")
        return 2

    if not path_b.exists():
        sys.stderr.write(f"Error: Pack B directory '{path_b}' does not exist.\n")
        return 2
    if not path_b.is_dir():
        sys.stderr.write(f"Error: Pack B path '{path_b}' is not a directory.\n")
        return 2

    try:
        result = compare_packs(path_a, path_b)
    except Exception as exc:
        sys.stderr.write(f"Error during pack comparison: {exc}\n")
        return 2

    print_report(result, quiet=args.quiet, max_diffs=args.max_diffs)

    return 1 if result.has_differences else 0


if __name__ == "__main__":
    sys.exit(main())
