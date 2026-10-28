"""Emit the retail Sega/title front door into the ignored runtime pack.

The title screen is a small, self-contained presentation surface: Nemesis art
is loaded into the same VRAM ranges as the cartridge, Enigma mappings place it
on the 320x224 frame, and the four background pieces are concatenated in the
order the retail plane dump proves.  Only provenance and hashes belong in
tracked source; the PNGs and the generated layout JSON are runtime-pack
artifacts and remain ignored.
"""

from __future__ import annotations

import bisect
import hashlib
import json
from pathlib import Path
from typing import Any
import struct
import zlib

from . import m68k, png
from .decode_cache import scoped
from .gfx import decode_palette, palette_rgb
from .planes import (
    MAPPINGS,
    art_tiles,
    compose,
    decode_cells,
    decode_mapping,
    named_cram,
    render,
)

TITLE_DIRECTORY = "title"
PALETTE_CYCLE_NOTE = (
    "Retail CRAM words at Tape 25's frames, derived from the ROM's Sega and "
    "title routines; values are decimal Genesis CRAM words."
)
PALETTE_CYCLE_SOURCE = (
    "ROM: GameMode_Sega, DoSegaPaletteCycling, loc_410E2, Pal_FadeIn, "
    "PalFadeOut_ClrSpriteTbl, TitleRoutine_FadingText, RunTitleFadingText, "
    "loc_427DA, TitleRoutine_PressStartButton, DoPressStartButtonCyclingPal, "
    "loc_42C90 (psiv_tools.title_pack.derive_palette_cycle)"
)
PALETTE_CYCLE_REPLAY = "indexed title surfaces re-encoded against the derived CRAM words"
TITLE_PALETTE = "Pal_TitleScreen"

TITLE_ART_LABELS = {
    "sega_logo": "MapEni_SegaLogo",
    "title_logo": "MapEni_TitlePSTitle",
    "subtitle": "MapEni_TitleTheEndOfTheMillennium",
    "press_start": "MapEni_PressStartButton",
    "copyright": "MapEni_TitleCopyrightText",
}

BACKGROUND_PIECES = (
    ("MapEni_TitleBGLeftPart", 0),
    ("MapEni_TitleBarBGBottomPart", 48),
    ("MapEni_TitleBGRightPart", 112),
)


def _mapping(label: str) -> dict[str, Any]:
    try:
        return next(spec for spec in MAPPINGS if spec["label"] == label)
    except StopIteration as exc:
        raise ValueError(f"title mapping {label!r} is not in planes.MAPPINGS") from exc


def _write_json(path: Path, payload: dict[str, Any]) -> str:
    data = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode("utf-8")
    path.write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def _indexed_png(image: bytes) -> tuple[int, int, bytes, bool]:
    """Read the minimal indexed PNGs this module emits.

    Keeping the pixel indices lets the runtime replay CRAM words instead of
    applying a post-hoc RGB tint, which would change unrelated colours.
    """
    if not image.startswith(png.PNG_SIGNATURE):
        raise ValueError("title replay source is not a PNG")
    cursor = len(png.PNG_SIGNATURE)
    width = height = None
    idat = bytearray()
    transparent = False
    while cursor < len(image):
        size = struct.unpack(">I", image[cursor:cursor + 4])[0]
        kind = image[cursor + 4:cursor + 8]
        payload = image[cursor + 8:cursor + 8 + size]
        cursor += 12 + size
        if kind == b"IHDR":
            width, height, depth, colour_type = struct.unpack(">IIBB", payload[:10])
            if depth != 8 or colour_type != png.COLOR_TYPE_INDEXED:
                raise ValueError("title replay source is not 8-bit indexed PNG")
        elif kind == b"tRNS":
            transparent = bool(payload and payload[0] == 0)
        elif kind == b"IDAT":
            idat += payload
        elif kind == b"IEND":
            break
    if width is None or height is None:
        raise ValueError("title replay source has no dimensions")
    raw = zlib.decompress(bytes(idat))
    stride = width
    pixels = bytearray()
    for row in range(height):
        start = row * (stride + 1)
        if raw[start] != 0:
            raise ValueError("title replay source uses a non-zero PNG filter")
        pixels += raw[start + 1:start + 1 + stride]
    return width, height, bytes(pixels), transparent


# ---------------------------------------------------------------------------
# The CRAM replay, derived from the cartridge
# ---------------------------------------------------------------------------
# Tape 25 (`oracle/tapes/25_title_boot.tape`) samples the front door at these
# 1-based `retro_run()` frames; `title.rs` replays one surface set per frame.
# The words at each frame are computed here from the retail routines that write
# `Palette_Table_Buffer` ($FFFFFB00), read out of the ROM: nothing is copied
# from a capture. The capture under `oracle/layouts/title/` is oracle evidence
# only (`tests/test_title_boot.py` compares the two when it is present).
PALETTE_CYCLE_FRAMES = (
    25, 50, 75, 100, 125, 150, 175, 200, 300, 400, 401, 450, 500, 550, 600, 650,
)

