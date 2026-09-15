# Implementation archive

Preserved from the README at code checkpoint `82de4a3`. This accumulated
record contains historical counts and isolated-fixture results. Use the
[current overview](../README.md) and [roadmap](ROADMAP.md) for present scope;
a transcribed scene or rendered frame does not establish a playable campaign.

## Historical implementation record (2026-08-16 through 2026-08-18)

The runtime has moved past the original field-only vertical slice. These are
closed and tested in the current tree:

- battle data, the headless battle engine, the real battle screen with enemy
  overlay composition, and the Igglanova event battle;
- flag-gated map effects, encounters, chests, inventory, and the eleven-seat
  roster;
- shop inventories, locations, prices, inn rules, portraits, greeting
  selectors, and the decoded-layout shop UI;
- the dual-plane field camera and EC24/EC25/EC26 gates, plus the packed random
  wander families and all three extracted speed records, including shared-RNG
  tape coverage;
- field sprites, dialogue, and the pack/data/runtime joins that drive them;
- event scenes through the opening act and the Zema/Tonoe arc (Holt → Rune →
  Dorin → Alshline → Zema aftermath, `docs/scenes/`), with scripted actor
  movement live in the field;
- save/SRAM: retail layout scouted from ROM (`docs/SAVE_SCOUT.md`),
  byte-compatible three-slot serialization, camp STATE slot chooser, and
  `PSIV_LOAD_SLOT` boot hooks;
- equip/unequip: scouted rules (`docs/EQUIP_SCOUT.md`), transactional core
  seam, and the interactive camp EQUIP screen;
- party order: STATE/ORDER pick, undo, cancel and automatic last-slot
  completion, with unchanged character records and verified SAVE/CONTINUE
  (`docs/PARTY_ORDER.md`);
- sound extraction: all 557 tracks (music/SFX/special), 327 voices, DAC
  banks, and the full driver vocabulary as raw records in
  `runtime-pack/sound/` (`docs/SOUND_EXTRACTION.md`);
- sound playback foundation: `psiv-sound` (Nuked-OPN2 YM2612 core, SN76489
  PSG, SMPS-derived driver interpreter) with register-log fixture tests and a
  Godot `AudioStreamGenerator` output path.
- sound integration: the runtime pack now feeds real voices, envelopes,
  tracks, and DAC sample banks through the live driver; map music, field and
  event battle themes, victory, and current menu/cursor SFX routes are wired
  through Godot (`docs/SOUND_INTEGRATION.md`);
- save payload completion: the full character-record byte census (every byte
  named or proven padding), vehicle records, settings, and battle macros all
  round-trip at exact retail offsets (`docs/SAVE_SCOUT.md`); the only
  remaining save divergence is three per-slot files instead of one SRAM
  device;
- event scenes through MeetingRika (`docs/scenes/` 26–30); FortuneTeller and
  AfterFortuneTeller are Grand Cross hack content with no retail bodies, so
  the retail chain resumes at the next retail beat;
- the front door: Sega logo → title reveal → Press Start → save menu
  (START/CONTINUE/ERASE DATA gating from real slot state), built to the
  decoded layout numbers and RMSE-checked against oracle frames
  (`docs/TITLE_BOOT.md`), with all `PSIV_DEBUG_*`/`PSIV_LOAD_SLOT` fast
  paths intact;
- scene presentation consumption: ordered `ScenePresentation` events drive
  fades, panels, palettes, sprite gating, scene audio, and saved-music
  restore in Godot; the opening cinematic renders through the same op family
  (historical live opening comparison: **rmse=6.586813**),
  the additive pack now carries all 7 decoded `LoadArt` writes, 6 standalone
  temporary-object keys, and the generic shopkeeper window/portrait surfaces
  (`docs/SCENE_PRESENTATION.md`); the panel/opening extraction is part of
  `python -m psiv_tools pack` (`presentation/`);
- event scenes through the post-Zio Zelan/Dezolis/Kuran handoff
  (`docs/scenes/` 31–50: Zio's fall, Wren, spaceship travel, crash landing,
  Landale, Kuran and the first Dark Force beat), with roster/heal/revive,
  inventory and typed presentation records; Molcum is a proven event dead end,
  and the later retail Reunion gate remains `$50 → $801F`;
- battle SFX: ordered battle-timeline events carry the retail SFX at the
  retail moment — per-weapon attack sounds, miss, death, menu routes, EB
  sequence sounds over music through the driver's priority arbitration; all
  153 enemies dispatch their exact per-enemy attack SFX from the decoded
  `EnemyAttackOffs` tables, proven live (`docs/SOUND_INTEGRATION.md`,
  `docs/BATTLE_ANIMATIONS.md`);
