"""NPC movement commands: what a scene's `MoveActorCommand` byte means.

A scene op hands the NPC movement path a command index and a speed selector,
and the path turns that pair into a step. Both come from one table, so this is
data extraction rather than a transcription of behaviour -- but the table is
reached only from code, so the code is what pins it.

`FieldObj_NPCMove` (`0x04A15E`), the whole of the lookup:

    tst.w   x_step_duration(a4)     ; still mid-step? then nothing happens
    bne.w   loc_4A200
    tst.w   y_step_duration(a4)
    bne.w   loc_4A200
    lea     (loc_4A202).l, a1       ; three longs, one per speed
    add.w   d7, d7
    add.w   d7, d7
    movea.l (a1,d7.w), a1           ; d7 selects the speed table
    ...
    move.w  d0, d2                  ; d0 is the command index
    lsl.w   #3, d2                  ; eight bytes per command

so there are two indexes and they are independent: `d7` picks how fast, `d0`
picks which way. The three speed tables hold the same eleven commands and
differ only in velocity.

The eight-byte record, field for field, from `loc_4A1AC`:

    +0  byte   x distance in pixels   -> x_step_duration, scaled by 256
    +1  byte   y distance in pixels   -> y_step_duration, scaled by 256
    +2  word   x velocity, signed     -> x_step_constant, scaled by 256
    +4  word   y velocity, signed     -> y_step_constant, scaled by 256
    +6  byte   facing direction       -> facing_dir, unless negative
    +7  byte   unused; zero in every retail record

`FieldObj_UpdateStepDuration` closes the loop: it subtracts `|constant| >> 8`
from the duration every frame, so a command lasts `distance * 256 / |velocity|`
frames and covers `distance` pixels. Every moving command in the cartridge
covers exactly 16 pixels, which is one collision cell.

Two behaviours worth carrying into an engine, both from the same routine:

* A command whose velocity is zero on both axes is a genuine stand-still, and
  it also resets the animation (`clr.b mappings_idx`, `mappings_duration = 1`).
* When the move is refused -- `loc_4A188`, reached when the pre-checks or the
  collision test fail -- the NPC still takes the record's facing and still
  resets its animation. A blocked NPC turns on the spot rather than doing
  nothing.

Neither index is bounds-checked. The speed table has three entries and the
command tables eleven, and this module derives both counts from the table
extents rather than asserting them, so a command id past the end is a caller
bug the pack can describe rather than a number this module invented.
"""

from __future__ import annotations

import hashlib
import re
import struct
from dataclasses import dataclass
from typing import Any

#: `lea (abs).l,a1 / add.w d7,d7 / add.w d7,d7 / movea.l (a1,d7.w),a1`, the
#: only place the speed table is named. The capture is the table address.
_DISPATCH = re.compile(
    b"\x43\xf9(....)\xde\x47\xde\x47\x22\x71\x70\x00", re.DOTALL
)

#: `lsl.w #3,d2` on the command index.
RECORD_BYTES = 8

#: `FieldObj_UpdateStepDuration`: `asr.l #8` on the constant before it is
#: subtracted from the duration, and the duration was the distance byte shifted
#: left by 8. Both scalings are the same 256.
FIXED_POINT = 256

#: `Map_Start_Facing_Dir`'s values, and the `bmi` that means "leave it alone".
FACING_NAMES = {0x0: "down", 0x4: "up", 0x8: "right", 0xC: "left"}
FACING_UNCHANGED = 0xFF

#: The pack's collision cell, for reporting a command's reach in cells.
COLLISION_CELL_PIXELS = 16


class NpcCommandError(ValueError):
    pass


@dataclass(frozen=True)
class Command:
    """One eight-byte movement record."""

    index: int
    raw: bytes
    x_pixels: int
    y_pixels: int
    x_velocity: int
    y_velocity: int
    facing: int
    unused: int

    @property
    def moves(self) -> bool:
        return bool(self.x_velocity or self.y_velocity)

    @property
    def frames(self) -> int | None:
        """How long the step takes, from the routine's own arithmetic."""
        for distance, velocity in ((self.x_pixels, self.x_velocity),
                                   (self.y_pixels, self.y_velocity)):
            if velocity:
                return distance * FIXED_POINT // abs(velocity)
        return None

    @property
    def direction(self) -> str | None:
        """Which way the step goes, or `None` for a stand-still."""
        if not self.moves:
            return None
        if self.y_velocity:
            return "down" if self.y_velocity > 0 else "up"
        return "right" if self.x_velocity > 0 else "left"

    def to_json(self) -> dict[str, Any]:
        return {
            "id": self.index,
            "raw_hex": self.raw.hex(),
            "moves": self.moves,
            "direction": self.direction,
            "x_pixels": self.x_pixels,
            "y_pixels": self.y_pixels,
            "x_cells": self.x_pixels // COLLISION_CELL_PIXELS,
            "y_cells": self.y_pixels // COLLISION_CELL_PIXELS,
            "x_velocity": self.x_velocity,
            "y_velocity": self.y_velocity,
            "frames": self.frames,
            "facing": {
                "byte": self.facing,
                "name": FACING_NAMES.get(self.facing),
                "unchanged": self.facing == FACING_UNCHANGED,
            },
        }


