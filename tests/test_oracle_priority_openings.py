"""ROM-free controls for opening measurements, including deliberate bad traces."""
import copy
import csv
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from oracle.checks import update_rng_seed
from oracle.priority_measurement import analyze, events, ordered, seed_schedule, verify_rom, verify_windows
from oracle.rng_trace import roll_for, rotate_seed
from oracle.priority_openings import analyze_capture, sha


def fixture(kind):
    """Synthetic instructions/RAM with the same routine contracts, no ROM data."""
    trace = []
    def emit(frame, *pcs):
        for pc in pcs:
            trace.append(dict(frame=str(frame), pc=f"{pc:06X}", cycles=str(len(trace) * 10)))
    emit(100, 0xB62A, 0x4239E, 0xB650)
    if kind == "ambush":
        emit(100, 0xB662, 0xB662)
    emit(100, 0xB670, 0x52D6, 0x530E)
    widths, timers = {}, {}
    if kind == "preemptive":
        emit(100, 0x5316, 0x48F6, 0xFBE)
        widths[100] = 2
        for i, frame in enumerate(range(102, 110)):
            emit(frame, 0x48F6, 0xFBE)
            widths[frame] = 4 + 2 * i
        emit(109, 0x491E)
        for i, frame in enumerate(range(110, 155)):
            emit(frame, 0x4974)
            timers[frame] = (35 - i) & 0xFFFF
            if frame >= 146:
                emit(frame, 0x497A, 0x4996, 0xFF8)
                widths[frame] = 16 - 2 * (frame - 146)
        emit(154, 0x499C)
        emit(155, 0x5380)
        for width, frame in zip([2, 4, 6, 8], [155, 157, 158, 159]):
            emit(frame, 0x1228, 0xFBE)
            widths[frame] = width
        emit(159, 0x123C)
        emit(160, 0x1246)
        # Later rounds are outside the opening, including reused boundaries.
        emit(162, 0x52D6, 0x1246)
        end, high, lags = 160, 0x58B4, {100, 101, 156}
    else:
        emit(100, 0x5326, 0x5350)
        for i, frame in enumerate(range(102, 111)):
            emit(frame, 0x52C2)
            timers[frame] = (7 - i) & 0xFFFF
        emit(110, 0x52CA)
        emit(111, 0x54CC, 0x550A, 0x554E)
        emit(111, *([0x4239E] * 9), 0x56A8, *([0x4239E] * 4), 0x56D8)
        emit(120, 0x52D6)
        end, high, lags = 111, 0x58B3, {100, 101}
    frames, rolls = [], []
    seed, count, width, timer = 0xABCD9843, 99, 0, 36 if kind == "preemptive" else 8
    for frame in range(99, end + 1):
        if frame == 100:
            seed = (high << 16) | (seed & 65535)
        if frame not in lags:
            count += 1
            seed = update_rng_seed(seed)
        calls = sum(int(r["frame"]) == frame and int(r["pc"], 16) == 0x4239E for r in trace)
        for index in range(calls):
            after = rotate_seed(seed)
            rolls.append(dict(frame=str(frame), call_index_in_frame=str(index), pc="0423A2",
                              hv="1234", frame_count=str(count), seed_before=f"{seed:08X}",
                              roll=f"{roll_for(0x1234, count, seed):04X}", seed_after=f"{after:08X}"))
            seed = after
        width, timer = widths.get(frame, width), timers.get(frame, timer)
        row = dict(frame=str(frame), rng_seed=f"{seed:08X}", main_frame_count=str(count),
                   battle_priority="01" if kind == "preemptive" else ("00" if frame == end else "FF"),
                   battle_routine="10" if kind == "ambush" and frame == end else "00",
                   opening_continuation="000E", party_fighter_routine="0004", name_routine="0005",
                   enemy_6_reaction="08", enemy_7_reaction="08", opening_message_delay="0024",
                   opening_window_width=f"{width:04X}", opening_message_timer=f"{timer:04X}",
                   opening_timer=f"{timer:04X}")
        row.update({f"turn_{i:02}": "0000" for i in range(18)})
        if frame == end and kind == "ambush":
            row.update(turn_00="0006", turn_02="0007")
        frames.append(row)
    return frames, trace, rolls