# Tape 25 holds START on these 1-based frames ("4 S" after 400 and after 604
# idle frames). `tests/test_title_boot.py` checks them against the tape.
TAPE25_START_HELD = (range(401, 405), range(605, 609))

# Lag frames: frames on which VInt finds `VInt_Flag` clear (ps4.asm:549-551),
# so `Main_Frame_Count` does not advance and no palette is uploaded, because
# the 68000 is still decompressing art or the DMA queue overran the blanking
# interval. They are cycle timing, which the routines' frame logic cannot
# express, so they are measured: the oracle's per-frame `main_frame_count` on
# tape 25. Keyed by the routine the game entered and the game frame (VInt)
# within it after which the lag falls; 0 is the frames before the routine's
# first wait completes. `GameMode_Sega`'s 10 include power-on and
# `MainGameProgram`'s set-up.
TITLE_LAG_FRAMES: dict[str, dict[int, int]] = {
    "GameMode_Sega": {0: 10, 1: 4},
    "TitleRoutine_FadingText": {0: 49, 1: 2},
    "loc_427DA": {
        0: 15, 3: 1, 5: 1, 7: 1, 9: 1, 11: 1, 13: 1, 14: 1, 15: 1, 17: 1,
        19: 1, 20: 1, 21: 1, 23: 1,
    },
    "loc_42C0E": {0: 1, 1: 2, 2: 2, 3: 2, 4: 2, 5: 2, 6: 2, 7: 2, 8: 1},
}

PALETTE_WORDS = 64
_START_OPCODE = 0x0838  # btst #n, (Joypad_Pressed).w
_JOYPAD_PRESSED = 0xEF03


class TitleCycleError(m68k.DecodeError):
    """The title palette routines were not what retail holds, or the input
    left the paths this model follows."""


def _imm(rom: bytes, at: int, opcode: int, what: str, *, long: bool = False) -> int:
    """The immediate (or address) operand after the opcode word at `at`."""
    m68k.expect(rom, at, opcode, what)
    return m68k.l(rom, at + 2) if long else m68k.w(rom, at + 2)


def _moveq(rom: bytes, at: int, register: int, what: str) -> int:
    word = m68k.w(rom, at)
    if word & 0xFF00 != 0x7000 | register << 9:
        raise TitleCycleError(f"0x{at:06X}: expected {what} (moveq to d{register})")
    return word & 0xFF


def _entry(rom: bytes, at: int, opcode: int, what: str) -> int:
    """The palette entry an absolute-short `$FBxx.w` operand names."""
    operand = _imm(rom, at, opcode, what) - 0xFB00
    if operand & 1 or not 0 <= operand < PALETTE_WORDS * 2:
        raise TitleCycleError(f"0x{at:06X}: {what} is not a palette word")
    return operand // 2


def _expect_start_test(rom: bytes, at: int, what: str) -> None:
    """The loop tests START (`ButtonStart` = 7) in `Joypad_Pressed`, the only
    button tape 25 presses and so the only one this model reads."""
    if _imm(rom, at, _START_OPCODE, what) != 7 or m68k.w(rom, at + 4) != _JOYPAD_PRESSED:
        raise TitleCycleError(f"0x{at:06X}: {what} does not test START in Joypad_Pressed")


def _ror(word: int, count: int) -> int:
    return ((word >> count) | (word << (16 - count))) & 0xFFFF


def _rol(word: int, count: int) -> int:
    return ((word << count) | (word >> (16 - count))) & 0xFFFF


