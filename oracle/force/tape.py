"""The tape format (`oracle/host/tape.c`) and the composed input policies.

A tape is `"<frames> <buttons> [mark]"` lines with `repeat N`/`end` blocks, and
every command the oracle gets is one composed tape: the base tape's field
prefix, an optional idle delay, then the scripted policy that drives the fight.
Frame arithmetic is the whole subject here - the prefixes, the trims and the
policies are all counted in frames, and the composition has to keep the field
prefix the base tape's own frames so the encounter stays where the scout found
it.
"""
from __future__ import annotations

import dataclasses

from .errors import ForceError

#: One `C` press per 16 frames, as `oracle/tapes/07_first_battle.tape` and
#: `09_second_battle.tape` do it: 4 held, 12 released.
PRESS_FRAMES = 4
RELEASE_FRAMES = 12
#: Frames of log kept after the battle's last in-battle frame.
TAIL_FRAMES = 60
#: The probe only needs the formation draw, so it stops well before a fight
#: can finish: an untrimmed run that outlives a defeat walks into the game-over
#: sequence, where a new game re-initializes `Main_Frame_Count` and the host
#: rightly refuses to close the trace. Its input frames up to the draw are the
#: capture's own, which is what makes the probe's draw facts hold for it.
PROBE_REPEATS = 200


@dataclasses.dataclass(frozen=True)
class Step:
    frames: int
    buttons: str
    mark: str = ""


def expand_tape(text: str) -> list[Step]:
    """The tape's steps with `repeat` blocks expanded, in frame order."""
    steps: list[Step] = []
    stack: list[tuple[int, int]] = []
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("repeat"):
            stack.append((len(steps), int(line.split()[1])))
            continue
        if line == "end":
            if not stack:
                raise ForceError("tape has an 'end' without 'repeat'")
            start, count = stack.pop()
            block = steps[start:]
            for _ in range(count - 1):
                steps.extend(block)
            continue
        parts = line.split()
        if len(parts) < 2:
            raise ForceError(f"tape line is not '<frames> <buttons>': {line!r}")
        if int(parts[0]) < 1:
            raise ForceError(f"tape line has no frames: {line!r}")
        steps.append(Step(int(parts[0]), parts[1],
                          parts[2] if len(parts) > 2 else ""))
    if stack:
        raise ForceError("tape ends inside a 'repeat' block")
    if not steps:
        raise ForceError("tape has no steps")
    return steps


def tape_frames(steps: list[Step]) -> int:
    return sum(step.frames for step in steps)


def emit_tape(steps: list[Step], header: str = "") -> str:
    """A tape file for these steps, merging runs of markless equal steps."""
    lines: list[Step] = []
    for step in steps:
        if step.frames <= 0:
            continue
        if lines and not step.mark and not lines[-1].mark \
                and lines[-1].buttons == step.buttons:
            lines[-1] = Step(lines[-1].frames + step.frames, step.buttons)
            continue
        lines.append(step)
    body = "".join(
        f"{s.frames} {s.buttons}" + (f" {s.mark}" if s.mark else "") + "\n"
        for s in lines)
    return header + body


def trim_tape(steps: list[Step], upto: int) -> list[Step]:
    """The steps covering frames 1..`upto` (1-based, inclusive)."""
    out: list[Step] = []
    frame = 1
    for step in steps:
        if frame > upto:
            break
        out.append(Step(min(step.frames, upto - frame + 1), step.buttons,
                        step.mark))
        frame += step.frames
    return out


def policy_steps(name: str, repeats: int) -> list[Step]:
    """The scripted input policy as tape steps."""
    if name == "attack":
        return [Step(PRESS_FRAMES, "C"), Step(RELEASE_FRAMES, ".")] * repeats
    if name == "defend":
        # Tape 14: the per-character command menu is HORIZONTAL, so four Right
        # presses step ATTACK -> TECHNIQUE -> SKILL -> ITEM -> DEFEND; the C
        # before them picks COMD and the one after confirms DEFEND.
        block = [Step(PRESS_FRAMES, "C"), Step(RELEASE_FRAMES, ".")]
        for _ in range(4):
            block += [Step(PRESS_FRAMES, "R"), Step(10, ".")]
        block += [Step(PRESS_FRAMES, "C"), Step(RELEASE_FRAMES, ".")]
        return block * repeats
    raise ForceError(f"unknown policy {name!r} (attack, defend)")


def compose(base_steps: list[Step], cut: int, delay: int, repeats: int,
            policy: str) -> list[Step]:
    """Field prefix (frames 1..cut) + optional delay + policy + tail."""
    steps = trim_tape(base_steps, cut)
    if delay:
        steps.append(Step(delay, ".", "delay"))
    steps += policy_steps(policy, repeats)
    steps.append(Step(TAIL_FRAMES * 10, ".", "policy_end"))
    return steps
