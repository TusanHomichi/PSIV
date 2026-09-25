"""Turning the port's first divergences into the manifest the test reads.

`every_fixture_replays_as_recorded` needs one entry per diverging fixture, and
its panic is one fixture at a time: harvesting a directory's entries by hand
would take a test run each. `replay/data.rs`'s `dump_manifest_entries` - an
`#[ignore]`d test - prints every diverging fixture's first divergence as one
JSON object per line; this reads that dump, assigns each finding to a
**cluster**, and writes `replay_fixtures/divergences.json`.

    CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml -p psiv-core \\
        -- --ignored --nocapture dump_manifest_entries \\
        > build/lane-evidence/findings.txt
    python3 -m oracle.sweep.manifest --dump build/lane-evidence/findings.txt \\
        --manifest rust/psiv-core/src/battle/replay_fixtures/divergences.json

A cluster is the worklist's unit: one cause, its fixtures and its fix scope. The
clusters below are a reading of the evidence, not a computation - which is why
they are written down here, in a committed file, rather than guessed at from a
`kind` string: `signature` decides which finding belongs to which cluster, and
the assignment is asserted to be exactly one cluster per entry. `--clusters`
prints the table the ledger (`docs/BATTLE_ORACLE_SWEEP.md`) carries: the slug,
its title, and each fixture with the divergence that put it there.

Every cluster here is a **port rule** the cartridge has and `psiv-core` does
not: the ledger's §4 is the triage that settled which findings those are, and
the harness artifacts it found were fixed rather than listed
(`docs/BATTLE_ORACLE_SWEEP.md` §4-§5; `oracle/fixture/*` and `replay/compare.rs`
are where they were).
"""
from __future__ import annotations

import argparse
import collections
import dataclasses
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
FIXTURES = (ROOT / "rust" / "psiv-core" / "src" / "battle" / "replay_fixtures")
MANIFEST = FIXTURES / "divergences.json"
LEDGER = "docs/BATTLE_ORACLE_SWEEP.md"


@dataclasses.dataclass(frozen=True)
class Cluster:
    """One cause in the worklist: what to call it, and what belongs to it."""

    slug: str
    title: str
    #: Where the ledger writes it up.
    anchor: str
    #: Whether this finding is the cluster's. One cluster must match.
    signature: object  # callable(finding, fixture) -> bool


def load_dump(path: pathlib.Path) -> list[dict]:
    """The finding lines `dump_manifest_entries` printed, in file order."""
    findings = []
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        findings.append(json.loads(line))
    if not findings:
        raise SystemExit(f"{path}: no finding lines (run the #[ignore]d "
                         f"dump_manifest_entries test first)")
    return findings


def action_text(finding: dict, fixture: dict) -> str:
    """What the log's action did, for the entry's `action` field."""
    round_ = next((entry for entry in fixture["rounds"]
                   if entry["round"] == finding["round"]), None)
    if round_ is None:
        return f"round {finding['round']}"
    actor = None
    for action in round_["actions"]:
        if action["start_frame"] <= finding["frame"] <= action["end_frame"]:
            actor = action
            break
    if actor is None:
        return f"round {finding['round']}'s queue build"
    shape = actor.get("kind", "attack")
    if shape != "attack":
        shape += f" ${actor.get('ability', 0):02X}"
    return (f"round {finding['round']}: actor {actor['actor']} "
            f"({shape}), {actor['roll_count']} roll(s), "
            f"{len(actor['targets'])} target(s)")


def summarise(finding: dict) -> str:
    """The first divergence in one line, as the ledger's tables carry it."""
    return (f"f{finding['frame']} ({finding['kind']}): the log has "
            f"{finding['expected']}, the port {finding['actual']}")


def entry_for(finding: dict, fixture: dict, cluster: Cluster) -> dict:
    """One `divergences.json` entry: the finding, its numbers and its cluster."""
    return {
        "round": finding["round"],
        "frame": finding["frame"],
        "kind": finding["kind"],
        "action": action_text(finding, fixture),
        "expected": finding["expected"],
        "actual": finding["actual"],
        "round_rolls": finding.get("round_rolls"),
        "ledger": f"{LEDGER}#{cluster.anchor}",
    }


def load_fixture(fixtures: pathlib.Path, name: str) -> dict:
    return json.loads((fixtures / f"{name}.json").read_text())


def assign(findings: list[dict], clusters: list[Cluster],
           fixtures: pathlib.Path) -> dict[str, list[dict]]:
    """{cluster slug: [finding, ...]}, with the assignment's own checks."""
    grouped: dict[str, list[dict]] = collections.OrderedDict(
        (cluster.slug, []) for cluster in clusters)
    for finding in findings:
        fixture = load_fixture(fixtures, finding["fixture"])
        matches = [cluster for cluster in clusters
                   if cluster.signature(finding, fixture)]
        if len(matches) != 1:
            raise SystemExit(
                f"{finding['fixture']}: {finding['frame']} matches "
                f"{len(matches)} cluster(s) "
                f"({', '.join(cluster.slug for cluster in matches)}): exactly "
                "one cluster owns a finding")
        grouped[matches[0].slug].append(finding)
    return grouped


