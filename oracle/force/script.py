"""Party scripts, validated against a previously observed base-tape RAM image.

The image is read-only evidence, not an input to the emulator. Techniques and
skills live at character offsets $52/$62 (ps4.constants.asm:55-59); inventory
and party slots are $F410/$F40A (constants:2373-2381). Definition bytes come
from the ignored runtime pack, decoded by psiv_tools, never from this module.
"""
from __future__ import annotations

import dataclasses
import json
import pathlib

from .errors import ForceError
from .runs import sha256

KINDS = ("attack", "technique", "skill", "item", "defend")


def number(value, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, (int, str)):
        raise ForceError(f"{label} must be an integer or 0x-prefixed string")
    try:
        return int(value, 0) if isinstance(value, str) else value
    except ValueError as error:
        raise ForceError(f"invalid {label}: {value!r}") from error


@dataclasses.dataclass(frozen=True)
class Command:
    kind: str
    target: int | None = None
    id: int | None = None

    @classmethod
    def parse(cls, value) -> Command:
        if not isinstance(value, dict) or value.get("command") not in KINDS:
            raise ForceError(f"command must name one of {KINDS}")
        kind = value["command"]
        keys = {"command"}
        if kind != "defend":
            keys.add("target")
        if kind in ("technique", "skill", "item"):
            keys.add("id")
        if set(value) != keys:
            raise ForceError(f"{kind} needs exactly {sorted(keys)}")
        target = number(value["target"], "target") if "target" in keys else None
        if target is not None and target not in (-1, *range(1, 10)):
            raise ForceError("target must be fighter 1..9, or -1 for a whole side")
        ability = number(value["id"], "id") if "id" in keys else None
        limit = 255 if kind == "item" else 127
        if ability is not None and not 1 <= ability <= limit:
            raise ForceError(f"id must be in 1..{limit}")
        return cls(kind, target, ability)


@dataclasses.dataclass(frozen=True)
class Script:
    rounds: tuple[dict[int, Command], ...]
    repeat_last: bool = False

    @classmethod
    def parse(cls, value) -> Script:
        if not isinstance(value, dict) or set(value) - {"rounds", "repeat_last"}:
            raise ForceError("script needs rounds and optional repeat_last")
        rounds = value.get("rounds")
        if not isinstance(rounds, list) or not rounds:
            raise ForceError("rounds must be a nonempty list of fighter-command maps")
        repeat = value.get("repeat_last", False)
        if not isinstance(repeat, bool):
            raise ForceError("repeat_last must be boolean")
        parsed = []
        for round_ in rounds:
            if not isinstance(round_, dict) or not round_:
                raise ForceError("each round must be a nonempty fighter-command map")
            commands = {}
            for key, value in round_.items():
                fighter = number(key, "fighter")
                if not 1 <= fighter <= 5 or fighter in commands:
                    raise ForceError("fighter ids must be distinct party slots 1..5")
                commands[fighter] = Command.parse(value)
            parsed.append(commands)
        return cls(tuple(parsed), repeat)

    def round(self, index: int) -> dict[int, Command]:
        if index < 0:
            raise ForceError("queue precedes the first player command phase")
        if index < len(self.rounds):
            return self.rounds[index]
        if self.repeat_last:
            return self.rounds[-1]
        raise ForceError(f"script has no player-command round {index + 1}")

    def command(self, fighter: int, index: int, completed_rounds: int, cap: int) -> Command:
        # The capped fixture ends before the next queue executes. Selecting
        # DEFEND builds that boundary without needing an extra scripted round.
        if cap and completed_rounds >= cap:
            return Command("defend")
        return self.round(index)[fighter]


def definitions(pack: pathlib.Path) -> dict:
    try:
        abilities = json.loads((pack / "battle/abilities.json").read_text())
    except (OSError, ValueError) as error:
        raise ForceError(f"cannot read runtime-pack battle definitions: {error}") from error
    return {kind: {entry["id"]: entry for entry in abilities[key]}
            for kind, key in (("technique", "techniques"), ("skill", "skills"),
                              ("item", "item_effects"))}


