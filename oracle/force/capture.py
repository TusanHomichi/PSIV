"""Reading a capture: what the log says the battle did.

The log's own columns decide everything here - the battle's window
(`Game_Mode_Index` $10/$14), the enemy slots it built (ids *and* max HP, since
an enemy id of `0` is also what an empty slot reads), which ability each enemy
actually ran, the party's HP, the vehicle fighter's HP in a vehicle battle, and
the rewards the battle paid out. Nothing is inferred from what the tool asked
for: `matches` compares the log's slots against the pack's record for the forced
formation, and the tool stops when they disagree.
"""
from __future__ import annotations

import dataclasses
import pathlib

from .. import fixture
from .errors import ForceError
from .pack import Pack
from .runs import Run, by_frame, battle_window, hp_of, read_rows, sha256


@dataclasses.dataclass
class Capture:
    window: tuple[int, int]
    outcome: str
    enemies: list[dict]
    party: list[dict]
    vehicle_hp: int
    abilities: dict[str, int]
    rewards: dict
    log_path: pathlib.Path
    trace_path: pathlib.Path
    log_sha256: str
    trace_sha256: str
    #: The frame the extractor reads the battle's start state from
    #: (`oracle/fixture/observations.py`'s `enemies_loaded`), and the frames the
    #: log's own `Battle_Turn_Order` changes on: its rounds. Both are read only
    #: when the run asked for them - `--durable` and `--max-rounds` - because
    #: they need the log's battle columns and nothing else does.
    start_frame: int = 0
    round_frames: list[int] = dataclasses.field(default_factory=list)
    #: A capped capture: the frame its last captured round ends on, and the
    #: round the cap sits at.
    cut_frame: int | None = None
    rounds_captured: int | None = None

    @property
    def rounds(self) -> int:
        """The rounds the log holds, once they have been read."""
        return len(self.round_frames)

    @property
    def truncated(self) -> bool:
        return self.cut_frame is not None


def enemies_at(rows: list[dict], frame: int) -> list[dict]:
    """The enemy slots the log shows at `frame`, as (slot, id, maxhp).

    The frame is the one the extractor reads the battle's start state from
    (`oracle/fixture/observations.py`'s `enemies_loaded`), because a formation's
    enemies are what it *seated*: a battle whose enemies change while it runs -
    formation `$3C`'s InfantWorms grow into a SandWorm at f28969 in this sweep's
    capture - would otherwise be reported as whatever the last frame holds, and
    the formation check would refuse a capture the cartridge played correctly.
    """
    row = by_frame(rows)[frame]
    count = int(row["enemy_count"])
    return [{"slot": slot, "id": int(row[f"e{slot}_id"]),
             "maxhp": int(row[f"e{slot}_maxhp"])}
            for slot in range(1, 5) if slot <= count]


def enemy_slots(rows: list[dict], window: tuple[int, int]) -> list[dict]:
    """The enemy slots the battle built, as (slot, id, maxhp) at the end."""
    first, last = window
    count = max((int(row["enemy_count"]) for row in rows
                 if first <= int(row["frame"]) <= last), default=0)
    end = by_frame(rows)[last]
    return [{"slot": slot, "id": int(end[f"e{slot}_id"]),
             "maxhp": int(end[f"e{slot}_maxhp"])}
            for slot in range(1, 5) if slot <= count]


def classify(rows: list[dict], window: tuple[int, int],
             vehicle: bool = False) -> str:
    """What the log says the battle did, from the party's and enemies' HP.

    `victory` needs every built enemy slot at or below zero HP, `defeat` every
    party member. A battle that ends with both sides standing is reported as
    `withdrawal` rather than guessed at - an enemy escape, or a vehicle battle
    whose wrecked vehicle the party's own HP columns cannot show (the vehicle
    is the party-side fighter there, and `vehicle_fighter_hp` says so).
    """
    if window[1] >= int(rows[-1]["frame"]):
        return "unfinished"
    end = by_frame(rows)[window[1]]
    slots = enemy_slots(rows, window)
    if slots and all(hp_of(end, f"e{entry['slot']}_hp") <= 0
                     for entry in slots):
        return "victory"
    if vehicle and hp_of(end, "vehicle_fighter_hp") <= 0:
        return "defeat"
    if all(hp_of(end, f"{who}_hp") <= 0 for who in ("chaz", "alys", "hahn")):
        return "defeat"
    return "withdrawal"