def manifest_document(findings: list[dict], clusters: list[Cluster],
                      fixtures: pathlib.Path) -> dict:
    """The `divergences.json` document, keyed by fixture name."""
    grouped = assign(findings, clusters, fixtures)
    entries = {}
    for cluster in clusters:
        for finding in grouped[cluster.slug]:
            entries[finding["fixture"]] = entry_for(
                finding, load_fixture(fixtures, finding["fixture"]), cluster)
    return {"fixtures": entries}


def print_clusters(grouped: dict[str, list[dict]],
                   clusters: list[Cluster]) -> None:
    """The ledger's cluster rows: one block per cluster, its fixtures listed."""
    for cluster in clusters:
        rows = grouped[cluster.slug]
        print(f"\n### {cluster.title} (`{cluster.slug}`, {len(rows)} fixture(s))")
        if not rows:
            print("(no fixtures)")
            continue
        for finding in rows:
            print(f"- `{finding['fixture']}` - {summarise(finding)}")


def main(argv: list[str] | None = None) -> int:
    parsed = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parsed.add_argument("--dump", default=str(ROOT / "build" / "lane-evidence"
                                              / "findings.txt"))
    parsed.add_argument("--manifest", default=str(MANIFEST))
    parsed.add_argument("--fixtures", default=str(FIXTURES))
    parsed.add_argument("--clusters", action="store_true",
                        help="print the clusters and their fixtures instead of "
                             "writing the manifest")
    parsed.add_argument("--check", action="store_true",
                        help="also fail when the manifest on disk differs from "
                             "the one this run builds")
    arguments = parsed.parse_args(argv)

    findings = load_dump(pathlib.Path(arguments.dump))
    clusters = CLUSTERS
    fixtures = pathlib.Path(arguments.fixtures)
    grouped = assign(findings, clusters, fixtures)
    if arguments.clusters:
        print(f"{len(findings)} finding(s) in {len(clusters)} cluster(s)")
        print_clusters(grouped, clusters)
        return 0
    document = manifest_document(findings, clusters, fixtures)
    text = json.dumps(document, indent=2, sort_keys=True) + "\n"
    manifest = pathlib.Path(arguments.manifest)
    if arguments.check and manifest.read_text() != text:
        print(f"{manifest} is not what this dump builds", file=sys.stderr)
        return 1
    manifest.write_text(text)
    print(f"wrote {manifest} ({len(document['fixtures'])} entry/entries, "
          f"{len(clusters)} cluster(s))")
    return 0


def _kind(name: str):
    """A signature that accepts one divergence kind, and nothing else."""
    return lambda finding, fixture: finding["kind"] == name


def _targets_single(finding: dict, fixture: dict) -> bool:
    """The log resolves exactly one slot and the port swings at another."""
    return (finding["kind"] == "targets"
            and finding["expected"].count("FighterId") == 1)


def _ability_mismatch(finding: dict, fixture: dict) -> bool:
    """The log ran one ability and the port another.

    The action's own byte is the ability the cartridge *executed*, which for a
    carrier with a nonzero `condition_ids` entry is the conditional ability its
    AI instruction picked - not the one the regular roll landed on
    (`ps4.asm:19158-19168`; `generated/enemies.json`'s `ai`).
    """
    return (finding["kind"] == "ability"
            and finding["expected"].startswith("ability $"))


def _critical_value(finding: dict, fixture: dict) -> bool:
    """A critical's damage differs, with the round's draws agreeing."""
    rolls = finding.get("round_rolls") or {}
    return (finding["kind"] == "value" and "hit flag 01" in finding["expected"]
            and rolls.get("log") == rolls.get("port"))


def _missing_draw(finding: dict, fixture: dict) -> bool:
    """The round draws one roll fewer than the log's frames hold.

    Either the count is the whole finding (`draws`) or the stream is shifted by
    the missing roll and the first thing it moves is a damage word (`value`) -
    `formation_07`'s swing reads the tiebreak draw as its hit roll.
    """
    rolls = finding.get("round_rolls") or {}
    if finding["kind"] == "draws":
        return True
    return (finding["kind"] == "value"
            and rolls.get("port", 0) < rolls.get("log", 0))


#: The clusters, in the order the ledger lists them. A finding belongs to the
#: first whose `signature` accepts it - and to exactly one, which `assign`
#: asserts. Each `anchor` is the ledger's own heading for the cluster; the
#: cause and the fix scope are the ledger's prose, and these titles are what
#: the ledger's rows are headed with.
CLUSTERS: list[Cluster] = [
    Cluster(
        slug="party-retarget",
        title="A swing whose commanded enemy has fallen lands on another slot",
        anchor="w1-a-swing-whose-commanded-enemy-has-fallen",
        signature=_targets_single),
    Cluster(
        slug="enemy-ai-conditional",
        title="The enemy's AI instruction, not the ability roll, picks the "
              "ability",
        anchor="w2-the-enemys-ai-instruction",
        signature=_ability_mismatch),
    Cluster(
        slug="critical-bonus",
        title="A critical reads the attack power's low byte, not the word",
        anchor="w3-the-critical-bonus",
        signature=_critical_value),
    Cluster(
        slug="retarget-tiebreak",
        title="The retarget scan's tiebreak draw is missing from the round",
        anchor="w4-the-tiebreak-draw",
        signature=_missing_draw),
]


if __name__ == "__main__":
    raise SystemExit(main())