class PriorityMeasurementTests(unittest.TestCase):
    def test_both_complete_paths_count_lag_and_seed_calls(self):
        for kind, counted, calls in [("preemptive", 58, 0), ("ambush", 10, 13)]:
            with self.subTest(kind=kind):
                result = analyze(*fixture(kind), kind)
                self.assertEqual(counted, result["after_origin"]["counted"])
                self.assertEqual(calls, result["after_origin"]["hv_calls"])
                self.assertIn(101, result["after_origin"]["lag_frames"])

    def test_window_identity_rejects_mutation_and_truncation(self):
        data = b"synthetic instruction window"
        windows = [("example", 2, 5, hashlib.sha256(data[2:7]).hexdigest())]
        verify_windows(data, windows)
        for changed in [data[:4], data[:3] + b"?" + data[4:]]:
            with self.assertRaises(ValueError):
                verify_windows(changed, windows)
        with self.assertRaisesRegex(ValueError, "verified US ROM"):
            verify_rom(data)

    def test_boundary_order_and_missing_endpoint_fail(self):
        frames, trace, rolls = fixture("preemptive")
        for bad in [[r for r in trace if r["pc"] != "001246"], list(reversed(trace))]:
            with self.assertRaises(ValueError):
                analyze(frames, bad, rolls, "preemptive")
        with self.assertRaisesRegex(ValueError, "misordered"):
            ordered(events(trace), [0x499C, 0x491E])

    def test_incomplete_window_and_wrong_timer_fail(self):
        for field, frame in [("opening_window_width", 109), ("opening_message_timer", 113)]:
            frames, trace, rolls = fixture("preemptive")
            next(r for r in frames if int(r["frame"]) == frame)[field] = "0000"
            with self.assertRaises(ValueError):
                analyze(frames, trace, rolls, "preemptive")

    def test_wrong_signed_path_and_party_in_enemy_queue_fail(self):
        for field in ["battle_priority", "turn_00", "enemy_6_reaction", "party_fighter_routine"]:
            frames, trace, rolls = fixture("ambush")
            frame = 111 if field == "turn_00" else 100
            next(r for r in frames if int(r["frame"]) == frame)[field] = "0001"
            with self.assertRaises(ValueError):
                analyze(frames, trace, rolls, "ambush")

    def test_missing_roll_or_wrong_queue_call_attribution_fails(self):
        frames, trace, rolls = fixture("ambush")
        with self.assertRaisesRegex(ValueError, "coverage"):
            analyze(frames, trace, rolls[:-1], "ambush")
        bad = copy.deepcopy(trace)
        index = next(i for i, r in enumerate(bad) if r["pc"] == "0056A8")
        bad[index - 1], bad[index] = bad[index], bad[index - 1]
        with self.assertRaisesRegex(ValueError, "nine agility"):
            analyze(frames, bad, rolls, "ambush")

    def test_seed_move_during_lag_and_missing_frame_fail(self):
        frames, trace, rolls = fixture("preemptive")
        bad = copy.deepcopy(frames)
        next(r for r in bad if r["frame"] == "101")["rng_seed"] = "00000000"
        with self.assertRaisesRegex(ValueError, "unexplained seed"):
            seed_schedule(bad, rolls, events(trace), 100, 160, 0x58B4)
        with self.assertRaisesRegex(ValueError, "missing frame"):
            seed_schedule([r for r in frames if r["frame"] != "101"], rolls,
                          events(trace), 100, 160, 0x58B4)

    def test_count_jump_and_duplicate_samples_fail(self):
        frames, trace, rolls = fixture("ambush")
        next(r for r in frames if r["frame"] == "103")["main_frame_count"] = "999"
        with self.assertRaisesRegex(ValueError, "discontinuity"):
            analyze(frames, trace, rolls, "ambush")
        frames, trace, rolls = fixture("ambush")
        with self.assertRaisesRegex(ValueError, "duplicate"):
            analyze(frames + [frames[0]], trace, rolls, "ambush")

    def test_local_us_windows_when_available(self):
        rom = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
        if not rom.is_file():
            self.skipTest("local US ROM unavailable")
        verify_rom(rom.read_bytes())

    def test_retained_artifact_mutation_and_unrecognized_fixture_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            frames, trace, rolls = fixture("ambush")
            for rows in (frames, trace, rolls):
                for row in rows:
                    row["frame"] = str(int(row["frame"]) + 24740)
            for name, rows in [("frames.csv", frames), ("pc.csv", trace), ("rng.csv", rolls)]:
                with (directory / name).open("w") as handle:
                    writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
                    writer.writeheader()
                    writer.writerows(rows)
            for name in ("input.tape", "ram_map.tsv", "rom"):
                (directory / name).write_text("synthetic input")
            receipt = dict(exit_status=0, patches=["24840:FFFFEF0C:58B3"],
                           input_sha256={key: sha(directory / file) for key, file in
                                         [("rom", "rom"), ("tape", "input.tape"),
                                          ("ram_map", "ram_map.tsv")]},
                           output_sha256={p.name: sha(p) for p in directory.iterdir()})
            path = directory / "receipt.json"
            path.write_text(json.dumps(receipt))
            with patch("oracle.priority_openings.ROM", directory / "rom"):
                self.assertEqual(24840, analyze_capture(directory, "ambush")["origin"])
                with (directory / "frames.csv").open("a") as handle:
                    handle.write("changed\n")
                with self.assertRaisesRegex(ValueError, "artifact changed"):
                    analyze_capture(directory, "ambush")
                receipt["patches"].append("24840:FFFFEE45:FF")
                path.write_text(json.dumps(receipt))
                with self.assertRaisesRegex(ValueError, "fixture assumptions"):
                    analyze_capture(directory, "ambush")


if __name__ == "__main__":
    unittest.main()
