"""Census the retail ``$F2`` actions in an extracted dialogue JSON file.

The extractor deliberately keeps action operands raw.  This module is the
read-only audit view over that data: it names every action occurrence, derives
the typed payload where the retail decoder has one, and preserves the exact
tree/entry location for each occurrence.
"""

from __future__ import annotations

import json
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable


ACTION_SPECS: tuple[tuple[int, str], ...] = (
    (0, "load_panel"),
    (1, "destroy_last_panel"),
    (2, "destroy_all_panels"),
    (3, "load_sound"),
    (4, "load_sound_2"),
    (6, "update_palette"),
    (7, "zio_eyes_red"),
    (8, "pause_music"),
    (9, "resume_music"),
    (10, "sabotage_alarm_red_palette"),
    (11, "set_event_flag"),
    (12, "elsydeon_broken"),
)
ACTION_NAMES = dict(ACTION_SPECS)


def _entry_location(tree: dict[str, Any], entry: dict[str, Any]) -> str:
    label = tree.get("label") or f"DialogueTree{tree['tree']}"
    return f"{label}:{entry['id']}"


def _action_payload(action: str, operands: list[int]) -> tuple[str, int] | None:
    if action == "load_panel" and len(operands) >= 2:
        return "panel", (operands[0] << 8) | operands[1]
    if action in ("load_sound", "load_sound_2") and operands:
        return "sound", operands[0]
    if action == "set_event_flag" and operands:
        return "flag", operands[0]
    return None


def _hex(value: int) -> str:
    return f"0x{value:02X}" if value < 0x100 else f"0x{value:X}"


def _entry_counts(values: Iterable[str]) -> dict[str, int]:
    return dict(sorted(Counter(values).items()))


def census(document: dict[str, Any]) -> dict[str, Any]:
    """Return a stable, JSON-serialisable census for an extracted dialogue file."""

    entries = 0
    occurrences: list[dict[str, Any]] = []
    for tree in document.get("trees", []):
        for entry in tree.get("entries", []):
            entries += 1
            location = _entry_location(tree, entry)
            for segment_index, segment in enumerate(entry.get("segments", [])):
                if segment.get("ctrl") not in ("0xF2", "action"):
                    continue
                action_id = int(segment["action_id"])
                try:
                    action = segment.get("action") or ACTION_NAMES[action_id]
                except KeyError as exc:
                    raise ValueError(
                        f"unknown retail dialogue action id {action_id} at {location}"
                    ) from exc
                if action_id not in ACTION_NAMES:
                    raise ValueError(
                        f"Grand Cross-only or unknown dialogue action id {action_id} "
                        f"at {location}"
                    )
                operands = [int(value) for value in segment.get("operands", [])]
                payload = _action_payload(action, operands)
                record: dict[str, Any] = {
                    "tree": tree.get("tree"),
                    "entry": entry["id"],
                    "location": location,
                    "segment": segment_index,
                    "action_id": action_id,
                    "action": action,
                    "operands": operands,
                }
                if payload is not None:
                    record["payload_kind"], value = payload
                    record["payload"] = value
                    record["payload_hex"] = _hex(value)
                occurrences.append(record)

    by_action: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for occurrence in occurrences:
        by_action[occurrence["action"]].append(occurrence)

    actions: dict[str, Any] = {}
    for action_id, action in ACTION_SPECS:
        records = by_action[action]
        payloads: dict[str, list[str]] = defaultdict(list)
        for record in records:
            if "payload_hex" in record:
                payloads[record["payload_hex"]].append(record["location"])
        actions[action] = {
            "action_id": action_id,
            "occurrences": len(records),
            "entries": _entry_counts(record["location"] for record in records),
            "payloads": {
                value: {
                    "occurrences": len(locations),
                    "entries": _entry_counts(locations),
                }
                for value, locations in sorted(payloads.items())
            },
        }

    panel_ids = sorted(
        {
            record["payload"]
            for record in occurrences
            if record["action"] == "load_panel" and "payload" in record
        }
    )
    return {
        "entries": entries,
        "action_occurrences": len(occurrences),
        "action_entries": len({record["location"] for record in occurrences}),
        "actions": actions,
        "panel_ids": panel_ids,
        "panel_count": len(panel_ids),
        "occurrences": occurrences,
    }


def load(path: str | Path) -> dict[str, Any]:
    """Load and census a generated or runtime-pack dialogue JSON file."""

    document = json.loads(Path(path).read_text(encoding="utf-8"))
    return census(document)


def _locations(entries: dict[str, int]) -> str:
    return ", ".join(
        f"{location} ({count}x)" if count > 1 else location
        for location, count in entries.items()
    ) or "—"


def markdown(result: dict[str, Any]) -> str:
    """Render the census as a compact audit document."""

    lines = [
        "# Dialogue-embedded actions",
        "",
        "This census is produced from the extracted dialogue JSON with `python3 -m "
        "psiv_tools dialogue-census generated/dialogue.json --format markdown`. "
        "Locations are `DialogueTree:entry`; a repeated payload in one entry is "
        "shown as `Nx`.",
        "",
        f"- Dialogue entries: **{result['entries']}**",
        f"- `$F2` action occurrences: **{result['action_occurrences']}**",
        f"- Entries containing actions: **{result['action_entries']}**",
        f"- Distinct `LoadPanel` ids: **{result['panel_count']}**",
        "",
        "## Action-kind totals",
        "",
        "| id | ActionKind | occurrences | entries |",
        "|---:|---|---:|---:|",
    ]
    for action_id, action in ACTION_SPECS:
        item = result["actions"][action]
        lines.append(
            f"| `{action_id}` | `{action}` | {item['occurrences']} | "
            f"{len(item['entries'])} |"
        )

    lines += ["", "## Payload census", ""]
    for action_id, action in ACTION_SPECS:
        item = result["actions"][action]
        if item["payloads"]:
            payload_kind = next(
                record["payload_kind"]
                for record in result["occurrences"]
                if record["action"] == action and "payload_kind" in record
            )
            lines += [
                f"### `{action}` ({payload_kind})",
                "",
                "| payload | occurrences | entries |",
                "|---|---:|---|",
            ]
            for value, payload in item["payloads"].items():
                lines.append(
                    f"| `{value}` | {payload['occurrences']} | "
                    f"{_locations(payload['entries'])} |"
                )
            lines.append("")
        else:
            lines += [
                f"### `{action}`",
                "",
                f"Entries: {_locations(item['entries'])}.",
                "",
            ]

    ids = result["panel_ids"]
    lines += [
        "## Distinct action-referenced panel ids",
        "",
        "These are the additive panel records emitted by `presentation_pack.py`; "
        "the scene-owned records remain in the same manifest.",
        "",
    ]
    for start in range(0, len(ids), 12):
        lines.append(" ".join(f"`{_hex(value)}`" for value in ids[start : start + 12]))
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dialogue_json", type=Path)
    parser.add_argument("--format", choices=("json", "markdown"), default="json")
    args = parser.parse_args()
    result = load(args.dialogue_json)
    if args.format == "markdown":
        print(markdown(result))
    else:
        print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
