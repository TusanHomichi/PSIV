"""Oracle receipts that close the residual enemy attack presentations.

The static decoder remains the authority for retail bytes.  This module is the
separate observed-behaviour seam: the headless retail oracle records the live
sprite table/VRAM state for one representative of each shared routine body,
then the pack consumes that receipt instead of inventing a mapping loop from a
PLC upload alone.
"""

from __future__ import annotations

import json
from functools import lru_cache
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
RECEIPT_PATH = ROOT / "oracle" / "fixtures" / "battle_animation_remainder.json"


class BattleAnimationReceiptError(ValueError):
    """The checked-in oracle receipt is malformed or does not cover a body."""


@lru_cache(maxsize=1)
def receipt_file() -> dict[str, Any]:
    if not RECEIPT_PATH.is_file():
        raise BattleAnimationReceiptError(f"missing oracle receipt: {RECEIPT_PATH}")
    payload = json.loads(RECEIPT_PATH.read_text())
    if payload.get("format_version") != 1:
        raise BattleAnimationReceiptError("unsupported battle-animation receipt version")
    if payload.get("source", {}).get("grand_cross") != 0:
        raise BattleAnimationReceiptError("battle-animation receipt is not Grand Cross=0")
    fixture = payload.get("fixture", {})
    if fixture.get("enemy_position") != "0x0E":
        raise BattleAnimationReceiptError(
            "battle-animation fixture does not record Enemy_Positions[0] = $0E"
        )
    return payload


@lru_cache(maxsize=1)
def receipts_by_routine() -> dict[str, dict[str, Any]]:
    receipts = {}
    for receipt in receipt_file().get("receipts", []):
        routine = receipt.get("routine_offset")
        if not isinstance(routine, str) or routine in receipts:
            raise BattleAnimationReceiptError(
                f"duplicate or missing receipt routine offset: {routine!r}"
            )
        offsets = receipt.get("observed_offsets", [])
        durations = receipt.get("durations", [])
        if not offsets or len(offsets) != len(durations):
            raise BattleAnimationReceiptError(
                f"{routine}: observed offsets and durations do not line up"
            )
        if offsets[0] != 0 or any(value < 0 for value in offsets):
            raise BattleAnimationReceiptError(f"{routine}: invalid observation offsets")
        if any(value <= 0 for value in durations):
            raise BattleAnimationReceiptError(f"{routine}: non-positive observed duration")
        capture = receipt.get("capture")
        if not isinstance(capture, dict):
            raise BattleAnimationReceiptError(f"{routine}: capture hash receipt is missing")
        if sum(durations) != capture.get("frame_count"):
            raise BattleAnimationReceiptError(
                f"{routine}: observed durations do not cover the capture window"
            )
        if any(
            offset != sum(durations[:index])
            for index, offset in enumerate(offsets)
        ):
            raise BattleAnimationReceiptError(
                f"{routine}: offsets are not the starts of the observed dwells"
            )
        if capture.get("frame_last_exclusive") != (
            capture.get("frame_first", -1) + capture.get("frame_count", 0)
        ):
            raise BattleAnimationReceiptError(
                f"{routine}: capture frame range is inconsistent"
            )
        if len(receipt.get("enemy_ids", [])) == 0:
            raise BattleAnimationReceiptError(f"{routine}: receipt has no enemy ids")
        origin = receipt.get("origin_pixels")
        if (
            not isinstance(origin, list)
            or len(origin) != 2
            or not all(isinstance(value, int) for value in origin)
        ):
            raise BattleAnimationReceiptError(
                f"{routine}: formation-relative origin_pixels are missing"
            )
        receipts[routine] = receipt
    return receipts


def receipt_for(routine_offset: str) -> dict[str, Any] | None:
    """Return the observed receipt for a routine, if this wave covers it."""
    return receipts_by_routine().get(routine_offset)


def darkforce1_receipt() -> dict[str, Any]:
    """Return the formation/CRAM proof for DarkForce1's palette-bit mapping."""
    receipt = receipt_file().get("darkforce1")
    if not isinstance(receipt, dict):
        raise BattleAnimationReceiptError("DarkForce1 palette receipt is missing")
    words = receipt.get("cram_words", [])
    if len(words) != 16 or receipt.get("cram_line") not in (1, 2):
        raise BattleAnimationReceiptError("DarkForce1 CRAM receipt is malformed")
    return receipt


def _oracle_mapping_record(receipt: dict[str, Any], index: int) -> dict[str, Any]:
    """Build a typed mapping-record shell for one observed hardware frame.

    The pixel source is the oracle PNG fixture.  Keeping a record shell in the
    animation JSON still makes the timing and provenance visible to tools that
    do not load the art pack, without pretending that a RAM observation was a
    ROM mapping pointer.
    """
    offset = receipt["observed_offsets"][index]
    frame = receipt["action_frame"] + offset
    routine = receipt["routine_offset"]
    return {
        "rom_offset": f"oracle:{routine}:frame{index:02d}",
        "record_header_bytes": 0,
        "entry_bytes": 8,
        "sprite_count": None,
        "entries": [],
        "all_entries_valid": True,
        "record_end": None,
        "observed_frame": frame,
        "observed_offset": offset,
        "observation": "oracle state_dump sprite_table + vdp_vram",
        "receipt": receipt["receipt"],
    }