- vehicles: all three field machines — selector-specific terrain/movement and
  dismount rules, story-live Ice Digger, vehicle skill menu/use surface,
  per-map CRAM-line-3 palettes, mounted battle entry, 32 battle backgrounds,
  and the `$96` battle theme, with retail tape 29 receipts
  (`docs/VEHICLES.md`);
- two exact-frame retail-paced RMSE certifications: opening narration
  page 1 (clone t3450 vs oracle 4000, **rmse 6.587**) and page 2 (clone
  t4440 vs oracle 5200, **rmse 6.578**), captured under Xvfb with the
  `PSIV_DEBUG_SCENE_TICKS` timeline (`docs/SCENE_PRESENTATION.md`);
- dialogue-embedded actions: all 266 retail `Ctrl::Action` codes across 55
  entries dispatch for real — 178 extracted panels through the cutscene
  stack at the typewriter's byte position, sounds through the live driver,
  the `$45` flag write, and the four bespoke palette effects
  (`docs/DIALOGUE_ACTIONS.md`); scene dialogue plays over the blanked field
  exactly as the oracle shows;
- the full story engine: scenes 51–87 carry the Dezo campaign in retail
  dispatch order — LeRoof, Kyra, Eclipse Torch, Dark Forces 2 and 3, Seth,
  the Aero Prism, the tower trials, Elsydeon, Reunion, and the Profound
  Darkness battle request; only the `$8021` Ending surface remains, and the
  recorded retail boundaries live in `docs/scenes/88_RetailBoundaries.md`;
- enemy attack motion: decoded movement fields and mapping records drive 60
  enemies' exact frame playback (393 verified PNGs) with the proven
  body-flash fallback for the rest — no guessed art or motion
  (`docs/BATTLE_ANIMATIONS.md`).

These closed in the same wave: the `$8021` Ending (docs/scenes/89, 367 ops
— panels, staff roll, credits, Termi finale, `Game_Cleared_Flag`) plus Raja
Sick and Rykros, with the full-campaign arc test running title → Profound
Darkness → Ending on one snapshot; the oracle scroll receipt (all scroll
state zero, split disabled) and the verified `(1,1)` plane placement
(MeetingRika 59.4 → 36.3); enemy animation coverage doubled (120/153 exact,
956 frame PNGs, the remaining 32 records structurally classified across 16
shared routine bodies; DarkForce1's `$6000` palette bit is the one named
partial); seven more bespoke NPC families (374 placements, tape-02 clean at
350 columns); all eight `VehicleSkillData` effects; and a real ERASE DATA
(0x1400-byte payload wipe, header preserved, byte-level tests).

These closed in the same wave: the receipt-derived GPGX/RGB565 color ramp
(`docs/COLOR_PIPELINE.md`) with the **opening narration certified
pixel-identical to the emulator (rmse 0.000000, both pages)**; the bespoke
NPC census drained to **zero** (all 949 placements across 138 symbols
transcribed, gate writes audited, tape 30 receipt); and the enemy
animation census drained to **zero deferred** — all 153 enemies exact on
every surface (1,355 attack PNGs, all 16 remaining routine bodies decoded
from oracle sprite-table receipts, DarkForce1 included).

**All six selected reference screens — opening
narration pages 1 and 2, MeetingRika, title, battle command idle, and the
camp root — were recorded pixel-identical to the GPGX oracle at rmse 0.000000**
(`docs/SCENE_PRESENTATION.md`, closing receipts dated 2026-08-18).
This certifies those captures, not complete gameplay or continuous playback.

The previously recorded remaining item (superseded by the audit above):

1. Natural (non-fixture) vehicle oracle tapes — `31_natural_land_rover.tape`
   provides a 131,130-frame power-on prefix through the real Prof. Holt
   scene with a clean natural replay receipt. The full route to
   `GettingLandRover` remains owed; tape 29's mount/dismount/mounted-battle
   receipt is still fixture-only.

The extraction and rendering receipts above remain useful. Gameplay
completion follows the current native-playability ledger.

## The Rust runtime

`rust/` holds a Cargo workspace (see `docs/RUNTIME_DESIGN.md` for the design record): `psiv-data` (fail-closed schema over the runtime pack), `psiv-core` (the deterministic field, event, persistence, and battle engine — integer-only, dependency-free, floats banned by the compiler), and `psiv-runtime` (the bridge and game shell that `psiv-godot` drives). The headless golden path still passes: spawn in Piata, walk into the academy doorway, and the engine warps to `MapID_PiataAcademy` at exactly the cell and facing the cartridge's transition table stores; all 359 packed maps convert into live engine maps. The current Godot path also runs real encounters and the opening-act Igglanova battle through the retail-shaped battle screen. `cargo test` from `rust/`.
