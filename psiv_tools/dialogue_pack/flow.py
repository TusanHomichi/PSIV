"""Control codes, typed segments, and `RunText_CharacterLoop` as pagination.

This is the module that knows what an entry *does*: which byte is which
control code, what its operands mean, and where the window breaks.
"""

from __future__ import annotations

import hashlib
from typing import Any, Iterable, Sequence

from ..gfx import DIALOGUE_PORTRAIT_SYMBOLS
from ..text import DIALOGUE, DIALOGUE_CHARSET, TEXT_ACTIONS, decode_string
from .common import DialoguePackError, rom_slice
from .window import CHARS_PER_LINE, LINES_PER_WINDOW

#: Retail names for the sixteen control codes, as the emitted segments call
#: them. `$FB` is `null` here and `extended_event_flag_check` in
#: `psiv_tools.text`: the disassembly's handler is a Grand Cross addition and
#: the retail jump table sends `$FB` to `TextCtrlCode_Null`.
CTRL_NAMES: dict[int, str] = {
    0xF0: "null", 0xF1: "null", 0xF2: "action", 0xF3: "keep_npc_facing",
    0xF4: "portrait", 0xF5: "yes_no", 0xF6: "event", 0xF7: "close",
    0xF8: "null", 0xF9: "delay", 0xFA: "flag_check", 0xFB: "null",
    0xFC: "newline", 0xFD: "wait", 0xFE: "terminate", 0xFF: "terminate",
}

#: What each retail code does, for the consumer that has only the pack.
CTRL_NOTES: dict[int, str] = {
    0xF0: "TextCtrlCode_Null is an rts: it ends the message",
    0xF1: "TextCtrlCode_Null is an rts: it ends the message",
    0xF2: "TextActionsOffs sub-dispatch; see `action`",
    0xF3: "preamble only: the NPC does not turn to face the player",
    0xF4: "load portrait `id` into VRAM $55C and show it; id 0 hides it",
    0xF5: "yes/no window; the chosen operand is an entry id in this tree",
    0xF6: "preamble only: fire event `id` instead of showing the message",
    0xF7: "close the window and yield; the caller resumes after this byte",
    0xF8: "TextCtrlCode_Null is an rts: it ends the message",
    0xF9: "wait `frames` frames",
    0xFA: "if event flag `flag` is set, continue at entry `then_entry`",
    0xFB: "not a retail code: the jump table sends it to TextCtrlCode_Null",
    0xFC: "newline; does not check the two-line limit",
    0xFD: "show the scroll arrow, wait, clear the window, start at line 0",
    0xFE: "end the message",
    0xFF: "end the message; also the entry delimiter",
}

# ---------------------------------------------------------------------------
# Segments
# ---------------------------------------------------------------------------
def _action_segment(out: dict[str, Any], seg: dict[str, Any]) -> None:
    operands = out["operands"]
    out["action"] = seg["action"]
    out["action_id"] = seg["action_id"]
    if seg["action"] == "load_panel":
        out["panel"] = (operands[0] << 8) | operands[1]
    elif seg["action"] in ("load_sound", "load_sound_2"):
        out["sound"] = operands[0]
    elif seg["action"] == "set_event_flag":
        out["flag"] = operands[0]


def control_segment(seg: dict[str, Any]) -> dict[str, Any]:
    """One `psiv_tools.text` control record as a typed renderer segment.

    Every operand byte survives in `operands`, so the typed fields are a
    convenience over the raw bytes rather than a replacement for them.
    """
    code = int(seg["ctrl"], 16)
    if code == 0xFB:
        raise DialoguePackError(
            "control code $FB occurs in the script, but the retail jump table "
            "sends $FB to TextCtrlCode_Null (an rts) and the retail interaction "
            "preamble has no $FB branch; psiv_tools.text decodes it with the "
            "Grand Cross handler's three operand bytes, which would be wrong here"
        )
    operands = list(seg["operands"])
    out: dict[str, Any] = {
        "ctrl": CTRL_NAMES[code],
        "code": seg["ctrl"],
        "operands": operands,
    }
    if code == 0xF2:
        _action_segment(out, seg)
    elif code == 0xF4:
        out["id"] = seg["portrait_id"]
        out["position"] = seg.get("portrait_location")
    elif code == 0xF5:
        out["yes_entry"], out["no_entry"] = operands[0], operands[1]
    elif code == 0xF6:
        out["id"] = (operands[0] << 8) | operands[1]
    elif code == 0xF9:
        # `subq.b #1,d7` then `dbf`, so the operand is the frame count and a
        # zero operand would wrap to 256 rather than not waiting at all.
        out["frames"] = operands[0] or 0x100
    elif code == 0xFA:
        out["scope"] = "event_flag"
        out["flag"], out["then_entry"] = operands[0], operands[1]
    return out


