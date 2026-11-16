"""Validate and tabulate priority-opening captures, without runtime constants.

Only ROM offsets, lengths and digests are stored here. Instruction bytes and
video remain in the user's local evidence directory.
"""
from __future__ import annotations

import hashlib
from collections import Counter

from oracle.checks import update_rng_seed
from oracle import rng_trace

ROM_SHA = "511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a"
WINDOWS = (
    ("priority", 0xB62A, 0x78, "f78fb3cc419b0def584fae32d8b076945da3f7663029f1153844f68aa2bb2e14"),
    ("opening", 0x52C2, 0xA4, "3e0ff8ba33788321967d390db1169dcea59c28b17e2a4d488569b501a74f53cc"),
    ("surprise", 0x48F6, 0xB6, "790b67355e1b9d233c9551b80bf3dfedd429ef050174d97d6a5cbc6133fae079"),
    ("windows", 0xFBE, 0x6A, "ee64da603aa16a67f775b2631da739baac8c17606dce7c436b1c6bfc04600817"),
    ("options", 0x1228, 0x54, "f9592a4b17e984e883992c1d0009cf90e9a6eb8d883ad6ae31e70258facbaf19"),
    ("menu_dispatch", 0x5380, 0x62, "d6e9193078712e7a9a98fb8cafec8a1e4afd2d1626071c9cde85904750c073dd"),
    ("order", 0x54CC, 0x224, "c286482e8b79046f758abd30c6b6ad3b4e0466b6e0b040a33d7174d6c8b819fd"),
    ("seed", 0x4236C, 0x44, "c7a1b38f635144c48558f0dee5409cdc40d31f89a9ec2482e5f942740fa9a503"),
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def verify_windows(data: bytes, windows=WINDOWS) -> None:
    for name, start, length, expected in windows:
        window = data[start:start + length]
        require(len(window) == length, f"truncated US window: {name}")
        require(hashlib.sha256(window).hexdigest() == expected, f"US window mismatch: {name}")


def verify_rom(data: bytes) -> None:
    require(len(data) == 3145728 and hashlib.sha256(data).hexdigest() == ROM_SHA,
            "not the verified US ROM")
    verify_windows(data)


def number(row: dict, key: str) -> int:
    return int(row[key], 16)


def events(rows: list[dict]) -> list[dict]:
    result = [{"frame": int(r["frame"]), "pc": int(r["pc"], 16),
               "cycles": int(r["cycles"])} for r in rows]
    require(bool(result), "empty instruction trace")
    require(all(a["frame"] <= b["frame"] for a, b in zip(result, result[1:])),
            "instruction trace reordered across frames")
    return result


def at(trace: list[dict], pc: int) -> list[dict]:
    return [r for r in trace if r["pc"] == pc]


def one(trace: list[dict], pc: int) -> dict:
    found = at(trace, pc)
    require(len(found) == 1, f"expected one boundary {pc:06X}, got {len(found)}")
    return found[0]


def ordered(trace: list[dict], pcs: list[int]) -> list[dict]:
    found = [one(trace, pc) for pc in pcs]
    require(all(trace.index(a) < trace.index(b) for a, b in zip(found, found[1:])),
            "routine boundaries missing or misordered")
    return found


def seed_schedule(frames: list[dict], rolls: list[dict], trace: list[dict],
                  start: int, end: int, patched_high: int) -> list[dict]:
    """Every sample, including frames with no rolls; patch precedes retro_run.

    Reuse the existing HV/rotation checker. A patched copy of the preceding
    row supplies its opening seed only at entry; the raw evidence is untouched.
    """
    log = rng_trace.by_frame(frames)
    require(len(log) == len(frames), "duplicate frame sample")
    calls = rng_trace.group_frames(rolls)
    pc_calls = Counter(r["frame"] for r in at(trace, 0x4239E))
    result = []
    for frame in range(start, end + 1):
        require(frame in log and frame - 1 in log, f"missing frame sample {frame}")
        previous, current = log[frame - 1], log[frame]
        before = number(previous, "rng_seed")
        if frame == start:
            before = (patched_high << 16) | (before & 0xFFFF)
        delta = (int(current["main_frame_count"]) - int(previous["main_frame_count"])) & 0xFFFF
        require(delta in (0, 1), f"Main_Frame_Count discontinuity at {frame}")
        these_calls = calls.get(frame, [])
        require(pc_calls[frame] == len(these_calls), f"HV/PC call coverage mismatch at {frame}")
        if these_calls:
            adapted = {**log, frame - 1: {**previous, "rng_seed": f"{before:08X}"}}
            state = dict(frames=0, rolls=0, skipped=0, ambiguous=0)
            rng_trace.check_frame(frame, these_calls, adapted, state)
            require(state["skipped"] == 0, "unanchored HV calls")
        else:
            expected = update_rng_seed(before) if delta else before
            require(number(current, "rng_seed") == expected, f"unexplained seed at {frame}")
        result.append({"frame": frame, "offset": frame - start,
                       "main_frame_count": int(current["main_frame_count"]),
                       "counted_vint": delta, "lag_vint": 1 - delta,
                       "hv_calls": len(these_calls), "seed": current["rng_seed"],
                       "opening_seed": f"{before:08X}"})
    return result


def widths(trace: list[dict], log: dict, pc: int, expected: list[int]) -> list[int]:
    samples = at(trace, pc)
    require([number(log[r["frame"]], "opening_window_width") for r in samples] == expected,
            f"incomplete window passes at {pc:06X}")
    return [r["frame"] for r in samples]


def analyze(frames: list[dict], pc_rows: list[dict], rolls: list[dict], kind: str) -> dict:
    require(kind in ("preemptive", "ambush"), "unknown signed-priority path")
    trace = events(pc_rows)
    start = one(trace, 0xB62A)["frame"]
    # Ignore later actions/rounds, but never truncate before the first endpoint.
    endpoint_pc = 0x1246 if kind == "preemptive" else 0x56D8
    endpoints = at(trace, endpoint_pc)
    require(bool(endpoints), "capture ended before the first command/queue endpoint")
    endpoint = endpoints[0]
    selected = [r for r in trace if start <= r["frame"] <= endpoint["frame"]]
    # A menu may be polled repeatedly; only its first boundary is the endpoint.
    selected = selected[:selected.index(endpoint) + 1]
    require(one(selected, 0x52D6)["frame"] == start, "dispatch differs from roll frame")
    log = rng_trace.by_frame(frames)
    require(start in log and endpoint["frame"] in log, "trace endpoint lacks RAM")
    priority, high = (1, 0x58B4) if kind == "preemptive" else (255, 0x58B3)
    require(number(log[start], "battle_priority") == priority, "wrong priority branch")
    phases = []
    if kind == "preemptive":
        boundaries = ordered(selected, [0xB62A, 0xB650, 0xB670, 0x52D6, 0x530E,
                                        0x5316, 0x491E, 0x499C, 0x5380, 0x123C, 0x1246])
        require(not at(selected, 0x5326) and not at(selected, 0x54CC), "mixed priority paths")
        grow = widths(selected, log, 0x48F6, list(range(2, 19, 2)))
        close = widths(selected, log, 0xFF8, list(range(16, -1, -2)))
        options = widths(selected, log, 0x1228, [2, 4, 6, 8])
        delay = number(log[start], "opening_message_delay")
        dwell = at(selected, 0x4974)
        require(len(dwell) == delay + len(close), "message timer/pass count differs")
        require([number(log[r["frame"]], "opening_message_timer") for r in dwell]
                == [(delay - i - 1) & 0xFFFF for i in range(len(dwell))],
                "message timer progression differs")
        require([r["frame"] for r in at(selected, 0x497A)] == close,
                "message close begins outside negative timer")
        require(one(selected, 0x491E)["frame"] == grow[-1]
                and one(selected, 0x499C)["frame"] == close[-1], "window continuation differs")
        phases = [("transient_growth", grow[0], grow[-1]),
                  ("nonnegative_timer", dwell[0]["frame"], close[0] - 1),
                  ("transient_close", close[0], close[-1]),
                  ("options_growth", options[0], options[-1]),
                  ("first_command_poll", endpoint["frame"], endpoint["frame"])]
    else:
        boundaries = ordered(selected, [0xB62A, 0xB650, 0xB670, 0x52D6, 0x530E,
                                        0x5326, 0x5350, 0x52CA, 0x54CC, 0x550A,
                                        0x554E, 0x56A8, 0x56D8])
        require(all(not at(selected, pc) for pc in (0x5316, 0x48F6, 0xFBE, 0x1246)),
                "ambush opened a message or command window")
        row = log[start]
        require(number(row, "opening_continuation") == 14
                and number(row, "party_fighter_routine") == 4
                and number(row, "name_routine") == 5, "ambush routine parking differs")
        require(len(at(selected, 0xB662)) == 2
                and all(number(row, k) & 8 for k in ("enemy_6_reaction", "enemy_7_reaction")),
                "ambush reaction writes missing")
        timer = at(selected, 0x52C2)
        require([number(log[r["frame"]], "opening_timer") for r in timer]
                == [7, 6, 5, 4, 3, 2, 1, 0, 65535], "ambush timer progression differs")
        queue_row = log[endpoint["frame"]]
        queue = [number(queue_row, f"turn_{i:02}") for i in range(0, 18, 2)]
        require(sorted(q for q in queue if q) == [6, 7], "queue is not the two enemies alone")
        require(number(queue_row, "battle_routine") == 16
                and number(queue_row, "battle_priority") == 0, "enemy queue continuation differs")
        first = selected.index(one(selected, 0x554E))
        middle = selected.index(one(selected, 0x56A8))
        require(len(at(selected[first:middle], 0x4239E)) == 9
                and len(at(selected[middle:], 0x4239E)) == 4,
                "queue calls do not split into nine agility and four target calls")
        phases = [("timer", timer[0]["frame"], timer[-1]["frame"]),
                  ("enemy_only_queue", endpoint["frame"], endpoint["frame"])]
    schedule = seed_schedule(frames, rolls, selected, start, endpoint["frame"], high)
    require(schedule[0]["hv_calls"] == 1, "priority must have exactly one roll")
    if kind == "preemptive":
        require(sum(r["hv_calls"] for r in schedule) == 1, "unexpected opening rolls")
    else:
        queue_calls = schedule[-1]["hv_calls"]
        require(queue_calls == 13 and sum(r["hv_calls"] for r in schedule) == 14,
                "queue draw count differs (nine agility plus four target calls)")
    def summary(first, last):
        rows = [r for r in schedule if first <= r["frame"] <= last]
        return {"first": first, "last": last, "samples": len(rows),
                "counted": sum(r["counted_vint"] for r in rows),
                "lag_frames": [r["frame"] for r in rows if r["lag_vint"]],
                "hv_calls": sum(r["hv_calls"] for r in rows)}
    return {"kind": kind, "origin": start, "endpoint": endpoint,
            "total_including_origin": summary(start, endpoint["frame"]),
            "after_origin": summary(start + 1, endpoint["frame"]),
            "phases": [{"name": name, **summary(first, last)} for name, first, last in phases],
            "boundaries": boundaries, "schedule": schedule}