class _TitleBoot:
    """`Palette_Table_Buffer` from power-on to the title menu, one game frame
    at a time.

    `states[k]` is the buffer when the game next waits after VInt `k` (the
    `Main_Frame_Count` value), which is what RAM holds at the end of that
    VInt's frame unless a lag frame follows. `vint_frames[k]` is the 1-based
    tape frame VInt `k` runs on; 0 is power-on.
    """

    def __init__(
        self,
        rom: bytes,
        start_held: tuple[range, ...],
        lag: dict[str, dict[int, int]],
    ) -> None:
        self.rom = rom
        self.start_held = start_held
        self.lag = lag
        self.pal = [0] * PALETTE_WORDS  # MainGameProgram clears RAM (ps4.asm:391-395)
        self.mfc = 0
        self.frame = 0
        self.states: list[tuple[int, ...]] = []
        self.vint_frames = [0]
        self.held = False
        self.pressed = False
        self.routine = ""
        self.routine_mfc = 0
        self.entries: dict[str, int] = {}

    # -- frame plumbing ----------------------------------------------------
    def enter(self, routine: str) -> None:
        self.routine = routine
        self.routine_mfc = self.entries[routine] = self.mfc

    def wait(self) -> None:
        """`VInt_Prepare` (ps4.asm:85745), directly or through a
        `DMAPlane*_VInt`: the buffer is uploaded at the next VInt that finds
        `VInt_Flag` set, which also advances `Main_Frame_Count` and reads the
        pad (ps4.asm:549-640)."""
        self.states.append(tuple(self.pal))
        lag = self.lag.get(self.routine, {}).get(self.mfc - self.routine_mfc, 0)
        self.frame += 1 + lag
        self.mfc += 1
        self.vint_frames.append(self.frame)
        held = any(self.frame in span for span in self.start_held)
        self.pressed = held and not self.held
        self.held = held

    def palette_at(self, frame: int) -> tuple[int, ...]:
        k = bisect.bisect_right(self.vint_frames, frame) - 1
        if k >= len(self.states):
            raise TitleCycleError(f"frame {frame} is past the modelled title routines")
        return self.states[k]

    # -- shared fades ------------------------------------------------------
    def fade_in(self) -> None:
        """`Pal_FadeIn` (ps4.asm:85897-85929): copy the buffer as the target,
        clear it, enable the display (one wait), then step every nibble one
        towards its target per frame until a pass changes nothing."""
        target = list(self.pal)
        self.pal = [0] * PALETTE_WORDS
        self.wait()  # VDP_EnableDisplay
        while True:
            changed = False
            for index, (current, goal) in enumerate(zip(self.pal, target)):
                for _ in range(4):
                    nibble = current & 0xF
                    if nibble != goal & 0xF:
                        changed = True
                        current = (current & 0xFFF0) | (nibble + 1)
                    current = _ror(current, 4)
                    goal = _ror(goal, 4)
                self.pal[index] = current
            self.wait()
            if not changed:
                return

    def fade_out(self) -> None:
        """`PalFadeOut_ClrSpriteTbl` (ps4.asm:85953-85972): step every nibble
        one towards zero per frame until a pass leaves the buffer black."""
        while True:
            seen = 0
            for index, current in enumerate(self.pal):
                for _ in range(4):
                    nibble = current & 0xF
                    current &= 0xFFF0
                    if nibble:
                        nibble -= 1
                    current = _rol(current | nibble, 4)
                self.pal[index] = current
                seen |= current
            self.wait()
            if not seen:
                return

    # -- GameMode_Sega -----------------------------------------------------
    def sega(self) -> None:
        rom = self.rom
        self.enter("GameMode_Sega")
        self.fade_out()
        # ps4.asm:84287-84291: fill the whole table with one long.
        m68k.expect(rom, 0x4107A, 0x43F8, "lea (Palette_Table_Buffer).w, a1")
        longs = _moveq(rom, 0x4107E, 7, "moveq #$1F, d7") + 1
        fill = _imm(rom, 0x41080, 0x203C, "move.l #$0E000E00, d0", long=True)
        self.pal[:longs * 2] = [fill >> 16, fill & 0xFFFF] * longs
        self.fade_in()
        count = _imm(rom, 0x410A6, 0x323C, "move.w #$B3, d1")
        _expect_start_test(rom, 0x410C4, "btst #ButtonStart (GameMode_SegaLoop)")
        m68k.expect(rom, 0x410CA, 0x56C9, "dbne d1, GameMode_SegaLoop")
        timer = cycle = 0  # clr.l (RAM_Start).l, ps4.asm:84302
        while True:  # GameMode_SegaLoop, ps4.asm:84304-84316
            timer, cycle = self.sega_cycle(timer, cycle)
            self.wait()
            if self.pressed:
                break
            count -= 1
            if count < 0:
                break
        self.sega_exit_palette()
        self.wait()
        self.fade_out()

    def sega_cycle(self, timer: int, cycle: int) -> tuple[int, int]:
        """`DoSegaPaletteCycling` (ps4.asm:84349-84411): every `period`
        calls, advance one step of the logo's fade, shine and white-in."""
        rom = self.rom
        period = _imm(rom, 0x41114, 0x0C79, "cmpi.w #5, ($FFFF0002).l")
        if timer == period:
            fade_end = _imm(rom, 0x41126, 0x0C40, "cmpi.w #8, d0")
            shine_end = _imm(rom, 0x41142, 0x0C40, "cmpi.w #$E, d0")
            white_end = _imm(rom, 0x41198, 0x0C40, "cmpi.w #$15, d0")
            acted = True
            if cycle < fade_end:
                table = _imm(rom, 0x4112C, 0x41F9, "lea (loc_411B8).l, a0", long=True)
                m68k.expect(rom, 0x41138, 0x3341, "move.w d1, 0(a1)")
                m68k.expect(rom, 0x4113C, 0x3341, "move.w d1, 2(a1)")
                first, second = m68k.w(rom, 0x4113A) // 2, m68k.w(rom, 0x4113E) // 2
                self.pal[first] = self.pal[second] = rom[table + cycle] << 8
            elif cycle < shine_end:
                m68k.expect(rom, 0x41148, 0x5140, "subq.w #8, d0")
                step = cycle - 8
                table = _imm(rom, 0x4114E, 0x41F9, "lea (loc_411C0).l, a0", long=True)
                entry = _imm(rom, 0x41154, 0x45E9, "lea $4(a1), a2") // 2
                count = _moveq(rom, 0x41158, 7, "moveq #4, d7") + 1
                index = step + 1
                for offset in range(count):
                    self.pal[entry + offset] = rom[table + index] << 8
                    index = max(index - 1, 0)
                table = _imm(rom, 0x41174, 0x41F9, "lea (loc_411C8).l, a0", long=True)
                entry = _imm(rom, 0x4117A, 0x45E9, "lea $1A(a1), a2") // 2
                count = _moveq(rom, 0x4117E, 7, "moveq #5, d7") + 1
                index = step + 1
                for offset in range(count):
                    word = self.pal[entry - offset]
                    self.pal[entry - offset] = (word & 0xFF00) | rom[table + index]
                    index = max(index - 1, 0)
            elif cycle < white_end:
                add = _imm(rom, 0x4119E, 0x0669, "addi.w #$222, 2(a1)")
                entry = m68k.w(rom, 0x411A2) // 2
                self.pal[entry] = (self.pal[entry] + add) & 0xFFFF
            else:
                acted = False
            if acted:
                cycle += 1
                timer = 0
        return timer + 1, cycle

    def sega_exit_palette(self) -> None:
        """`loc_410E2` (ps4.asm:84324-84344): black, white, black, then two
        ramps, then two blacks across line 0."""
        rom = self.rom
        m68k.expect(rom, 0x410E2, 0x41F8, "lea (Palette_Table_Buffer).w, a0")
        white = _imm(rom, 0x410EA, 0x30FC, "move.w #$EEE, (a0)+")
        ramp = _imm(rom, 0x410F0, 0x323C, "move.w #$200, d1")
        first = _moveq(rom, 0x410F4, 7, "moveq #5, d7") + 1
        first_step = _imm(rom, 0x410F6, 0x0641, "addi.w #$200, d1")
        second = _moveq(rom, 0x41100, 7, "moveq #4, d7") + 1
        second_step = _imm(rom, 0x41104, 0x0641, "addi.w #$20, d1")
        m68k.expect(rom, 0x4110C, 0x20C0, "move.l d0, (a0)+")
        words = [0, white, 0]
        for _ in range(first):
            ramp = (ramp + first_step) & 0xFFFF
            words.append(ramp)
        for _ in range(second):
            words.append(ramp)
            ramp = (ramp + second_step) & 0xFFFF
        words += [0, 0]
        self.pal[:len(words)] = words

    # -- GameMode_Title ----------------------------------------------------
    def title(self) -> None:
        rom = self.rom
        self.enter("TitleRoutine_FadingText")
        # ps4.asm:86255-86260: Pal_TitleScreen into the buffer.
        source = _imm(rom, 0x4255C, 0x41F9, "lea (Pal_TitleScreen).l, a0", long=True)
        words = _imm(rom, 0x42566, 0x303C, "move.w #$3F, d0") + 1
        self.pal[:words] = [m68k.w(rom, source + 2 * i) for i in range(words)]
        self.wait()  # DMAPlanes_VInt
        # CredText_A is clear on a power-on boot, so ps4.asm:86322 falls
        # through and blacks the two text entries before the fade.
        m68k.expect(rom, 0x42610, 0x08B8, "bclr #0, (CredText_A).w")
        m68k.expect(rom, 0x4261A, 0x7000, "moveq #0, d0")
        text_entries = (
            _entry(rom, 0x4261C, 0x31C0, "move.w d0, (Palette_Table_Buffer+$3E).w"),
            _entry(rom, 0x42620, 0x31C0, "move.w d0, (Palette_Table_Buffer+$5E).w"),
        )
        for entry in text_entries:
            self.pal[entry] = 0
        self.wait()  # DMAPlane_B_VInt
        self.fade_in()
        self.fading_text(text_entries)
        self.reveal()
        self.press_start()
        self.enter_menu()

    def fading_text(self, text_entries: tuple[int, int]) -> None:
        """`TitleRoutine_FadingTextLoop` (ps4.asm:86333-86377) until START."""
        rom = self.rom
        _expect_start_test(rom, 0x4263C, "btst #ButtonStart (TitleRoutine_FadingTextLoop)")
        last = _imm(rom, 0x42646, 0x0C78, "cmpi.w #$E0, (TextCounter).w")
        scroll = _imm(rom, 0x42534, 0x31FC, "move.w #$FF20, (Camera_Y_Pos_BG).w") << 16
        speed = _imm(rom, 0x42654, 0x06B8, "addi.l #$2280, (Camera_Y_Pos_BG).w", long=True)
        bit = _imm(rom, 0x42662, 0x0800, "btst #$10, d0")
        records = _imm(rom, 0x429E6, 0x41F9, "lea (Title_FadingTextData).l, a0", long=True)
        counter = control = 0
        while not self.pressed:
            if counter == last:
                raise TitleCycleError(
                    "TextCounter reached the end of the fading text (loc_426E0), "
                    "which this model does not follow"
                )
            moved = scroll ^ ((scroll + speed) & 0xFFFFFFFF)
            scroll = (scroll + speed) & 0xFFFFFFFF
            if moved >> bit & 1:
                counter += 1
            self.wait()  # DMAPlane_A_VInt, DMAPlane_B_VInt or VInt_Prepare
            control = self.read_fading_text(records, counter, control)
            control = self.run_fading_text(text_entries, control)

    def read_fading_text(self, records: int, counter: int, control: int) -> int:
        """`ReadTitleFadingTextData` (ps4.asm:86672-86697): the first record
        whose first byte is TextCounter's low byte sets its FadeControl bit
        through `Title_FadingTextRoutines` (ps4.asm:86700-86707)."""
        at = records
        while True:
            key = self.rom[at]
            if key == counter & 0xFF:
                routine = self.rom[at + 1]
                if routine > 5:
                    raise TitleCycleError(f"0x{at:06X}: fading-text routine {routine}")
                # 0/1 draw a line and set bit 0; 2-5 set bits 2-5.
                return control | (1 if routine < 2 else 1 << routine)
            if key == 0xFF:
                return control
            at += 3

    def run_fading_text(self, text_entries: tuple[int, int], control: int) -> int:
        """`RunTitleFadingText` (ps4.asm:86811-86918): every eighth frame, step
        the line's text colour by $222 in or out."""
        rom = self.rom
        mask = _imm(rom, 0x42B22, 0x0241, "andi.w #7, d1")
        step_in = _imm(rom, 0x42B2A, 0x0641, "addi.w #$222, d1")
        full = _imm(rom, 0x42B2E, 0x0801, "btst #$C, d1")
        step_out = _imm(rom, 0x42B74, 0x0441, "subi.w #$222, d1")
        snapshot = control
        if self.mfc & 0xFF & mask:
            return control
        for line, entry in enumerate(text_entries):
            if snapshot >> (2 + line) & 1:
                word = (self.pal[entry] + step_in) & 0xFFFF
                if word >> full & 1:
                    control &= ~(1 << (2 + line))
                else:
                    self.pal[entry] = word
        for line, entry in enumerate(text_entries):
            if snapshot >> (4 + line) & 1:
                if self.pal[entry]:
                    self.pal[entry] = (self.pal[entry] - step_out) & 0xFFFF
                else:
                    control = (control & ~(1 << (4 + line))) | 1
        return control

    def reveal(self) -> None:
        """`loc_427DA` to `loc_4295C` (ps4.asm:86463-86630): white text
        entries, the title art's masked reveal, then the prompt's planes."""
        rom = self.rom
        self.enter("loc_427DA")
        white = _imm(rom, 0x427E6, 0x303C, "move.w #$EEE, d0")
        for at in (0x427EA, 0x427EE):
            self.pal[_entry(rom, at, 0x31C0, "move.w d0, (Palette_Table_Buffer+n).w")] = white
        self.wait()  # DMAPlanes_VInt
        self.wait()  # VDP_EnableDisplay
        last = _imm(rom, 0x428E8, 0x0C00, "cmpi.b #$18, d0")
        step = 0
        while True:  # loc_428E0
            step += 1
            if step == last:
                break
            self.wait()  # DMAPlane_A_VInt
        self.wait()  # loc_4295C: jmp (DMAPlanes_VInt).l

    def press_start(self) -> None:
        """`TitleRoutine_PressStartButton` (ps4.asm:86920-86945) with
        `DoPressStartButtonCyclingPal` (ps4.asm:87014-87028)."""
        rom = self.rom
        self.enter("TitleRoutine_PressStartButton")
        count = _imm(rom, 0x42BDE, 0x303C, "move.w #$233, d0")
        _expect_start_test(rom, 0x42BF0, "btst #ButtonStart (PressStartButtonLoop)")
        table = _imm(rom, 0x42CC4, 0x45F9, "lea (PressStartButtonCyclingPal).l, a2", long=True)
        every = _imm(rom, 0x42CD0, 0x0241, "andi.w #3, d1")
        phase = _imm(rom, 0x42CD6, 0x0240, "andi.w #$7C, d0")
        m68k.expect(rom, 0x42CDA, 0xE248, "lsr.w #1, d0")
        entry = _imm(rom, 0x42CE0, 0x3352, "move.w (a2), $4A(a1)") // 2
        while True:
            if not self.mfc & every:
                self.pal[entry] = m68k.w(rom, table + ((self.mfc & phase) >> 1))
            self.wait()
            if self.pressed:
                return
            count -= 1
            if count < 0:
                raise TitleCycleError(
                    "the prompt timed out into TitleRoutine_CharPortraits, which "
                    "this model does not follow"
                )

    def enter_menu(self) -> None:
        """`loc_42C0E` to `loc_42C90` (ps4.asm:86947-87012): the prompt
        dissolves, Pal_Init_Line_3 replaces line 2, and
        `TitleRoutine_PickOption` takes over without writing a palette word
        on the no-save path (ps4.asm:87455-87586)."""
        rom = self.rom
        self.enter("loc_42C0E")
        last = _imm(rom, 0x42C24, 0x0C00, "cmpi.b #9, d0")
        step = 0
        while True:  # loc_42C1C
            step += 1
            if step == last:
                break
            self.wait()  # DMAPlane_A_VInt
        source = _imm(rom, 0x42C96, 0x41F9, "lea (Pal_Init_Line_3).l, a0", long=True)
        entry = _entry(rom, 0x42C9C, 0x43F8, "lea (Palette_Line_3).w, a1")
        words = _moveq(rom, 0x42CA0, 7, "moveq #$F, d7") + 1
        self.pal[entry:entry + words] = [m68k.w(rom, source + 2 * i) for i in range(words)]
        loops = _moveq(rom, 0x42CB2, 0, "moveq #9, d0") + 1
        for _ in range(loops):  # DoDMAPlanesLoop clears Joypad_Held each pass
            self.held = False
            self.wait()
        self.wait()  # jmp (VInt_Prepare).l
        self.states.append(tuple(self.pal))