def patch_state(raw: bytes, specs: list[str], frame: int) -> bytes:
    """Explicit party/inventory fixtures at the seam, never command/cursor RAM."""
    state = bytearray(raw)
    for spec in specs:
        try:
            when, address, data = spec.split(":")
            address = int(address, 16)
            if not 0xFFFF0000 <= address <= 0xFFFFFFFF:
                raise ValueError("expected a full cartridge RAM address")
            address &= 0xFFFF
            data = bytes.fromhex(data)
            when = int(when)
        except (ValueError, TypeError) as error:
            raise ForceError(f"invalid --ram-patch {spec!r}") from error
        if when != frame or not data or not 0xF400 <= address < address + len(data) <= 0xFA80:
            raise ForceError("script fixtures must patch party/inventory RAM $F400..$FA7F "
                             f"at the seam frame {frame}; menu RAM is forbidden")
        state[address:address + len(data)] = data
    return bytes(state)


def party_state(raw: bytes) -> dict[int, dict]:
    if len(raw) != 0x10000:
        raise ForceError("script state must be the host's 65536-byte big-endian RAM dump")
    party = {}
    for slot, character in enumerate(raw[0xF40A:0xF40F], 1):
        if character == 0xFF:
            continue
        if character > 10:
            raise ForceError(f"invalid character id {character} in party slot {slot}")
        start = 0xF500 + character * 0x80
        record = raw[start:start + 0x80]
        party[slot] = {"character": character, "technique": list(record[0x52:0x62]),
                       "skill": list(record[0x62:0x6A]),
                       "uses": list(record[0x6A:0x7A:2]),
                       "equipment": list(record[0x4C:0x50]),
                       "tp": int.from_bytes(record[0x12:0x14], "big"),
                       "profession": int.from_bytes(record[6:8], "big")}
    return party


def validate(script: Script, raw: bytes, records: dict) -> None:
    """Reject unavailable commands before the first capture/probe frame runs."""
    party = party_state(raw)
    inventory = list(raw[0xF410:0xF438])
    for index, commands in enumerate(script.rounds, 1):
        if set(commands) != set(party):
            raise ForceError(f"round {index} needs exactly party fighters {sorted(party)}")
        for fighter, command in commands.items():
            member = party[fighter]
            if command.kind in ("attack", "defend"):
                if command.kind == "attack" and command.target not in (-1, 6, 7, 8, 9):
                    raise ForceError("an attack targets the enemy side")
                continue
            record = records[command.kind].get(command.id)
            if record is None:
                raise ForceError(f"unknown {command.kind} {command.id}")
            if command.kind == "item":
                if command.id not in member["equipment"] + inventory:
                    raise ForceError(f"fighter {fighter} has no item {command.id}")
                if not record["battle_object_or_graphic_id"]:
                    raise ForceError(f"item {command.id} has no battle activation")
                range_ = record["targeting_or_parameter_3"] & 15
            else:
                if command.id not in member[command.kind]:
                    raise ForceError(f"fighter {fighter} does not know {command.kind} {command.id}")
                if not record["targeting"]["raw"] & 0x10:
                    raise ForceError(f"{command.kind} {command.id} is not usable in battle")
                if command.kind == "technique" and record["tp_cost"] > member["tp"]:
                    raise ForceError(f"fighter {fighter} cannot afford technique {command.id}")
                if command.kind == "skill" and not member["uses"][member["skill"].index(command.id)]:
                    raise ForceError(f"fighter {fighter} has no uses of skill {command.id}")
                range_ = record["targeting"]["raw"] & 15
            if range_ in (1, 4, 6, 8):
                targets = (6, 7, 8, 9) if range_ == 1 else tuple(party)
                if command.target not in targets:
                    raise ForceError(f"{command.kind} {command.id} needs a single target in {targets}")
                if range_ in (4, 6):
                    android = party[command.target]["profession"] == 5
                    if android == (range_ == 4):
                        raise ForceError(f"target {command.target} has the wrong biological type")
            elif command.target != -1:
                raise ForceError(f"{command.kind} {command.id} needs target -1 (no target cursor)")


def preflight(args, facts: dict) -> Script:
    try:
        script = Script.parse(json.loads(pathlib.Path(args.party_script).read_text()))
        snapshot = facts["script_state"]
        if snapshot["frame"] != facts["battle_first"]:
            raise ForceError("cached script-state frame does not match the tape seam")
        path = pathlib.Path(snapshot["path"])
        if sha256(path) != snapshot["sha256"]:
            raise ForceError("cached script-state hash changed")
        raw = patch_state(path.read_bytes(), args.ram_patch, facts["battle_first"] + 1)
    except (KeyError, OSError, ValueError) as error:
        raise ForceError("script needs a matching scout with --prepare-script evidence: "
                         f"{error}") from error
    validate(script, raw, definitions(pathlib.Path(args.runtime_pack)))
    return script
