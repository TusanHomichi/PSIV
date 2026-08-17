"""Decode the retail camera and VDP scroll receipt from an oracle state."""

from __future__ import annotations

from typing import Any

SCROLL_LINE_COUNT = 224
SCROLL_WINDOW_FIRST_LINE = 160
HINT_RTE_ADDRESS = 0x758
RETAIL_PLANE_RESIDUE_PIXELS = {"x": 1, "y": 1}


class ScrollDecodeError(ValueError):
    """The optional scroll receipt exists but is malformed."""


def _optional_region(
    state: dict[str, Any], name: str, expected_size: int
) -> tuple[bytes, dict[str, Any]] | None:
    region = state.get("regions", {}).get(name)
    if region is None:
        return None
    if not isinstance(region, dict):
        raise ScrollDecodeError(f"state region {name!r} is malformed")
    try:
        raw = bytes.fromhex(region["bytes_hex"])
    except (KeyError, TypeError, ValueError) as exc:
        raise ScrollDecodeError(f"state region {name!r} is malformed") from exc
    if len(raw) != expected_size or region.get("size_bytes") != expected_size:
        raise ScrollDecodeError(
            f"state region {name!r} is {len(raw)} bytes, expected {expected_size}"
        )
    return raw, region


def _be_words(raw: bytes) -> list[int]:
    return [int.from_bytes(raw[i : i + 2], "big") for i in range(0, len(raw), 2)]


def decode_cram_shadow(state: dict[str, Any]) -> dict[str, Any]:
    """Decode the game-owned 64-word CRAM shadow carried by a state dump.

    The host receipt names this region ``Palette_Table_Buffer``.  It is not a
    live VDP read, but it is the exact big-endian shadow the retail renderer
    copied into CRAM before the paired PNG frame was captured.  Keeping the
    raw word and its three logical levels here gives palette investigations a
    lossless receipt instead of making them scrape a JSON hex string.
    """
    region = _optional_region(state, "cram", 0x80)
    if region is None:
        return {
            "present": False,
            "missing_region": "cram",
            "provenance": {
                "source": "oracle host state dump",
                "region_symbol": "Palette_Table_Buffer",
            },
        }

    raw, metadata = region
    words = _be_words(raw)
    entries = [
        {
            "index": index,
            "raw": f"0x{word:04X}",
            "levels": {
                "r": (word >> 1) & 0x7,
                "g": (word >> 5) & 0x7,
                "b": (word >> 9) & 0x7,
            },
        }
        for index, word in enumerate(words)
    ]
    return {
        "present": True,
        "word_count": len(entries),
        "raw_hex": raw.hex().upper(),
        "source": {
            "symbol": metadata.get("symbol", "Palette_Table_Buffer"),
            "storage": metadata.get("storage", "work_ram"),
            "address": metadata.get("address"),
            "byte_order": metadata.get("byte_order", "68000_address_order"),
        },
        "words": entries,
        "lines": [
            {
                "line": line,
                "words": entries[line * 16 : (line + 1) * 16],
            }
            for line in range(4)
        ],
        "provenance": {
            "receipt": "state.regions.cram",
            "paired_frame": state.get("frame"),
            "region_symbol": "Palette_Table_Buffer",
        },
    }


def _fixed_camera(raw: bytes, offset: int) -> dict[str, Any]:
    value = int.from_bytes(raw[offset : offset + 4], "big", signed=True)
    return {
        "raw": f"0x{value & 0xFFFFFFFF:08X}",
        "signed_16_16": value,
        "whole_pixels": value >> 16,
        "fraction_16": value & 0xFFFF,
    }


def _line_values(raw: bytes) -> list[dict[str, Any]]:
    return [
        {"line": line, "word": f"0x{word:04X}", "decimal": word}
        for line, word in enumerate(_be_words(raw)[:SCROLL_LINE_COUNT])
    ]