def run_title_boot(
    rom: bytes,
    *,
    start_held: tuple[range, ...] = TAPE25_START_HELD,
    lag: dict[str, dict[int, int]] | None = None,
) -> _TitleBoot:
    """Power-on to the no-save title menu under tape 25's input."""
    boot = _TitleBoot(rom, start_held, TITLE_LAG_FRAMES if lag is None else lag)
    boot.sega()
    boot.title()
    return boot


def derive_palette_cycle(
    rom: bytes,
    frames: tuple[int, ...] = PALETTE_CYCLE_FRAMES,
    *,
    start_held: tuple[range, ...] = TAPE25_START_HELD,
    lag: dict[str, dict[int, int]] | None = None,
) -> dict[str, Any]:
    """The four CRAM lines at each sampled tape-25 frame, from the ROM."""
    boot = run_title_boot(rom, start_held=start_held, lag=lag)
    sampled: dict[str, Any] = {}
    previous: tuple[int, ...] | None = None
    for frame in frames:
        words = boot.palette_at(frame)
        sampled[str(frame)] = {
            "lines": {
                str(line): list(words[line * 16:(line + 1) * 16]) for line in range(4)
            },
            "changed_entries_from_previous": []
            if previous is None
            else [index for index, (old, new) in enumerate(zip(previous, words)) if old != new],
        }
        previous = words
    return {
        "format_version": 1,
        "kind": "psiv_title_palette_cycle",
        "line_count": 4,
        "colors_per_line": 16,
        "note": PALETTE_CYCLE_NOTE,
        "frames": sampled,
    }


