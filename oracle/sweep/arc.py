"""The Motavia status/stat capture recipe, using the shared case runner."""
from __future__ import annotations

from .forced_cases import Case, FIXTURE_ROOT, ROOT, run_cases

FIXTURES = FIXTURE_ROOT / "arc_motavia"


def Entry(formation: int, delay: int, capture_rounds: int, rounds: int,
          note: str, event: bool = False) -> Case:
    """Declare a legacy arc capture in the common case format."""
    kind = "event" if event else "formation"
    name = (f"{kind}_{formation:02X}_d{delay}" if event
            else f"{kind}_{formation:X}_d{delay}")
    return Case(name, note, None, capture_rounds, keep=rounds,
                formation=None if event else formation,
                event=formation if event else None, delay=delay,
                augment_cells=True)


ENTRIES = (
    Entry(0xD7, 0, 8, 8, "VOICE: FlyScreamr x3; the party is paralyzed and loses"),
    Entry(0xD5, 0, 8, 8, "VOICE: one FlyScreamr; sleepers wake on the round-end roll"),
    Entry(0xD6, 0, 8, 8, "VOICE: two FlyScreamr; wake rolls, a repeat on a sleeper"),
    Entry(0xD6, 1, 8, 6, "VOICE: two FlyScreamr, a second shift of the same fight"),
    Entry(0x100, 0, 10, 10, "STASISBALL: Blauzen; two landings and one skipped target"),
    Entry(0x1A3, 0, 8, 8, "STASISBALL: Goldine; three landings"),
    Entry(0x1A5, 5, 8, 1, "STASISBALL: LifeDeletr, before its MICROMISSL"),
    Entry(0x119, 0, 12, 3, "SEALS, DORAN, SEALS: Greneris"),
    Entry(0x119, 2, 6, 3, "SEALS, GELUN, DORAN: Greneris"),
    Entry(0x119, 8, 8, 3, "VOL kills, RIMIT sleeps, DORAN: Greneris"),
    Entry(0x1C6, 0, 8, 8, "VOL: two BloodSaber kill the party"),
    Entry(0x1C6, 1, 8, 3, "VOL: two BloodSaber, three kills in three rounds"),
    Entry(0x1E1, 0, 8, 1, "VOL: SoldrFiend (a miss), before BLADESHINE"),
    Entry(0xFB, 1, 8, 3, "EVIL EYE: two Haunt; the party wins"),
    Entry(0xFB, 2, 8, 3, "EVIL EYE: two Haunt, a second shift of the same fight"),
    Entry(0x16D, 1, 8, 2, "EVIL EYE: two Spector, before CORRSION"),
    Entry(0x12E, 10, 8, 2, "RIMIT: two TechPlant, before GIZAN"),
    Entry(0x12E, 12, 8, 2, "RIMIT: two TechPlant, a second shift of the same fight"),
    Entry(0xD8, 0, 10, 10, "DEBAN then its guard: ShadowSabr"),
    Entry(0xD2, 2, 8, 8, "Fusion: two Zol slugs become a MetaSlug"),
    Entry(0x116, 2, 8, 8, "GIRES: TechMaster beside a ZiosGuard"),
)


def main(argv: list[str] | None = None) -> int:
    return run_cases(ENTRIES, FIXTURES, ROOT / "build" / "arc",
                     __doc__.splitlines()[0], argv)


if __name__ == "__main__":
    raise SystemExit(main())