def ability_uses(rows: list[dict], window: tuple[int, int],
                 from_frame: int) -> dict[str, int]:
    """{"eN=0xID": first frame}: the ability each enemy actually ran."""
    first, last = window
    seen: dict[str, int] = {}
    for row in rows:
        frame = int(row["frame"])
        if not max(first, from_frame) <= frame <= last:
            continue
        for slot in (1, 2, 3, 4):
            value = int(row[f"e{slot}_ability"], 16)
            if value:
                seen.setdefault(f"e{slot}=0x{value:02X}", frame)
    return seen


def read_capture(run: Run, draw_frame: int, vehicle: bool = False,
                 rows: list[dict] | None = None) -> Capture:
    """What the log says the battle did.

    `rows` is the run's log when the caller has already read it - one parse per
    run, since these logs are tens of megabytes - and `None` reads it here.
    """
    rows = read_rows(run.log) if rows is None else rows
    window = battle_window(rows)
    if window is None:
        raise ForceError(f"the run in {run.log.parent} holds no battle: "
                         "game mode $10/$14 never appears")
    end = by_frame(rows)[window[1]]
    return Capture(
        window=window,
        outcome=classify(rows, window, vehicle),
        enemies=enemy_slots(rows, window),
        party=[{"who": who, "hp": hp_of(end, f"{who}_hp")}
               for who in ("chaz", "alys", "hahn")],
        vehicle_hp=hp_of(end, "vehicle_fighter_hp")
        if "vehicle_fighter_hp" in end else 0,
        abilities=ability_uses(rows, window, draw_frame + 1),
        rewards={"experience": int(end["battle_exp_total"]),
                 "meseta": int(end["battle_meseta_total"])},
        log_path=run.log, trace_path=run.trace,
        log_sha256=sha256(run.log),
        trace_sha256=sha256(run.trace),
    )


def battle_shape(capture: Capture, rows: list[dict],
                 layout: dict[str, dict]) -> None:
    """Fill in the capture's start frame and rounds, from the extractor's own
    readings of the same log - `enemies_loaded` and `round_frames`.

    Both are the extractor's rules, not this tool's: the start frame is the
    frame a fixture takes the battle's start state from (what `--durable`'s
    patch has to land on and be verified at), and the round frames are
    `Battle_Turn_Order`'s own changes (what `--max-rounds` counts).
    """
    log = fixture.Log(rows, layout)
    first, last = capture.window
    capture.start_frame = fixture.enemies_loaded(log, first, last)
    capture.round_frames = fixture.round_frames(log, capture.start_frame, last)
    # The formation's own enemies, as the fixture's start state will read them.
    capture.enemies = enemies_at(rows, capture.start_frame)


def cap_rounds(capture: Capture, max_rounds: int) -> None:
    """Apply `--max-rounds`: the capture ends one frame before round N+1 opens.

    `Battle_Turn_Order`'s next change *is* the round boundary (`round_frames`),
    so round N's last captured frame is the one before it; that frame is what a
    fixture window is cut at, and the tape is trimmed to the boundary itself so
    the extractor can see it.

    A log that holds no more rounds than the cap is left alone: a battle that
    ends inside the cap simply ends, and its outcome is the battle's. One that
    holds *more* is capped whether or not the fight ended later - a battle that
    outlasts the cap is exactly what the cap is for - and the fixture's
    `outcome.truncated` is the same cut (`oracle/fixture/assembly.py`).
    """
    if max_rounds <= 0:
        return
    if len(capture.round_frames) > max_rounds:
        capture.cut_frame = capture.round_frames[max_rounds] - 1
        capture.rounds_captured = max_rounds
        capture.outcome = "truncated"
        return
    if capture.outcome == "unfinished":
        raise ForceError(
            f"the capture's log holds {len(capture.round_frames)} round(s) "
            f"and the battle is still running: --max-rounds {max_rounds} "
            "cannot be placed against a boundary the tape never reaches "
            "(re-run with more --repeats)")


def matches(capture: Capture, pack: Pack, formation: int) -> bool:
    """Whether the log built exactly the pack's record for the formation."""
    return capture.enemies == pack.enemies_of(formation)
