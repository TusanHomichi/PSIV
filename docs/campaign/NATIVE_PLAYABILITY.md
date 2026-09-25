# Native playability — evidence ledger

## Current checkpoint — 2026-09-15

Connected routes now reach the Zema aftermath and a verified BioPlant `$A7`
save. STATE/ORDER, CROSSCUT gameplay and bounded ANTI-before-healing recovery
are implemented and verified. The onward six-fight attempt reaches `$A9`
but loses Hahn; the connected Rika join/escape remains unfinished.

The matching source passed 898 Rust tests. Code checkpoint `82de4a3` also
passed 927 Python tests, post-commit camp checks, formatting and strict Clippy.
See the [BioPlant ledger](BIOPLANT_NATIVE.md) for current save hashes and proof
limits, and the [roadmap](../ROADMAP.md) for the next milestones. Earlier dated
results below retain their original counts and scope.

## Owner objective

Play Phantasy Star IV on a modern computer using Godot, Rust, and the
ROM-derived JSON/assets. The installed game must run without an emulator.
The emulator remains a development reference. Completing the playable game
comes first; a modding frontend is a later goal.

The eventual mod interface may be a local browser editor with an API usable
by agents. Preserve the extracted retail pack, store mods separately, and
validate changes before launching Godot. No editor implementation is part
of this repair. Keep the default unmodified game usable without any editor
or agent service.

## Segment ledgers

- [Foundations and audit](PLAYABILITY_FOUNDATIONS.md) — audit context, the repairs in the opening slice, its local evidence, and the standing implementation priorities.
- [Academy](PLAYABILITY_ACADEMY.md) — command menus and technique/skill/item combat, the opening resistance repair, the first-boss Fission, dialogue and choices, the connected Academy route, camp restorative rules and battle return reload.
- [Holt and Tonoe](PLAYABILITY_HOLT_TONOE.md) — field TECH and SKILL recovery, the connected route through Holt, Acid Breath and attack ailments, map entry and live rock removal, field status and the first Zio encounter, field travel, and chest input.
- [Alshline and Zema](PLAYABILITY_ALSHLINE_ZEMA.md) — the connected Alshline progression and completion, instant-death commands, battle recovery techniques, STATUS and RIMIT, the THREAD correction, the Zema rescue and equipment restarts, and the BioPlant elevator checkpoint.
