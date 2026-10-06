"""Replay a whole campaign run natively and compare every chapter save.

Run this entry point under the shared heavy flock. It feeds one completed
`psiv-campaign run` (its `run.tape` and `report.json`) through the generic
Godot tape driver in a single continuous replay from New Game, and compares the
slot snapshot with each chapter's saved bytes at the frame the chapter ended
on. It writes one receipt per chapter and a summary.

The replay is one process because a chapter replayed from its predecessor's
save is a different game: a save carries no RNG state and no frame counter, so
a loaded game restarts both (`psiv-campaign run --from-chapter` plays other
battles, documented in docs/campaign/CAMPAIGN_RUNNER.md). Only the full tape
from New Game is the route's evidence, so only a prefix of it can be replayed
(`--until-chapter`), never a single chapter.

`--continue-probes` is the other half: it loads each chapter save in a fresh
Godot process through the ordinary title CONTINUE and requires the snapshot to
re-encode the same bytes. The replay never loads a save, so persistence is
proven there, one chapter at a time.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys
import time

try:  # imported as `tools.verify_native_route` by tests
    from tools import verify_native_tape
except ModuleNotFoundError:  # direct `python3 tools/verify_native_route.py`
    import verify_native_tape


FNV_OFFSET = 0xCBF29CE484222325
FNV_PRIME = 0x100000001B3
RECEIPT_VERSION = 1


def fnv1a64(data: bytes) -> int:
    """The tape codec's save hash (psiv-runtime `fnv1a64`)."""
    value = FNV_OFFSET
    for byte in data:
        value = ((value ^ byte) * FNV_PRIME) & 0xFFFFFFFFFFFFFFFF
    return value


class RouteError(Exception):
    """The run directory cannot be verified; nothing was replayed."""


def load_chapters(run_dir: Path, until: str | None = None) -> tuple[dict, list[dict], int]:
    """The completed report, the chapters up to `until` (all by default), each
    with its resolved save and the frame it ended on, and the run's chapter
    count. Every save read must hash to what the report recorded."""
    report_path = run_dir / "report.json"
    try:
        report = json.loads(report_path.read_text())
    except (OSError, ValueError) as error:
        raise RouteError(f"cannot read {report_path}: {error}") from error
    if report.get("result") != "completed" or not report.get("chapters"):
        raise RouteError(f"{report_path} is not a completed run's report")
    records = report["chapters"]
    if sum(record["frames"] for record in records) != report.get("frames"):
        raise RouteError(f"the chapters do not add up to the report's {report.get('frames')} "
                         "frames")
    if until is not None:
        ids = [record["id"] for record in records]
        if until not in ids:
            raise RouteError(f"the run has no chapter {until!r}")
        records = records[:ids.index(until) + 1]
    chapters = []
    end = 0
    for record in records:
        end += record["frames"]
        save = run_dir / f"{record['index']:02d}-{record['id']}" / "slot_1.sram"
        try:
            data = save.read_bytes()
        except OSError as error:
            raise RouteError(f"cannot read chapter save {save}: {error}") from error
        if f"{fnv1a64(data):016x}" != record["save_fnv"]:
            raise RouteError(f"{save} does not hash to the report's save_fnv "
                             f"{record['save_fnv']}: the run directory is stale or edited")
        chapters.append({"index": record["index"], "id": record["id"],
                         "frames": record["frames"], "battles": record.get("battles"),
                         "end_frame": end, "save": save, "save_fnv": record["save_fnv"]})
    return report, chapters, len(report["chapters"])


def checkpoint_list(chapters: list[dict]) -> list[dict]:
    return [{"frame": chapter["end_frame"], "label": f"{chapter['index']:02d}-{chapter['id']}",
             "expect_save": str(chapter["save"])} for chapter in chapters]


def chapter_receipts(chapters: list[dict], native: dict | None) -> list[dict]:
    """One receipt per chapter from the native report's checkpoint results."""
    reached = {item["label"]: item
               for item in (native or {}).get("checkpoints", [])}
    receipts = []
    start_frame = 0
    previous_ms = 0
    for chapter in chapters:
        label = f"{chapter['index']:02d}-{chapter['id']}"
        item = reached.get(label)
        receipt = {
            "version": RECEIPT_VERSION, "chapter": chapter["id"], "index": chapter["index"],
            "frames": chapter["frames"], "start_frame": start_frame,
            "end_frame": chapter["end_frame"], "battles": chapter["battles"],
            "expected_save": str(chapter["save"]), "expected_save_fnv": chapter["save_fnv"],
            "reached": item is not None,
            "match": bool(item and item["match"]),
        }
        if item is not None:
            wall_s = (item["elapsed_ms"] - previous_ms) / 1000
            previous_ms = item["elapsed_ms"]
            receipt.update({
                "wall_s": round(wall_s, 3),
                "frames_per_s": round(chapter["frames"] / wall_s, 1) if wall_s > 0 else None,
                "snapshot_sha256": item["snapshot_sha256"],
                "first_differing_byte": item.get("first_differing_byte"),
                "divergent_snapshot": item.get("divergent_snapshot"),
            })
        receipts.append(receipt)
        start_frame = chapter["end_frame"]
    return receipts


