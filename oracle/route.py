#!/usr/bin/env python3
"""Plan a walking route across PSIV maps and emit it as an oracle tape.

The route is computed from the project's own extracted data (the runtime pack's
collision grids and warp records), not from poking the emulator. That makes it
a second job as well as a convenience: if a generated route walks correctly on
the cartridge, the extraction agrees with the cartridge about collision and the
map graph, and if it desyncs, the oracle log says exactly which cell disagreed.

Cell coordinates here are the emulator's: cell = pixel // 16, which matches the
warp records' `x_byte`/`y_byte` fields. The pack's `y_cell` carries a +1 offset
relative to `y_byte`, so it is deliberately not used for sources.
"""
import argparse
import collections
import glob
import json
import pathlib

# Blocking collision types, per docs/RUNTIME_DESIGN.md: 8 solid, 9 water,
# $A sand, $B ice, $C shop. Type 1 (map change) deliberately does not block.
BLOCKING = {0x8, 0x9, 0xA, 0xB, 0xC}

# dx, dy, tape letter, facing value (constants:107)
DIRS = [(0, -1, 'U', 4), (0, 1, 'D', 0), (-1, 0, 'L', 12), (1, 0, 'R', 8)]

FRAMES_PER_CELL = 8  # measured on the cartridge; see oracle/README.md


def load_pack(pack_dir):
    maps = {}
    for p in glob.glob(str(pathlib.Path(pack_dir) / "maps" / "*.json")):
        d = json.loads(pathlib.Path(p).read_text())
        maps[int(d["id"])] = d
    return maps


# The pack's collision grid is indexed by the *occupied* cell, which is one row
# below curr_y_pos / 16: GetChunkAndCollision adds $10 to Y before shifting
# down, so a character at pixel row R stands on grid row R + 1. The packer
# already applies that shift when it emits y_cell (see the coordinates note in
# rust/psiv-data/src/map.rs), and this harness measured the same offset
# independently against the game's own Tile_Collision_* readouts: 99.86% over
# 6510 samples, versus 93% or worse for every other offset. Since this module
# works in curr_y_pos/16 space to match the emulator's logs, it re-applies the
# shift when indexing the grid.
GRID_Y_BIAS = 1


def walkable(m, x, y):
    c = m["collision"]
    gy = y + GRID_Y_BIAS
    if not (0 <= x < c["width_cells"] and 0 <= gy < c["height_cells"]):
        return False
    return c["rows"][gy][x] not in BLOCKING


def warps_at(m):
    """cell -> (target_map, dest_x, dest_y), keyed by every cell in each rect."""
    out = {}
    for w in m.get("warps", []):
        tgt = w["target"]["id"]
        dx, dy = w["destination"]["x_cell"], w["destination"]["y_cell"]
        r = w["rect"]
        for yy in range(r["y"] - 1, r["y"] - 1 + r["height"]):
            for xx in range(r["x"], r["x"] + r["width"]):
                out.setdefault((xx, yy), (tgt, dx, dy))
    return out


def plan(maps, start, goal_map, blocked=frozenset()):
    """BFS over (map, x, y). Returns a list of ((map, x, y), step_letter).

    `blocked` is a set of (map, x, y) the search must not enter - used by
    `oracle.scripts.navigate` to route around field objects and cells the
    cartridge has refused to admit the character to.
    """
    # A node carries whether the character arrived here through a warp. The
    # cartridge initialises Tile_Collision_Standing to 1 on placement
    # (GameMode_LoadFieldMap), which is its own anti-ping-pong rule: a doorway
    # never fires on the frame you are placed on its destination. Without
    # modelling that, the planner happily walks straight back through the door
    # it just came out of.
    # The start position was observed to be stable - the game did not warp the
    # character away from it - so it is entered as already-placed, which both
    # suppresses an immediate re-warp and lets the search step off a warp cell.
    start = (start[0], start[1], start[2], True)
    seen = {start: None}
    q = collections.deque([start])
    goal = None
    while q:
        cur = q.popleft()
        mid, x, y, just_warped = cur
        if mid == goal_map:
            goal = cur
            break
        m = maps.get(mid)
        if m is None:
            continue
        wm = warps_at(m)
        if (x, y) in wm and not just_warped:
            t, dx, dy = wm[(x, y)]
            nxt = (t, dx, dy, True)
            if t in maps and nxt not in seen and (t, dx, dy) not in blocked:
                seen[nxt] = (cur, '*')
                q.append(nxt)
            continue
        for ddx, ddy, letter, _ in DIRS:
            nx, ny = x + ddx, y + ddy
            if not walkable(m, nx, ny):
                continue
            nxt = (mid, nx, ny, False)
            if (mid, nx, ny) in blocked:
                continue
            if nxt not in seen:
                seen[nxt] = (cur, letter)
                q.append(nxt)
    if goal is None:
        return None
    path = []
    cur = goal
    while seen[cur] is not None:
        prev, letter = seen[cur]
        path.append(((prev[0], prev[1], prev[2]), letter))
        cur = prev
    path.reverse()
    return path


def to_tape(path):
    """Collapse the step list into held-direction tape lines."""
    lines = []
    run_letter, run_len = None, 0

    def flush():
        nonlocal run_letter, run_len
        if run_letter and run_len:
            lines.append(f"{run_len * FRAMES_PER_CELL} {run_letter}")
        run_letter, run_len = None, 0

    for (mid, x, y), letter in path:
        if letter == '*':
            flush()
            lines.append(f"120 . warp_from_{mid:02X}")
            continue
        if letter == run_letter:
            run_len += 1
        else:
            flush()
            run_letter, run_len = letter, 1
    flush()
    return lines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pack", default="runtime-pack")
    ap.add_argument("--from-map", type=lambda s: int(s, 0), required=True)
    ap.add_argument("--from-cell", required=True, help="x,y")
    ap.add_argument("--to-map", type=lambda s: int(s, 0), required=True)
    ap.add_argument("--out")
    a = ap.parse_args()

    maps = load_pack(a.pack)
    x, y = (int(v) for v in a.from_cell.split(","))
    path = plan(maps, (a.from_map, x, y), a.to_map)
    if path is None:
        raise SystemExit(f"no route from map {a.from_map:#x} to {a.to_map:#x}")

    hops = [p for p in path if p[1] == '*']
    print(f"route: {len(path)} steps, {len(hops)} map transitions")
    seen_maps = []
    for (mid, cx, cy), letter in path:
        if letter == '*':
            seen_maps.append(mid)
    print("  maps traversed:",
          " -> ".join(f"${m:02X}" for m in [a.from_map] + seen_maps))

    lines = to_tape(path)
    text = "\n".join(lines) + "\n"
    if a.out:
        pathlib.Path(a.out).write_text(text)
        print(f"  wrote {a.out} ({len(lines)} tape lines, "
              f"{sum(int(l.split()[0]) for l in lines)} frames)")
    else:
        print(text)


if __name__ == "__main__":
    main()