def _cycle_colours(frame: dict[str, Any]) -> list[tuple[int, int, int]]:
    raw = b"".join(
        int(word).to_bytes(2, "big")
        for line in range(4)
        for word in frame["lines"][str(line)]
    )
    return palette_rgb(decode_palette(raw))


def _emit_replay_variants(
    root: Path,
    cycle: dict[str, Any],
    assets: dict[str, dict[str, Any]],
    background: dict[str, Any],
    transfer: dict[str, Any],
) -> dict[str, Any]:
    """Re-encode every title surface against each sampled CRAM frame."""
    sources = {
        "background": (background["png"], False),
        "background_transfer": (transfer["png"], False),
        **{
            name: (asset["png"], True)
            for name, asset in assets.items()
        },
    }
    frames: dict[str, Any] = {}
    for frame_name, frame in sorted(cycle["frames"].items(), key=lambda pair: int(pair[0])):
        colours = _cycle_colours(frame)
        frame_dir = root / TITLE_DIRECTORY / "replay" / f"frame_{frame_name}"
        frame_dir.mkdir(parents=True, exist_ok=True)
        for name, (relative, transparent) in sources.items():
            source = root / relative
            width, height, pixels, source_transparent = _indexed_png(source.read_bytes())
            output = frame_dir / Path(relative).name
            output.write_bytes(
                png.encode_indexed(
                    width,
                    height,
                    pixels,
                    colours,
                    transparent=(0,) if transparent or source_transparent else (),
                )
            )
        frames[frame_name] = {
            "changed_entries_from_previous": frame["changed_entries_from_previous"],
            "directory": f"title/replay/frame_{frame_name}",
        }
    path = root / TITLE_DIRECTORY / "palette_cycle.json"
    cycle_payload = {
        **cycle,
        "frames": frames,
        "source": PALETTE_CYCLE_SOURCE,
        "replay": PALETTE_CYCLE_REPLAY,
    }
    cycle_sha = _write_json(path, cycle_payload)
    return {
        "path": str(path.relative_to(root)),
        "sha256": cycle_sha,
        "frames": [int(frame) for frame in sorted(frames, key=int)],
        "surface_count": len(sources),
    }


