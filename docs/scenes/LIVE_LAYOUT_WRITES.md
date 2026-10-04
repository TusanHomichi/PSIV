# Live layout writes: the tiles a scene changes under the party

`MapDataManager` runs at map load and only at map load
([map effects](../field/MAP_EFFECTS.md#2-the-dispatcher)), so a story flag that
changes while the party stands on a map changes nothing until the map reloads.
Three cartridge mechanisms make a change *live* anyway, and H19 was the port
missing the first of them:

1. **the scene writes the layout itself.** `GetMapLayoutOffset` (`$53514`) or
   `GetMapLayoutChunkBG` (`$53EEC`) resolves a layout address from literal
   column and row operands, the routine writes chunk ids into it, and
   `RefreshPlaneBG` (`$540FA`) redraws the plane. Collision is read from that
   layout, so the write is walkable the same frame. **This is the mechanism
   both H19 cases use** — `Event_JuzaDefeated` (`$06FB74`) and
   `Event_MachineCenterAppearing` (`$06B5CE`) — and neither calls `RefreshMap`.
2. the overworld page hooks (`loc_53BDC`/`loc_53CD4`), which retail replays per
   1 KB page copy and this port applies at map construction
   ([map effects §12](../field/MAP_EFFECTS.md#12-native-overworld-page-hook-consumption-2026-09-23)).
   The two overworld writes below have page-hook *twins* gated on the same flag,
   which is how the pack corroborates their coordinates.
3. a whole-map reload the scene asks for, which is the ordinary `LoadMap` op.

## Census

Every call site of the two layout-offset routines in the US image, attributed
to its owning scene by the retail pointer tables (`EventPtrs` at `$5A2B4`,
`CutscenePtrs` at `$5A580`). The name column is the clone's label for the same
table slot, whose address was checked against the image.

### Collision-bearing writes the port transcribes

| Owner | Site | Write | Port |
|---|---|---|---|
| `Event_BioPlantDoorOpening` `$08` | `$06B94C`, `$06B962` | two-chunk door animation | `WriteActorMapChunks` ×3 (atlas tiles in the pack) |
| `Event_ElevatorDoorOpening` `$13` | `$06C2D2` | one-chunk door animation | `WriteActorMapChunks` ×5 |
| `Event_RidingElevator` `$14` | `$06C376`, `$06C41C` | open/close animation | `WriteActorMapChunks` |
| `Event_RuneFlaeli` `$27` | `$06DBB6` | BG `(15,14)` ← `$0C,$0D,$14,$15` | `RestoreMapChunks` (the ids are the map's own base chunks) |
| `Event_JuzaDefeated` `$41` | `$06FB74` | BG `(12,9),(13,9),(12,10),(13,10)` ← `$23,$1A,$19,$1B` | **was presentation-only** → `RestoreMapChunks` |
| `Event_MachineCenterAppearing` `$06` | `$06B5CE` | BG `(57,90)` ← `$D3` | **was dropped** → `WriteMapChunks` |
| `Cutscene_PsycoWand` `$800A` | `$07580A`, `$07581E` | FG `(9,5)` ← `$8F`, BG `(9,6)` ← `$90` | **was `ReloadMapChunks`** → `WriteMapChunks` |
| `Cutscene_ZioDefeated` `$800B` (via `Event_MotaSpaceportAppearing` `$07`) | `$06B808` | BG `(26,45)` ← `$3F` | **was dropped** → `WriteMapChunks` |
| `Cutscene_CrashLanding` `$800F` | `$076E00` (called at `$076AE6`) | Raja Temple BG `(47,9),(48,9),(47,10),(48,10)` ← five frames ending `$56,$57,$5E,$5F` | `WriteMapChunks` ×5; raw `$50..$5F` atlas tiles, [retail decode](43_CrashLanding.md#live-layout-write-retail-correction-67) |
| `Event_TylerGraveOpening` `$44` | `$06FCEE` | BG `(10,12)` ← `$47`, with the twelve grave objects drifting aside (`DriftNpcs`) | `WriteMapChunks` + `DriftNpcs`, [92](92_TylerGraveOpening.md) |

Several of these were transcribed as presentation or not at all, which is
the class H19 reports: a scene op that changes collision was recorded as art.
`Event_JuzaDefeated` and `Cutscene_PsycoWand` were on the campaign route and
had workarounds in it (`go_to_map` round trips); the rest were latent.

### Collision-bearing writes still not transcribed

| Owner | Site | Write | Why not |
|---|---|---|---|
| `Event_VahFortMovingPlatform1..2` `$15`,`$16`, `Event_WpnPlntMovingPlatform1..4` `$17`-`$1A` | `$06C4C4`…`$06CD8A` | moving-platform chunk swaps | not transcribed at all — no Vahal Fort / Weapon Plant scene exists in the registry |
| `Event_WreckageEngine` `$29` | `$06DD14` | one chunk | not transcribed |
| `Event_GaruberkTwDoorOpening1/2` `$35`,`$36`, `Event_GaruberkTwDoorEntered1/2` `$37`,`$38` | `$06F454`…`$06F83A` | tower door animations | not transcribed |
| `Event_SilenceTmDoorOpening` `$42` | `$06FC04`, `$06FC16` | two-chunk door | not transcribed (the file's next body after `Event_JuzaDefeated`) |

The last three groups are the already-recorded direct bodies outside this
slice ([12_ArcTriggerCensus](12_ArcTriggerCensus.md)); listing them here keeps
the *class* visible: each is a live collision change, not decoration.

### Visual-only writes on a collision plane that is not read

| Owner | Site | Write | Note |
|---|---|---|---|
| `Cutscene_ZioDefeated` `$800B` | `$075EEA` | FG `(14,12)` `,$15,12`,`(16,12)` ← `$B9,$BD,$BB` (seven-row animation) | Motavia `$000` reads collision from BG (`collision.plane`), so this is the spaceport's *picture*; left as presentation, recorded here rather than dropped. |

## How the port consumes a write

`SceneOp::WriteMapChunks { chunks }` = `(chunk x, chunk y, replacement chunk
id)` in world chunk coordinates, straight from the routine's operands, and the
runtime applies it through the same path as the door scenes: the id resolves to
an atlas tile (pixels and its four collision nibbles), the named chunks' old
patches are dropped, the map is rebuilt with the new collision, and a
`MapRefreshed` reaches the renderer. Two atlas lookups are possible:

1. a raw chunk id — `patch_tiles.tiles[].chunk_id`, the ordinary case
   (`Event_BioPlantDoorOpening`, `Cutscene_PsycoWand`, all MapDataManager
   doors);
2. a **composed overworld tile** — the overworlds hold some chunks only as the
   resolved result of a page hook, in `overworld_patches`, whose
   `collision_chunk_id` is the raw id a scene writes. The Machine Center's
   `$D3` at (57,90) is exactly the twin of the hook gated on Machine Center
   `$43`, and the Mota Spaceport's `$3F` at (26,45) of the hook gated on Mota
   Spaceport `$66` — so both scene writes resolve against today's pack without
   a rebuild.

`SceneOp::RestoreMapChunks { chunks }` is the other shape: `(chunk x, chunk y,
base id)` when the row a scene's animation table ends on *is* the map's own
layout at those chunks — `Event_RuneFlaeli`'s rock and `Event_JuzaDefeated`'s
stairs. The runtime verifies each literal id against the base layout before
removing that chunk's overlays, so a mistranscribed coordinate fails loudly
instead of opening the wrong wall.

The crash helper uses raw tiles. The earlier census misattributed it to
Dezolis and interpreted `$40(a1)` as an X displacement. Retail first loads
Raja Temple (`$076820`, `$07684A`; `ps4.asm:155869-155876`), whose BG width is
64 (`$1A9A12`; `ps4.asm:254149`), so `$40(a1)` is the next row. Its own
Kosinski chunk source is `$1ABDA8` (`ps4.asm:254150`). The pack now
emits all 16 temple animation chunks via `scene_patch_chunks` and records
each contributing map in `manifest.map_effects.scene_patch_chunks`.
No Dezolis tiles are added. The
[line-by-line helper and collision basis](43_CrashLanding.md#live-layout-write-retail-correction-67)
replace that incorrect row.

## Verification

`rust/psiv-runtime/src/scene_map_tests.rs`:

- `juzas_stairs_open_on_the_live_map_when_the_event_writes_them` — the stairs
  read `MapChange` the moment the event ends, the effect's own closed-chunk
  write is gone, and a fresh build of the same flags agrees; the negative
  control is the same map without the event, which keeps the shut layout and
  the effect's chunk write.
- `the_machine_center_door_opens_on_the_live_overworld` — all four door cells
  read `MapChange` right after the scene, with no battle and no reload; the
  negative control is the same flags without the event.

Those two H19 checks needed no pack regeneration. For #67,
`rust/psiv-runtime/src/scene_crash_tests.rs` checks five live temple frames,
six ticks between writes, all 16 affected collision cells, the later flag and
same-map reload, and a decoded-slot reconstruction of the final frame. Its
negative control sets `$85` without a write or reload: the lower-right roof
stays walkable. All five writes are required while `$85` is still clear, so
the load hook cannot substitute for the scene's animation.

`tests/test_crash_landing.py` checks the retail map/stride/plane operands,
all five source rows, plain atlas pixels and priority pixels against a
one-chunk map render, collision nibbles, and the manifest census. Negative
controls reject truncated/missing/early terminators and exclude Dezolis from
the temple chunk census. See the [#67 receipt](43_CrashLanding.md#67-delivery-receipt)
for the rebuilt pack and verification boundaries.