def entry_segments(segments: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        dict(seg) if "text" in seg else control_segment(seg)
        for seg in segments
    ]


# ---------------------------------------------------------------------------
# Pagination
# ---------------------------------------------------------------------------
def _ctrl(segment: dict[str, Any] | None) -> str | None:
    if segment is None or "text" in segment:
        return None
    return segment["ctrl"]


def split_preamble(segments: Sequence[dict[str, Any]]) -> tuple[int, bool]:
    """How much of an entry the interaction code eats, and whether it is an event.

    `entry := $FA* ( $F6 event | $F3? text... )`. The `$FA` run is walked with
    the flags unknown, so what is skipped here is the not-set branch: the entry
    as it reads when no flag sends the text somewhere else.
    """
    index = 0
    while index < len(segments) and _ctrl(segments[index]) == "0xFA":
        index += 1
    if index < len(segments) and _ctrl(segments[index]) == "0xF6":
        return index + 1, True
    if index < len(segments) and _ctrl(segments[index]) == "0xF3":
        index += 1
    return index, False


def paginate(segments: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    """Split one entry into windows, exactly as `RunText_CharacterLoop` does.

    Returns one record per window: the lines it holds and how it ended --
    `full` (two full lines, the arrow appears), `wait` (`$FD`), `close`
    (`$F7`), `choice` (`$F5`) or `end` (the entry ran out).

    Pagination starts at line 0 column 0, which is where every entry the
    interaction code opens begins, and after the preamble the interaction code
    has already consumed. An entry reached by a `$FA` jump does *not* reset the
    counters -- `GetOffsetByID` only moves the text pointer -- so for those the
    pages are what the entry would look like from a fresh window. A `$FA` met
    mid-message is paginated as its not-set branch, for the same reason: which
    way it goes is a runtime fact, not a cartridge one.

    An entry whose preamble ends in `$F6` shows nothing at all -- the event
    fires instead -- and paginates to no windows.

    The input is `psiv_tools.text`'s segment list, which is exactly the byte
    stream regrouped, so "the next byte is `$FC`" is "the next segment is the
    `$FC` control": `decode_string` flushes its run at every control code. The
    entry's own `$FF` terminator is not in the list, so running off the end is
    the same lookahead as reading it.
    """
    pages: list[dict[str, Any]] = []
    lines = [""]
    line = 0
    column = 0
    index, is_event = split_preamble(segments)
    if is_event:
        return []

    def close(end: str) -> None:
        nonlocal lines, line, column
        pages.append({"lines": lines, "end": end})
        lines, line, column = [""], 0, 0

    def peek(offset: int = 0) -> str | None:
        position = index + offset
        return _ctrl(segments[position]) if position < len(segments) else None

    def interrupt(waited: str) -> bool:
        """`TextCtrlCode_Interrupt`, reached from `$FD` and from a full window.

        Returns whether the message ended here. The routine looks at the byte
        it stopped on before it shows anything: `$FF` and `$F7` terminate
        without an arrow, and a `$FD` is swallowed so the window does not wait
        twice for one page.
        """
        nonlocal index
        if index >= len(segments):
            # The next byte is the entry's own $FF terminator.
            close("end")
            return True
        after = peek()
        if after == "0xF7":
            # Terminates the same way $F7 does: the caller resumes past it.
            index += 1
            close("close")
            return False
        if after == "0xFD":
            index += 1
        close(waited)
        return False

    while index < len(segments):
        segment = segments[index]
        index += 1

        if "text" in segment:
            run = segment["text"]
            for position, char in enumerate(run):
                lines[line] += char
                column += 1
                if column < CHARS_PER_LINE:
                    continue
                column = 0
                # The loop peeks at the *next byte*, which is the next segment
                # only when the run ends on this character.
                at_run_end = position == len(run) - 1
                if at_run_end and peek() == "0xF5":
                    index += 1
                    close("choice")
                    return pages
                line += 1
                if at_run_end and peek() == "0xFC":
                    index += 1
                if line % LINES_PER_WINDOW:
                    lines.append("")
                elif not at_run_end:
                    # The next byte is another glyph, so the interrupt has
                    # nothing to swallow.
                    close("full")
                elif interrupt("full"):
                    return pages
            continue

        code = segment["ctrl"]
        if code == "0xFC":
            column = 0
            line += 1
            while len(lines) <= line:
                lines.append("")
        elif code == "0xFD":
            if interrupt("wait"):
                return pages
        elif code == "0xF7":
            close("close")
        elif code in ("0xFE", "0xFF"):
            close("end")
            return pages
        elif code == "0xF5":
            close("choice")
            return pages
        elif code in ("0xF0", "0xF1", "0xF3", "0xF8", "0xFB"):
            # TextCtrlCode_Null is an rts, and the preamble is already gone.
            close("end")
            return pages

    if any(lines):
        pages.append({"lines": lines, "end": "end"})
    return pages



def tree_json(tree: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """One tree's runtime record, and its entries' page lists."""
    entries = []
    for entry in tree["entries"]:
        segments = entry_segments(entry["segments"])
        pages = paginate(entry["segments"])
        entries.append({
            "id": entry["id"],
            "block_offset": entry["block_offset"],
            "text": entry["text"],
            "raw_hex": entry["raw_hex"],
            "segments": segments,
            "pages": pages,
        })
    record = {
        "tree": tree["tree"],
        "label": tree["label"],
        "rom_offset": tree["rom_offset"],
        "rom_end_exclusive": tree["rom_end_exclusive"],
        "compression": tree["compression"],
        "compressed_size": tree["compressed_size"],
        "compressed_sha256": tree["compressed_sha256"],
        "decompressed_size": tree["decompressed_size"],
        "portrait_operand_bytes": tree["portrait_operand_bytes"],
        "is_talk_tree": tree["is_talk_tree"],
        "entry_count": tree["entry_count"],
        "entries": entries,
    }
    return record, entries


# ---------------------------------------------------------------------------
# System messages
#
# `WinTiles_PlayerNothingMsg` sits immediately after the last dialogue tree,
# uncompressed, and holds the eleven lines a party leader says when the player
# presses Talk at nothing. `Interaction_DoPlayerNothingMsg` is the whole
# addressing story in four instructions:
#
#     move.b  (Current_Party_Slots).w, d5
#     lea     (WinTiles_PlayerNothingMsg).l, a0
#     exg     d0, d5
#     jsr     (GetOffsetByID).l
#     exg     d0, d5
#     jsr     (RunText).l
#
# so the leader's character id counts `$FF` terminators from 0x1FE660 exactly
# as a dialogue tree's entry id does, and the message that comes back runs
# through the same `RunText` -- same control codes, same window, same
# pagination. Each of the eleven opens with `$F4` naming that character's own
# portrait, which is the check that the block is what it is claimed to be.
#
# The records that follow (`loc_1FE7E6` onwards, cabinets and graves and the
# rest) are *not* part of this id space: an `even` pad byte sits between them
# and the eleven, and each carries its own label because each is reached by its
# own pointer. Emitting them would mean guessing which caller reaches which,
# so they are left out and reported instead.
# ---------------------------------------------------------------------------
#: `lea (WinTiles_PlayerNothingMsg).l, a0`.
SYSTEM_MESSAGES_ROM_OFFSET = 0x1FE660
#: One per playable character, in `Current_Party_Slots` order.
SYSTEM_MESSAGE_COUNT = 11
SYSTEM_MESSAGES_LABEL = "WinTiles_PlayerNothingMsg"
#: The disassembly's `even` after the eleventh record.
SYSTEM_MESSAGES_ALIGNMENT_PAD = 1


def system_messages(rom: bytes) -> dict[str, Any]:
    """The eleven "nothing here" lines, in the shape a tree entry has.

    Same `segments` and `pages` as an entry, because the cartridge runs them
    through the same routine: a renderer that can draw a dialogue tree can draw
    these with no extra code, and it picks one by the party leader's id.
    """
    offset = SYSTEM_MESSAGES_ROM_OFFSET
    messages = []
    for index in range(SYSTEM_MESSAGE_COUNT):
        end = rom.find(0xFF, offset)
        if end < 0:
            raise DialoguePackError(
                f"{SYSTEM_MESSAGES_LABEL} message {index} at 0x{offset:06X} has no "
                "$FF terminator"
            )
        payload = rom_slice(rom, offset, end - offset, SYSTEM_MESSAGES_LABEL)
        decoded = decode_string(payload, charset=DIALOGUE, portrait_operand_bytes=1)
        segments = entry_segments(decoded["segments"])
        portraits = [s["id"] for s in segments if s.get("ctrl") == "portrait"]
        # Message N belongs to character N and opens with character N's
        # portrait. If that ever stops holding, this is not the block the
        # `lea` points at.
        if portraits[:1] != [index + 1]:
            raise DialoguePackError(
                f"{SYSTEM_MESSAGES_LABEL} message {index} at 0x{offset:06X} opens "
                f"with portrait {portraits[:1]}, not {index + 1}"
            )
        messages.append({
            "id": index,
            "character_id": index,
            "portrait": index + 1,
            "portrait_symbol": DIALOGUE_PORTRAIT_SYMBOLS[index + 1],
            "rom_offset": f"0x{offset:06X}",
            "text": decoded["text"],
            "raw_hex": decoded["raw_hex"],
            "segments": segments,
            "pages": paginate(decoded["segments"]),
        })
        offset = end + 1

    block = rom_slice(
        rom, SYSTEM_MESSAGES_ROM_OFFSET, offset - SYSTEM_MESSAGES_ROM_OFFSET,
        SYSTEM_MESSAGES_LABEL,
    )
    return {
        "label": SYSTEM_MESSAGES_LABEL,
        "rom_offset": f"0x{SYSTEM_MESSAGES_ROM_OFFSET:06X}",
        "rom_end_exclusive": f"0x{offset:06X}",
        "compression": None,
        "size_bytes": len(block),
        "sha256": hashlib.sha256(block).hexdigest(),
        "count": len(messages),
        "charset": DIALOGUE,
        "selector": {
            "routine": "Interaction_DoPlayerNothingMsg",
            "rom_offset": "0x058CE8",
            "ram": "Current_Party_Slots ($FFFFF40A)",
            "addressing": "GetOffsetByID, counting $FF terminators from the label",
            "note": "the id is the party leader's character id, 0..10",
        },
        "alignment_pad_bytes": SYSTEM_MESSAGES_ALIGNMENT_PAD,
        "note": (
            "Uncompressed, unlike the 43 trees, and not part of any of them: "
            "these are the lines the leader says when Talk finds nothing. They "
            "are not counted in the census, which is about the trees. The "
            "records after this block (cabinets, graves, flowers) each have "
            "their own label and pointer and are not in this id space."
        ),
        "messages": messages,
    }


# ---------------------------------------------------------------------------
# Census
# ---------------------------------------------------------------------------
class Census:
    """What the retail script contains, as opposed to what the codes permit."""

    def __init__(self) -> None:
        self.control_codes: dict[str, int] = {}
        self.control_codes_by_tree: dict[int, dict[str, int]] = {}
        self.actions: dict[str, int] = {}
        self.portrait_ids: dict[int, int] = {}
        self.portrait_positions: dict[int, int] = {}
        self.delays: dict[int, int] = {}
        self.line_lengths: dict[int, int] = {}
        self.page_endings: dict[str, int] = {}
        #: Counted from decoded characters rather than raw bytes: a control
        #: code's operands are bytes below $F0 too, and they are not glyphs.
        self.glyph_bytes: dict[str, int] = {}
        self.empty_entries = 0
        self.entries = 0
        self.pages = 0
        self.longest_line = ""
        self.longest_line_at: dict[str, Any] | None = None
        self.preamble_events = 0
        self.preamble_keep_facing = 0
        self.codes_outside_preamble: list[dict[str, Any]] = []
        self.unreachable_tails: list[dict[str, Any]] = []
        self.line_overflow: list[dict[str, Any]] = []

    @staticmethod
    def _bump(counter: dict[Any, int], key: Any) -> None:
        counter[key] = counter.get(key, 0) + 1

    def add_entry(
        self, tree: int, entry: dict[str, Any], segments: Sequence[dict[str, Any]],
        pages: Sequence[dict[str, Any]],
    ) -> None:
        self.entries += 1
        by_tree = self.control_codes_by_tree.setdefault(tree, {})
        if not segments:
            self.empty_entries += 1
        for index, segment in enumerate(segments):
            if "text" in segment:
                for char in segment["text"]:
                    self._bump(self.glyph_bytes, char)
                continue
            code = segment["code"]
            self._bump(self.control_codes, code)
            self._bump(by_tree, code)
            name = segment["ctrl"]
            if name == "action":
                self._bump(self.actions, segment["action"])
            elif name == "portrait":
                self._bump(self.portrait_ids, segment["id"])
                if segment["position"] is not None:
                    self._bump(self.portrait_positions, segment["position"])
            elif name == "delay":
                self._bump(self.delays, segment["frames"])
            elif name in ("event", "keep_npc_facing"):
                # Both are preamble-only: everything before them must be a
                # flag check, or the interaction scanner never reaches them.
                preamble = all(
                    other.get("ctrl") == "flag_check" for other in segments[:index]
                )
                if preamble and name == "event":
                    self.preamble_events += 1
                elif preamble:
                    self.preamble_keep_facing += 1
                else:
                    self.codes_outside_preamble.append({
                        "tree": tree, "entry": entry["id"],
                        "segment": index, "ctrl": name,
                    })
            if name in ("yes_no",) and index != len(segments) - 1:
                self.unreachable_tails.append({
                    "tree": tree, "entry": entry["id"], "segment": index,
                    "ctrl": name, "trailing_segments": len(segments) - index - 1,
                })

        for page in pages:
            self.pages += 1
            self._bump(self.page_endings, page["end"])
            if len(page["lines"]) > LINES_PER_WINDOW:
                self.line_overflow.append({
                    "tree": tree, "entry": entry["id"], "lines": len(page["lines"]),
                })
            for line in page["lines"]:
                self._bump(self.line_lengths, len(line))
                if len(line) > len(self.longest_line):
                    self.longest_line = line
                    self.longest_line_at = {"tree": tree, "entry": entry["id"]}

    def to_json(
        self, portrait_ids: Iterable[int], blank_glyphs: Iterable[int]
    ) -> dict[str, Any]:
        known = set(portrait_ids)
        used = set(self.portrait_ids)
        blank = set(blank_glyphs)
        unused_glyphs = [
            {"byte": byte, "byte_hex": f"0x{byte:02X}", "char": char}
            for byte, char in sorted(DIALOGUE_CHARSET.items())
            if char not in self.glyph_bytes
        ]
        return {
            "entries": self.entries,
            "empty_entries": self.empty_entries,
            "pages": self.pages,
            "control_codes": dict(sorted(self.control_codes.items())),
            "control_codes_by_tree": {
                str(tree): dict(sorted(codes.items()))
                for tree, codes in sorted(self.control_codes_by_tree.items())
            },
            "unused_control_codes": sorted(
                f"0x{code:02X}" for code in CTRL_NAMES
                if f"0x{code:02X}" not in self.control_codes
            ),
            "text_actions": dict(sorted(self.actions.items())),
            "unused_text_actions": sorted(
                spec["name"] for spec in TEXT_ACTIONS.values()
                if spec["name"] not in self.actions
            ),
            "portrait_ids": {
                str(key): value for key, value in sorted(self.portrait_ids.items())
            },
            "portrait_ids_unused": sorted(known - used),
            "portrait_ids_outside_table": sorted(used - known - {0}),
            "portrait_positions": {
                str(key): value for key, value in sorted(self.portrait_positions.items())
            },
            "delay_frames": {
                str(key): value for key, value in sorted(self.delays.items())
            },
            "line_lengths": {
                str(key): value for key, value in sorted(self.line_lengths.items())
            },
            "longest_line": len(self.longest_line),
            "longest_line_text": self.longest_line,
            "longest_line_at": self.longest_line_at,
            "page_endings": dict(sorted(self.page_endings.items())),
            "preamble": {
                "events": self.preamble_events,
                "keep_npc_facing": self.preamble_keep_facing,
                "outside_preamble": self.codes_outside_preamble,
            },
            "unreachable_tails": self.unreachable_tails,
            "lines_past_the_window": self.line_overflow,
            "unused_charset_bytes": unused_glyphs,
            "blank_glyphs_in_charset": [
                {"byte": byte, "byte_hex": f"0x{byte:02X}", "char": char,
                 "used": char in self.glyph_bytes}
                for byte, char in sorted(DIALOGUE_CHARSET.items())
                if byte in blank and char != " "
            ],
            "note": (
                "Observed, not permitted. The script uses 10 of the 16 control "
                "codes and 66 of the 78 charset bytes; $FB is not a retail code "
                "at all; four charset bytes have an empty glyph in the font and "
                "none of the four is ever used; portrait id 0 means hide and is "
                "not a table entry; and no line anywhere exceeds 32 characters "
                "or spills past the window's two lines."
            ),
        }