def decode_scroll_state(
    state: dict[str, Any], grand_cross: int = 0
) -> dict[str, Any]:
    """Decode camera, VDP scroll, and the retail H-int split inputs.

    Older state files predate the host VDP receipt. They remain decodable, but
    explicitly report the missing receipt instead of inventing zeros.
    """
    cram_shadow = decode_cram_shadow(state)
    needed = {
        "camera": 0x10,
        "camera_step_counters": 0x10,
        "h_int_state": 0x12,
    }
    raw: dict[str, bytes] = {}
    missing: list[str] = []
    for name, size in needed.items():
        region = _optional_region(state, name, size)
        if region is None:
            missing.append(name)
        else:
            raw[name] = region[0]
    if missing:
        return {
            "present": False,
            "missing_regions": missing,
            "cram_shadow": cram_shadow,
            "provenance": {
                "grand_cross": grand_cross,
                "source": "oracle host VDP/state receipt (not present in this file)",
            },
        }

    camera = {
        name: _fixed_camera(raw["camera"], offset)
        for name, offset in zip(("y_fg", "x_fg", "y_bg", "x_bg"), range(0, 16, 4))
    }
    steps = {
        name: _fixed_camera(raw["camera_step_counters"], offset)
        for name, offset in zip(
            ("x_fg", "y_fg", "x_bg", "y_bg"), range(0, 16, 4)
        )
    }
    hint = raw["h_int_state"]
    hint_address = int.from_bytes(hint[2:6], "big")
    h_int = {
        "jump_word": f"0x{int.from_bytes(hint[0:2], 'big'):04X}",
        "address": f"0x{hint_address:08X}",
        "mode": int.from_bytes(hint[8:10], "big"),
        "split_cursor_bytes": int.from_bytes(hint[10:12], "big"),
        "split_slope": f"0x{int.from_bytes(hint[12:16], 'big'):08X}",
        "split_flags": f"0x{int.from_bytes(hint[16:18], 'big'):04X}",
        "enabled": hint_address != HINT_RTE_ADDRESS,
        "disabled_entry": "HInt = 0x00000758 (RTE)",
    }

    vdp_registers = _optional_region(state, "vdp_registers", 0x20)
    vdp_vsram = _optional_region(state, "vdp_vsram", 0x80)
    vdp_hscroll = _optional_region(state, "vdp_hscroll_table", 0x400)
    hardware: dict[str, Any] = {"present": vdp_registers is not None}
    if vdp_registers is not None:
        registers = list(vdp_registers[0])
        mode = registers[11] & 0x03
        hardware["registers"] = {
            "raw_hex": vdp_registers[0].hex().upper(),
            "hscroll_mode": {0: "full_screen", 1: "per_line", 2: "per_cell"}.get(
                mode, "invalid"
            ),
            "hscroll_mode_bits": mode,
            "hscroll_base_register": f"0x{registers[13] << 10:04X}",
            "vscroll_mode": "full_screen"
            if not (registers[11] & 0x04)
            else "per_2_cell",
        }
    if vdp_vsram is not None:
        vsram_words = _be_words(vdp_vsram[0])
        hardware["vsram"] = {
            "words": [f"0x{word:04X}" for word in vsram_words],
            "plane_a": f"0x{vsram_words[0]:04X}",
            "plane_b": f"0x{vsram_words[1]:04X}",
        }
    if vdp_hscroll is not None:
        hscroll_words = _be_words(vdp_hscroll[0])
        hardware["hscroll_table"] = {
            "base_address": vdp_hscroll[1].get("address"),
            "plane_a": f"0x{hscroll_words[0]:04X}",
            "plane_b": f"0x{hscroll_words[1]:04X}",
            "first_224_line_pairs": [
                {
                    "line": line,
                    "plane_a": f"0x{hscroll_words[line * 2]:04X}",
                    "plane_b": f"0x{hscroll_words[line * 2 + 1]:04X}",
                }
                for line in range(min(SCROLL_LINE_COUNT, len(hscroll_words) // 2))
            ],
        }

    work_h = _optional_region(state, "hscroll_work_buffer", 0x1C0)
    work_v = _optional_region(state, "vsram_shadow", 0x1C0)
    return {
        "present": True,
        "cram_shadow": cram_shadow,
        "provenance": {
            "grand_cross": grand_cross,
            "camera_symbols": "ps4.constants.asm:2346-2349, 16.16 fixed point",
            "camera_step_symbols": "ps4.constants.asm:2161-2164",
            "h_int_symbols": "ps4.constants.asm:2230-2231; ps4.asm:640-750",
            "hscroll_work_symbols": "ps4.asm:670-689 (loc_69E, $FFFF60E0)",
            "split_source_symbols": "ps4.constants.asm:2074-2075; ps4.asm:691-713,736-750",
            "vdp_receipt": "Genesis Plus GX local VDP symbols resolved from the loaded ELF",
        },
        "camera": camera,
        "step_counters": steps,
        "h_int": h_int,
        "hardware": hardware,
        "work_ram": {
            "hscroll_buffer": _line_values(work_h[0]) if work_h else None,
            "vsram_shadow_or_chunk_table": _line_values(work_v[0]) if work_v else None,
        },
        "window_rows": {
            "first_scanline": SCROLL_WINDOW_FIRST_LINE,
            "last_scanline": SCROLL_LINE_COUNT - 1,
            "h_int_split_active": h_int["enabled"],
            "plane_a_source": "HInt2 + Chunk_Table + Camera_Y_Pos_FG"
            if h_int["enabled"]
            else "VDP VSRAM[0]",
            "plane_b_source": "HInt2 + Chunk_Table + Camera_Y_Pos_BG"
            if h_int["enabled"]
            else "VDP VSRAM[1]",
        },
        "placement_provenance": {
            "grand_cross": grand_cross,
            "plane_window_residue_pixels": RETAIL_PLANE_RESIDUE_PIXELS,
            "basis": "MeetingRika frame 7250 normalized plane/window edge comparison after camera and H-int decode",
        },
    }
