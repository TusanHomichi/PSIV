"""The cartridge's rolls: how one is derived, and how the trace is checked.

`UpdateRNGSeed2` is:

    30 2D 00 08     move.w  $8(a5), d0        ; d0 = VDP HV counter
    D0 78 EF 1C     add.w   (Main_Frame_Count).w, d0
    90 78 EF 0C     sub.w   (RNG_Seed).w, d0   ; d0 = the roll
    E6 F8 EF 0C     ror     (RNG_Seed).w
    4E 75           rts

`(RNG_Seed).w` at `$FFFFEF0C` is the **high half** of the longword stored
there: a 68000 word operand at that address reads bytes `$EF0C`/`$EF0D`, and
`RNG_Seed` is a longword. So the roll is

    roll = (hv + frame_count - high_word(seed)) & $FFFF

This module derives every roll from the trace's raw `hv`, `frame_count` and
`seed_before` columns, which are not in doubt (the seed chain and the HV reads
were verified by `oracle/rng_trace.py check`), and **insists** that the trace's
own `roll` column is that same derivation - row by row, naming the frame and
call of the first one that is not:

    "roll_column": {"agrees": n, "subtracts_low_word": 0, "neither": 0}

A trace that carries the *low* half instead (what `oracle/host/rng_trace.c`
wrote before it was fixed, and what `oracle/rng_trace.py`'s `roll_for` used to
re-derive, so neither could see it) is rejected rather than replayed, because
its column is a per-frame-constant shift of the cartridge's rolls. The reading
is settled by the cartridge, not by preference: the high-word derivation
reproduces the battle's turn order (`turn_XX` in the log) and all six of its
damage values exactly, and the low-word one reproduces none of them. See
`docs/oracle/BATTLE_ORACLE_REPLAY.md`.
"""
from .errors import FixtureError

M16 = 0xFFFF

#: `Battle_CalculateDamage`'s loop bound, and therefore the length of every
#: damage run in the trace (`moveq #$F, d7` + `dbf`).
DAMAGE_RUN = 16

#: `Fighters_Hit_Flags`'s "not targeted or missed" byte.
HIT_NOT_TARGETED = "FF"


def roll_high_word(row):
    """The cartridge's roll: `sub.w (RNG_Seed).w, d0` reads $FFFFEF0C."""
    seed = int(row["seed_before"], 16)
    return (int(row["hv"], 16) + int(row["frame_count"])
            - ((seed >> 16) & M16)) & M16


def roll_low_word(row):
    """The low-half subtraction `oracle/host/rng_trace.c` wrote before it was
    fixed (`$FFFFEF0E`, the word `ror` never touches and no instruction reads
    as the subtrahend). Kept so a stale trace is named for what it carries
    instead of being replayed or called random."""
    seed = int(row["seed_before"], 16)
    return (int(row["hv"], 16) + int(row["frame_count"])
            - (seed & M16)) & M16


def roll_column_report(rows):
    """How the trace's `roll` column relates to the cartridge's derivation.

    The column is *checked*, not merely counted: every row must carry the
    cartridge's roll (`roll_high_word`), and the first row that does not is
    reported with its frame and call index. A trace whose column is the
    low-half subtraction is rejected here rather than replayed - the two
    conventions differ by a per-frame constant, so a fixture built on the
    column would replay rolls the cartridge never drew.

    Returns the counts for a trace that passes: every row agrees, so
    `subtracts_low_word` and `neither` are zero."""
    report = {"agrees": 0, "subtracts_low_word": 0, "neither": 0}
    for row in rows:
        logged = int(row["roll"], 16)
        if logged == roll_high_word(row):
            report["agrees"] += 1
            continue
        low = roll_low_word(row)
        if logged == low:
            report["subtracts_low_word"] += 1
            convention = (f"the low-half subtraction (hv + frame_count - "
                          f"seed_low) & $FFFF = {low:04X}, which "
                          f"oracle/host/rng_trace.c wrote before it was "
                          f"fixed")
        else:
            report["neither"] += 1
            convention = ("neither the cartridge's roll nor the low-half "
                          "subtraction")
        raise FixtureError(
            f"trace f{row.get('frame', '?')} call "
            f"{row.get('call_index_in_frame', '?')}: the roll column reads "
            f"{logged:04X}, {convention}; the cartridge's roll is "
            f"(hv + frame_count - seed_high) & $FFFF = "
            f"{roll_high_word(row):04X}")
    return report


def rolls_in_window(rows, first, last):
    """[(frame, roll)] for the trace rows inside the battle's frame window.

    Every row's own `roll` column has been checked against this derivation
    (`roll_column_report`), so the value here is the file's column too."""
    return [(int(row["frame"]), roll_high_word(row)) for row in rows
            if first <= int(row["frame"]) <= last]


def group_by_frame(rolls):
    """[(frame, [roll, ...])] in frame order, frames ascending and unique."""
    frames = []
    for frame, _ in rolls:
        if not frames or frames[-1] != frame:
            frames.append(frame)
    return [(frame, [roll for f, roll in rolls if f == frame]) for frame in frames]