def _decode_mapping_asset(
    rom: bytes,
    spec: dict[str, Any],
    cram: list[tuple[int, int, int]],
    path: Path,
    *,
    transparent: bool,
) -> dict[str, Any]:
    words, mapping = decode_mapping(
        rom,
        spec["rom_offset"],
        spec["label"],
        base_tile=spec["base_tile"],
        compressed_size=spec["compressed_size"],
        expected_words=spec["columns"] * spec["rows"],
    )
    tiles, art = art_tiles(rom, spec["art"])
    image = render(
        decode_cells(words),
        spec["columns"],
        tiles,
        cram,
        spec["art_vram_tile"],
        transparent_backdrop=transparent,
    )
    path.write_bytes(image)
    mapping = {
        **mapping,
        "source": spec["source"],
        "columns": spec["columns"],
        "rows": spec["rows"],
        "art_label": spec["art"],
        "art_vram_tile": f"0x{spec['art_vram_tile']:03X}",
        "art_tile_count": len(tiles),
        "spans_art_exactly": mapping["tile_range"]
        == [spec["art_vram_tile"], spec["art_vram_tile"] + len(tiles) - 1],
    }
    if "note" in spec:
        mapping["note"] = spec["note"]
    return {
        "png": str(path.relative_to(path.parents[1])),
        "png_sha256": png.image_sha256(image),
        "size_pixels": [spec["columns"] * 8, spec["rows"] * 8],
        "mapping": mapping,
        "art": art,
    }