def _apply_observed_receipt(animation: dict[str, Any], receipt: dict[str, Any]) -> None:
    durations = list(receipt["durations"])
    pointers = [
        f"oracle:{receipt['routine_offset']}:frame{index:02d}"
        for index in range(len(durations))
    ]
    sequence = {
        "object_id": None,
        "assignment_offset": f"oracle:{receipt['routine_offset']}:sprite-table",
        "mapping_offset": f"oracle:{receipt['routine_offset']}:observed",
        "frame_duration": durations[0],
        "frame_count": len(durations),
        "total_frames": sum(durations),
        "mapping_pointers": pointers,
        "frame_timer_helper": "oracle:observed_sprite_table_clock",
        "frame_durations": durations,
        "timing_model": "observed",
        "mapping_records": [
            _oracle_mapping_record(receipt, index)
            for index in range(len(durations))
        ],
        "receipt": {
            "routine_offset": receipt["routine_offset"],
            "representative_enemy_id": receipt["representative_enemy_id"],
            "action_frame": receipt["action_frame"],
            "observed_offsets": receipt["observed_offsets"],
            "capture": receipt["capture"],
            "source": receipt["receipt"],
        },
    }
    animation["frame_sequence"] = sequence
    animation["frame_sequence_why_not"] = None
    animation["composition"] = {
        "status": "exact",
        "reason": (
            "oracle receipt resolves the retail presentation as the observed "
            f"{receipt['presentation']} sprite-table state; "
            f"{receipt['receipt']}"
        ),
    }
    animation["movement"] = {
        "status": "exact",
        "reason": (
            "observed SAT frames carry the retail effect motion; the local "
            "attack layer stays at the receipt-backed formation anchor"
        ),
        "fields": {
            "x": "$2C(a4)",
            "y": "$2E(a4)",
            "fixed_point_x": "$30(a4)",
            "fixed_point_y": "$34(a4)",
            "shared_init": "loc_12618",
        },
        "writes": animation["movement"].get("writes", []),
        "tracks": animation["movement"].get("tracks", []),
        "track_completion": {
            "status": "exact",
            "fields": ["$2C(a4)", "$2E(a4)", "$30(a4)", "$34(a4)"],
            "reason": "the oracle frame carries sprite/effect motion; layer anchor is receipt-backed",
        },
        "runtime": {
            "kind": "observed_receipt",
            "initial_offset_pixels": [0, 0],
            "step_pixels": [0, 0],
            "branches": [],
        },
    }
    animation["movement_proven"] = True
    animation["flash_timing_proven"] = True
    animation["sprite_sheet_proven"] = True
    animation["oracle_receipt"] = receipt
    animation["routine_classification_reason"] += "; completed by oracle hardware receipt"


def _apply_darkforce1_receipt(animation: dict[str, Any]) -> None:
    receipt = darkforce1_receipt()
    sequence = animation.get("frame_sequence")
    if sequence is None:
        raise BattleAnimationReceiptError("DarkForce1 has no static mapping sequence to resolve")
    selector = receipt["palette_selector"]
    masked = int(receipt["palette_word_masked"], 16)
    for record in sequence["mapping_records"]:
        for entry in record.get("entries", []):
            word = int(entry["tile_word"], 16)
            if word & int(receipt["palette_mask"], 16):
                if (word & int(receipt["palette_mask"], 16)) != masked:
                    raise BattleAnimationReceiptError(
                        f"DarkForce1 palette word {word:#06x} disagrees with receipt"
                    )
                if entry["palette_bits"] != selector:
                    raise BattleAnimationReceiptError(
                        "DarkForce1 palette selector disagrees with receipt"
                    )
                entry["cram_line"] = receipt["cram_line"]
            entry["attributes_valid"] = True
        record["all_entries_valid"] = True
        record["palette_receipt"] = {
            "cram_line": receipt["cram_line"],
            "palette_mask": receipt["palette_mask"],
            "palette_word_masked": receipt["palette_word_masked"],
        }
    sequence["palette_receipt"] = {
        "cram_line": receipt["cram_line"],
        "cram_words": receipt["cram_words"],
        "formation_patch": receipt["formation_patch"],
        "receipt": receipt["receipt"],
    }
    animation["composition"] = {
        "status": "exact",
        "reason": (
            "DarkForce1 $6000 palette-bit mapping is resolved by the oracle "
            f"formation CRAM receipt: selector {selector} selects CRAM line "
            f"{receipt['cram_line']} ({receipt['receipt']})"
        ),
    }
    animation["oracle_receipt"] = receipt
    animation["sprite_sheet_proven"] = True


def apply_oracle_receipt(animation: dict[str, Any]) -> None:
    """Close one extracted animation using a checked-in oracle receipt."""
    routine = animation["routine_offset"]
    if animation["enemy_id"] == 130:
        _apply_darkforce1_receipt(animation)
        return
    receipt = receipt_for(routine)
    if receipt is not None and animation["enemy_id"] in receipt["enemy_ids"]:
        _apply_observed_receipt(animation, receipt)