def summary_table(receipts: list[dict]) -> str:
    lines = ["| # | Chapter | Frames | Battles | Wall (s) | Frames/s | Native save bytes |",
             "| --- | --- | ---: | ---: | ---: | ---: | --- |"]
    for item in receipts:
        verdict = ("match" if item["match"] else
                   ("DIFFERS at frame %d (first byte %s)" % (item["end_frame"],
                                                            item["first_differing_byte"])
                    if item["reached"] else "not reached"))
        wall = f"{item['wall_s']:.1f}" if "wall_s" in item else "-"
        speed = f"{item['frames_per_s']:.0f}" if item.get("frames_per_s") else "-"
        lines.append(f"| {item['index']} | {item['chapter']} | {item['frames']:,} | "
                     f"{item['battles']} | {wall} | {speed} | {verdict} |")
    return "\n".join(lines) + "\n"


def continue_probe(chapter: dict, probe_dir: Path, campaign_bin: Path, pack: Path,
                   timeout: int) -> dict:
    """One fresh Godot process loads the chapter's save through the ordinary
    title CONTINUE (a zero-pad save-start tape) and must re-encode it to the
    same bytes. This is the persistence evidence each retired driver began
    with; the continuous replay never loads a save, so it does not carry it."""
    probe_dir.mkdir(parents=True)
    tape = probe_dir / "probe.tape"
    made = subprocess.run([str(campaign_bin), "save-probe-tape", str(chapter["save"]),
                           str(tape), "0"], capture_output=True, text=True)
    label = f"{chapter['index']:02d}-{chapter['id']}"
    if made.returncode != 0:
        return {"chapter": label, "pass": False, "error": made.stderr.strip() or "no probe tape"}
    started = time.monotonic()
    code = verify_native_tape.main([
        "--tape", str(tape), "--from-save", str(chapter["save"]),
        "--expect-save", str(chapter["save"]), "--pack", str(pack),
        "--out", str(probe_dir / "native"), "--timeout", str(timeout)])
    receipt_path = probe_dir / "native" / "receipt.json"
    receipt = json.loads(receipt_path.read_text()) if receipt_path.is_file() else {}
    return {"chapter": label, "pass": bool(code == 0 and receipt.get("pass")),
            "wall_s": round(time.monotonic() - started, 3),
            "save_sha256": receipt.get("source_sha256_before"),
            "snapshot_sha256": receipt.get("snapshot_sha256"),
            "source_unchanged": receipt.get("source_sha256_before")
            == receipt.get("source_sha256_after"),
            "receipt": str(receipt_path)}


def run_continue_probes(chapters: list[dict], out: Path, campaign_bin: Path, pack: Path,
                        timeout: int) -> int:
    results = [continue_probe(chapter, out / "continue" / f"{chapter['index']:02d}-{chapter['id']}",
                              campaign_bin, pack, timeout) for chapter in chapters]
    passed = all(item["pass"] and item.get("source_unchanged", False) for item in results)
    (out / "continue-summary.json").write_text(json.dumps(
        {"version": RECEIPT_VERSION, "pass": passed, "chapters": results},
        indent=2, sort_keys=True) + "\n")
    print(f"native route: CONTINUE probes {'PASS' if passed else 'FAIL'}; "
          f"{sum(1 for item in results if item['pass'])}/{len(results)} chapter saves load and "
          f"re-encode identically; summary={out / 'continue-summary.json'}")
    return 0 if passed else 1