def _background(
    rom: bytes,
    cram: list[tuple[int, int, int]],
    root: Path,
) -> tuple[dict[str, Any], dict[str, dict[str, Any]], dict[str, Any]]:
    canvas = bytearray(320 * 224)
    pieces: dict[str, dict[str, Any]] = {}
    placements = []
    for label, x in BACKGROUND_PIECES:
        spec = _mapping(label)
        words, mapping = decode_mapping(
            rom,
            spec["rom_offset"],
            spec["label"],
            base_tile=spec["base_tile"],
            compressed_size=spec["compressed_size"],
            expected_words=spec["columns"] * spec["rows"],
        )
        tiles, art = art_tiles(rom, spec["art"])
        width, height, pixels = compose(
            decode_cells(words), spec["columns"], tiles, spec["art_vram_tile"]
        )
        if height != 224 or x + width > 320:
            raise ValueError(
                f"title background piece {label} is {width}x{height} at x={x}, "
                "not a slice of the 320x224 frame"
            )
        for row in range(height):
            start = row * width
            destination = row * 320 + x
            canvas[destination:destination + width] = pixels[start:start + width]
        piece_name = label.removeprefix("MapEni_").lower() + ".png"
        piece_path = root / TITLE_DIRECTORY / piece_name
        piece_image = png.encode_indexed(width, height, pixels, cram)
        piece_path.write_bytes(piece_image)
        piece_key = label.removeprefix("MapEni_")
        pieces[piece_key] = {
            "png": str(piece_path.relative_to(root)),
            "png_sha256": png.image_sha256(piece_image),
            "size_pixels": [width, height],
            "mapping": {
                **mapping,
                "source": spec["source"],
                "columns": spec["columns"],
                "rows": spec["rows"],
                "art_label": spec["art"],
                "art_vram_tile": f"0x{spec['art_vram_tile']:03X}",
                "art_tile_count": len(tiles),
                "spans_art_exactly": mapping["tile_range"]
                == [spec["art_vram_tile"], spec["art_vram_tile"] + len(tiles) - 1],
            },
            "art": art,
        }
        placements.append({
            "piece": piece_key,
            "x_pixels": x,
            "y_pixels": 0,
            "width_pixels": width,
            "height_pixels": height,
        })

    # The title routine first places this 8-cell strip at x=6 while the
    # background transfer is still in flight.  It is a real Enigma mapping,
    # not a crop of the settled planet, and is what the oracle sees at frames
    # 300 and 400.
    transfer_path = root / TITLE_DIRECTORY / "titlebarbgtoppart.png"
    transfer = _decode_mapping_asset(
        rom,
        _mapping("MapEni_TitleBarBGTopPart"),
        cram,
        transfer_path,
        transparent=False,
    )
    image = png.encode_indexed(320, 224, bytes(canvas), cram)
    path = root / TITLE_DIRECTORY / "background.png"
    path.write_bytes(image)
    return {
        "png": str(path.relative_to(root)),
        "png_sha256": png.image_sha256(image),
        "size_pixels": [320, 224],
        "placements": placements,
        "source": "plane_b at title frame 401: left + bottom + right",
    }, pieces, transfer