@dataclass(frozen=True)
class SpeedTable:
    """One `d7` selection: the same commands at one speed."""

    index: int
    rom_offset: int
    commands: tuple[Command, ...]
    sha256: str

    @property
    def rom_end(self) -> int:
        return self.rom_offset + len(self.commands) * RECORD_BYTES

    @property
    def velocity(self) -> int:
        """The magnitude every moving command in this table shares."""
        magnitudes = {
            abs(v) for command in self.commands
            for v in (command.x_velocity, command.y_velocity) if v
        }
        if len(magnitudes) != 1:
            raise NpcCommandError(
                f"speed table {self.index} mixes velocities {sorted(magnitudes)}; "
                "a table is supposed to be one speed"
            )
        return magnitudes.pop()

    def to_json(self) -> dict[str, Any]:
        frames = {command.frames for command in self.commands if command.moves}
        return {
            "selector": self.index,
            "rom_offset": f"0x{self.rom_offset:06X}",
            "rom_end": f"0x{self.rom_end:06X}",
            "sha256": self.sha256,
            "velocity": self.velocity,
            "pixels_per_frame": self.velocity / FIXED_POINT,
            "frames_per_cell": sorted(frames),
            "commands": [command.to_json() for command in self.commands],
        }


def _decode(index: int, raw: bytes) -> Command:
    x_pixels, y_pixels = raw[0], raw[1]
    x_velocity, y_velocity = struct.unpack(">hh", raw[2:6])
    command = Command(
        index=index, raw=raw, x_pixels=x_pixels, y_pixels=y_pixels,
        x_velocity=x_velocity, y_velocity=y_velocity, facing=raw[6], unused=raw[7],
    )
    if command.facing != FACING_UNCHANGED and command.facing not in FACING_NAMES:
        raise NpcCommandError(
            f"command {index} faces 0x{command.facing:02X}, which is neither a "
            "direction nor the negative that leaves facing alone"
        )
    if bool(x_velocity) and bool(y_velocity):
        raise NpcCommandError(
            f"command {index} moves on both axes; the routine only ever produces "
            "one step at a time"
        )
    for distance, velocity, axis in ((x_pixels, x_velocity, "x"),
                                     (y_pixels, y_velocity, "y")):
        if bool(distance) != bool(velocity):
            raise NpcCommandError(
                f"command {index} has {axis} distance {distance} and velocity "
                f"{velocity}; one without the other never terminates or never moves"
            )
        if velocity and distance * FIXED_POINT % abs(velocity):
            raise NpcCommandError(
                f"command {index}'s {axis} step is {distance} pixels at "
                f"{velocity}/256 per frame, which is not a whole number of frames"
            )
    return command


def dispatch_site(rom: bytes) -> tuple[int, int]:
    """Where `FieldObj_NPCMove` names the speed table, and what it names.

    Returned as `(site, table)`. The site is checked to be unique, so the table
    address is the cartridge's own rather than a constant this module carries.
    """
    matches = list(_DISPATCH.finditer(rom))
    if len(matches) != 1:
        raise NpcCommandError(
            f"the NPC movement dispatch matches {len(matches)} times, not once"
        )
    match = matches[0]
    return match.start(), int.from_bytes(match.group(1), "big")


