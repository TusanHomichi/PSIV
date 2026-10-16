"""Stamp a synthetic table directory for a test.

The loader module has no function that blesses whatever is in a directory: a
real extract is stamped by `write_extract` alone. A test that builds a tiny
directory of its own uses this, and must call it again after rewriting a table,
because the stamp lists the bytes of each table.
"""
from __future__ import annotations

import json
from pathlib import Path

from psiv_tools.extract_stamp import (
    METADATA_FILE, METADATA_KEY, digest, metadata_digest, table_stamp)


def write_table_stamp(directory: str | Path) -> Path:
    """Stamp `directory` as a current extract of every `*.json` table in it."""
    directory = Path(directory)
    path = directory / METADATA_FILE
    metadata = json.loads(path.read_text(encoding="utf-8")) if path.is_file() else {}
    files = {table.name: digest(table.read_bytes())
             for table in sorted(directory.glob("*.json")) if table.name != METADATA_FILE}
    files[METADATA_FILE] = metadata_digest(metadata)
    metadata[METADATA_KEY] = {**table_stamp(), "files": files}
    directory.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    return path