@scoped
def emit_title(rom_bytes: bytes, out_dir: str | Path) -> dict[str, Any]:
    """Emit title art, its composed background, and numeric provenance."""
    root = Path(out_dir)
    title_root = root / TITLE_DIRECTORY
    title_root.mkdir(parents=True, exist_ok=True)
    cram = named_cram(rom_bytes, TITLE_PALETTE)
    assets: dict[str, dict[str, Any]] = {}
    for name, label in TITLE_ART_LABELS.items():
        spec = _mapping(label)
        path = title_root / f"{name}.png"
        assets[name] = _decode_mapping_asset(
            rom_bytes, spec, cram, path, transparent=True
        )

    background, pieces, background_transfer = _background(rom_bytes, cram, root)
    cycle = derive_palette_cycle(rom_bytes)
    replay = _emit_replay_variants(
        root,
        cycle,
        assets,
        background,
        background_transfer,
    )
    layout = {
        "format_version": 1,
        "kind": "psiv_title_pack",
        "screen": {"width_pixels": 320, "height_pixels": 224},
        "palette": {
            "name": TITLE_PALETTE,
            "cram_lines": [
                [list(colour) for colour in cram[line * 16:(line + 1) * 16]]
                for line in range(4)
            ],
        },
        "assets": assets,
        "background": background,
        "background_pieces": pieces,
        "background_transfer": background_transfer,
        "palette_cycle": replay,
        "placement_contract": {
            "sega_logo": {"x_cell": 12, "y_cell": 11, "width_cells": 17, "height_cells": 5},
            "title_logo": {"x_cell": 11, "y_cell": 3, "width_cells": 17, "height_cells": 13},
            "subtitle": {"x_cell": 5, "y_cell": 17, "width_cells": 29, "height_cells": 3},
            "press_start": {"x_cell": 11, "y_cell": 22, "width_cells": 18, "height_cells": 1},
            "copyright": {"x_cell": 12, "y_cell": 25, "width_cells": 17, "height_cells": 1},
        },
    }
    layout_path = title_root / "layout.json"
    layout_sha = _write_json(layout_path, layout)
    return {
        "format_version": 1,
        "directory": TITLE_DIRECTORY,
        "layout": {
            "path": str(layout_path.relative_to(root)),
            "sha256": layout_sha,
            "screen": [320, 224],
            "asset_count": len(assets),
        },
        "background": {
            "png": background["png"],
            "png_sha256": background["png_sha256"],
            "pieces": len(pieces),
        },
        "background_transfer": {
            "png": background_transfer["png"],
            "png_sha256": background_transfer["png_sha256"],
            "size_pixels": background_transfer["size_pixels"],
        },
        "palette_cycle": replay,
        "assets": {
            name: {
                "png": asset["png"],
                "png_sha256": asset["png_sha256"],
                "size_pixels": asset["size_pixels"],
            }
            for name, asset in assets.items()
        },
    }