def extract_npc_commands(rom: bytes) -> dict[str, Any]:
    """The movement-command tables, as the pack emits them."""
    site, table = dispatch_site(rom)
    if not 0 < table < len(rom):
        raise NpcCommandError(f"the speed table address 0x{table:06X} is outside the ROM")

    # How many speeds there are is the gap between the pointer table and the
    # first thing it points at: the tables sit flush behind it, so the table's
    # own length is data rather than a number this module carries.
    first = int.from_bytes(rom[table:table + 4], "big")
    span = first - table
    if span <= 0 or span % 4:
        raise NpcCommandError(
            f"the speed table at 0x{table:06X} points at 0x{first:06X}, which is "
            "not a whole number of longs past it; the tables are supposed to be flush"
        )
    pointers = list(struct.unpack_from(f">{span // 4}I", rom, table))

    # The command tables are contiguous and equal in length, so the gap between
    # two of them is one table's size -- the record count is derived, not assumed.
    strides = {b - a for a, b in zip(pointers, pointers[1:])}
    if len(strides) != 1:
        raise NpcCommandError(
            f"the command tables are {sorted(strides)} bytes apart; they are "
            "supposed to be equal in length"
        )
    stride = strides.pop()
    if stride <= 0 or stride % RECORD_BYTES:
        raise NpcCommandError(
            f"a command table is {stride} bytes, not a whole number of "
            f"{RECORD_BYTES}-byte records"
        )
    count = stride // RECORD_BYTES

    tables = []
    for index, offset in enumerate(pointers):
        block = rom[offset:offset + stride]
        if len(block) != stride:
            raise NpcCommandError(f"command table {index} runs past the end of the ROM")
        tables.append(SpeedTable(
            index=index,
            rom_offset=offset,
            commands=tuple(
                _decode(i, block[i * RECORD_BYTES:(i + 1) * RECORD_BYTES])
                for i in range(count)
            ),
            sha256=hashlib.sha256(block).hexdigest(),
        ))

    # The eleven commands are the same in every table, so a consumer can treat
    # the id as a direction and the selector as a speed. Proving it is cheap and
    # it is the whole reason the emitted shape separates the two.
    shapes = {
        tuple((c.x_pixels, c.y_pixels, c.direction, c.facing) for c in t.commands)
        for t in tables
    }
    if len(shapes) != 1:
        raise NpcCommandError(
            "the speed tables do not hold the same commands; the id cannot be "
            "read as a direction independent of the speed"
        )
    if any(command.unused for table_ in tables for command in table_.commands):
        raise NpcCommandError("a command's trailing byte is not zero")

    reference = tables[0].commands
    return {
        "kind": "npc_movement_commands",
        "note": (
            "What a scene's MoveActorCommand byte means. `commands` is indexed "
            "by the command byte and `speeds` by the routine's d7 selector; the "
            "same eleven commands appear at all three speeds. A refused move "
            "still applies the command's facing and still resets the animation, "
            "so a blocked NPC turns on the spot."
        ),
        "dispatch": {
            "routine": "FieldObj_NPCMove",
            "site": f"0x{site:06X}",
            "speed_table": f"0x{table:06X}",
            "speed_count": len(tables),
            "selector": "d7",
            "command_index": "d0",
            "bounds_checked": False,
        },
        "record": {
            "bytes": RECORD_BYTES,
            "fixed_point": FIXED_POINT,
            "fields": {
                "0": "x distance in pixels",
                "1": "y distance in pixels",
                "2": "x velocity, signed word, pixels per frame times 256",
                "4": "y velocity, signed word, pixels per frame times 256",
                "6": "facing direction, or 0xFF to leave facing unchanged",
                "7": "unused, zero in every retail record",
            },
            "frames": "distance * 256 / abs(velocity), from FieldObj_UpdateStepDuration",
        },
        "command_count": count,
        "commands": [
            {
                "id": command.index,
                "direction": command.direction,
                "cells": max(command.x_pixels, command.y_pixels) // COLLISION_CELL_PIXELS,
                "facing": {
                    "byte": command.facing,
                    "name": FACING_NAMES.get(command.facing),
                    "unchanged": command.facing == FACING_UNCHANGED,
                },
            }
            for command in reference
        ],
        "speeds": [table_.to_json() for table_ in tables],
        "census": {
            "distinct_directions": sorted(
                {c.direction for c in reference if c.direction}
            ),
            "standing_still": [c.index for c in reference if not c.moves],
            "duplicate_directions": {
                direction: [c.index for c in reference if c.direction == direction]
                for direction in sorted({c.direction for c in reference if c.direction})
            },
            "pixels_per_command": sorted(
                {max(c.x_pixels, c.y_pixels) for c in reference if c.moves}
            ),
            "note": (
                "Every moving command covers exactly one collision cell, and "
                "three of the eleven ids stand still. Left and right each have "
                "three ids that decode identically, which is why a consumer must "
                "read the id through this table rather than deriving a direction "
                "from its value."
            ),
        },
    }