def parser() -> argparse.ArgumentParser:
    command = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    command.add_argument("--run-dir", type=Path, required=True,
                         help="the `psiv-campaign run --save-dir` of a completed run; it "
                              "holds run.tape, report.json and the NN-id chapter saves")
    command.add_argument("--out", type=Path, required=True,
                         help="a fresh directory for the receipts")
    command.add_argument("--until-chapter",
                         help="replay the prefix of the tape that ends with this chapter")
    command.add_argument("--pack", type=Path, default=verify_native_tape.ROOT / "runtime-pack")
    command.add_argument("--timeout", type=int, default=21600,
                         help="seconds for the whole replay (default six hours)")
    command.add_argument("--drop-pad-at", type=int,
                         help="negative control: replace one pad byte with neutral")
    command.add_argument("--continue-probes", action="store_true",
                         help="instead of the replay, load every chapter save in its own fresh "
                              "Godot process through the ordinary title CONTINUE and require "
                              "the snapshot to re-encode the same bytes")
    command.add_argument("--campaign-bin", type=Path,
                         default=verify_native_tape.ROOT / "rust/target/release/psiv-campaign",
                         help="the psiv-campaign binary that cuts the probe tapes")
    return command


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    run_dir = args.run_dir.resolve(strict=True)
    try:
        report, chapters, chapters_in_run = load_chapters(run_dir, args.until_chapter)
    except RouteError as error:
        print(f"native route: REFUSED: {error}")
        return 1
    tape = run_dir / "run.tape"
    if not tape.is_file():
        print(f"native route: REFUSED: {tape} is missing")
        return 1
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    if args.continue_probes:
        if not args.campaign_bin.is_file():
            print(f"native route: REFUSED: {args.campaign_bin} is missing; build it with "
                  "`cargo build --release -p psiv-campaign`")
            return 1
        return run_continue_probes(chapters, out, args.campaign_bin.resolve(),
                                   args.pack, args.timeout)
    checkpoints = out / "checkpoints.json"
    checkpoints.write_text(json.dumps(checkpoint_list(chapters), indent=2) + "\n")
    full_run = len(chapters) == chapters_in_run
    final_pad_save = run_dir / "route" / "slot_1.sram"
    tape_args = ["--tape", str(tape), "--pack", str(args.pack), "--out", str(out / "native-tape"),
                 "--timeout", str(args.timeout), "--checkpoints", str(checkpoints),
                 "--expect-save", str(chapters[-1]["save"])]
    if not full_run:
        tape_args += ["--stop-at", str(chapters[-1]["end_frame"])]
    elif final_pad_save.is_file():
        # The runner's last camp SAVE is the file the native game must write.
        tape_args += ["--expect-written-save", str(final_pad_save)]
    if args.drop_pad_at is not None:
        tape_args += ["--drop-pad-at", str(args.drop_pad_at)]
    started = time.monotonic()
    code = verify_native_tape.main(tape_args)
    wall_s = time.monotonic() - started
    receipt_path = out / "native-tape" / "receipt.json"
    tape_receipt = json.loads(receipt_path.read_text()) if receipt_path.is_file() else None
    native = tape_receipt.get("native_report") if tape_receipt else None
    receipts = chapter_receipts(chapters, native)
    chapters_dir = out / "chapters"
    chapters_dir.mkdir()
    for item in receipts:
        (chapters_dir / f"{item['index']:02d}-{item['chapter']}.json").write_text(
            json.dumps(item, indent=2, sort_keys=True) + "\n")
    passed = bool(code == 0 and tape_receipt and tape_receipt.get("pass")
                  and all(item["match"] for item in receipts))
    summary = {
        "version": RECEIPT_VERSION, "pass": passed,
        "scope": "full route" if full_run else f"prefix through {chapters[-1]['id']}",
        "run_dir": str(run_dir), "tape": str(tape), "tape_sha256": verify_native_tape.sha256(tape),
        "run_digest": report.get("digest"), "frames": chapters[-1]["end_frame"],
        "chapters_total": len(chapters),
        "chapters_matched": sum(1 for item in receipts if item["match"]),
        "wall_s": round(wall_s, 3), "native_receipt": str(receipt_path),
        "ordinary_save_acknowledged": (tape_receipt or {}).get("ordinary_save_acknowledged"),
        "camp_save_acks": (native or {}).get("camp_save_acks"),
        "final_pad_save_matches": (tape_receipt or {}).get("ordinary_save_matches")
        if full_run and final_pad_save.is_file() else None,
        "extension_sha256": ((tape_receipt or {}).get("code_identity_before", {})
                             .get("extension", {}).get("sha256")),
        "pack_manifest_sha256": (tape_receipt or {}).get("pack_manifest_sha256"),
        "identity_stable": (tape_receipt or {}).get("identity_stable"),
        "mapped_library_matches_preflight":
            (tape_receipt or {}).get("mapped_library_matches_preflight"),
        "source_head": ((tape_receipt or {}).get("source_identity_before", {}).get("head")),
        "chapters": [f"{item['index']:02d}-{item['chapter']}" for item in receipts],
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    (out / "summary.md").write_text(summary_table(receipts))
    print(summary_table(receipts), end="")
    print(f"native route: {'PASS' if passed else 'FAIL'}; {summary['chapters_matched']}/"
          f"{summary['chapters_total']} chapters match; {wall_s:.1f}s; summary={out / 'summary.json'}")
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
