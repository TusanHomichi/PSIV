# Cutscene return: the field reload

A scene's return value matters in exactly one place: `FieldRoutine_Cutscene`.
This note owns the rule, the cartridge's load with its flags, the frame table
and the port. Retail bytes and lines are `reference/ps4disasm/ps4.asm`.

## The rule

`FieldRoutine_Event` (`:120542-120548`) tests bit 15 of `Event_Index`:

| Dispatch | Routine | What it does with `d0` |
|---|---|---|
| plain event | `loc_5A27A` (`:120552-120568`) | nothing: clears `Field_Input_Buffer` and `Char_Move_Flags`, hands `Routine_Exit_Flags` bit 3 to `RunEventBattle`, calls `loc_5B368` (`:122258`, eight palette-buffer words) and returns through `VInt_Prepare`. **No reload on any value.** |
| cutscene | `FieldRoutine_Cutscene` (`:120738-120758`) | `jsr (a0)`, then `bne.s .skipmapreload`. **Zero** does `bset #2, Map_Load_Flags` and `move.w #8, Game_Mode_Index` (`GameMode_LoadFieldMap`); non-zero skips it. Both paths then clear the input and `Char_Move_Flags`, set `Game_Mode_Routine` to 0 and hand bit 3 to `RunEventBattle`. |

The doc comment on `SceneOp::Return` used to say a non-zero return suppresses
an *event's* reload. It does not: events never reload. `Event_IgglanovaBattle`'s
`moveq #1, d0` (`:152028`) is a convention; its battle hand-off is the
`bset #3, Routine_Exit_Flags` before it (`:152027`).

## What the load does with bit 2 set

`GameMode_LoadFieldMap` (`:107505-107635`). The flag byte is `Map_Load_Flags`
(`$FFFFEC4E`); a cutscene's own `bset`s (`Cutscene_MeetingKyra` sets bit 0,
some scenes set bit 7) are already in it when bit 2 joins them.

| Step | Source | Effect with bit 2 | Port |
|---|---|---|---|
| object and vehicle clear | `:107507-107521` | bits 0 and 2 skip `trap #0` over `Field_Objects_Memory`, `Tile_Anim_Memory` and `clr.w Vehicle_Index`: **the vehicle stays** | `apply_map_load_flags(MapLoad::Field)` |
| music | `:107536-107547` | a map with music, bit 0 **clear** and a raw id other than `Saved_Sound_Index` asks `AdjustMusicIDs`; a zero answer writes the saved word and sets `$FFFFECED` | `write_map_music` |
| party placement | `loc_53546`, `:110805-110823` | bit 2: `rts`. The party is not re-placed. | not run |
| `LoadMapObjects` | `:110924-110930` | bit 2: skipped. **No object is reloaded**; the live objects (a scene's despawns, moves and `$14` dialogue ids) stay. | `rewalk_map_retaining_objects` keeps the live `Npc`s and their dialogue ids |
| `LoadTreasureChests` | `:110966-110983` | chest art is decompressed before the bit-2 test; object creation is skipped, so lids stay | `attach_chests` keeps the live lids |
| `LoadTileAnimations` | `:111029` | runs (adds entries to the retained memory) | not modelled |
| `MapDataManager` | in the load | runs again over the retained objects | `effects::evaluate` |
| camera | `loc_53854`, `:111049` | zeroes all four cameras and re-seats them on `Character_1` | `camera_for_record` on the leader (or the vehicle) |
| tail resets | `loc_518D2`, `:107588-107596` | status counters, `$FFFFECE4 = $A` (ten free steps), `Field_Movement_Flags`, `$FFFFECF2`, `Event_Battle_Index = $FF` | `field_status.clock` and the encounter clock reset |
| flag spend | `:107594`, `:107628` | `andi.b #$80`, then `bclr #7`: the byte ends **zero**, and bit 7 skips `Pal_FadeIn` | `apply_map_load_flags` returns the bit |
| music stop | `:107619-107626` | `$FFFFECED` set: `Sound_StopAll` and one more `VInt_Prepare` | one extra frame |
| fade | `Pal_FadeIn`, `:107630` | 16 frames unless bit 7 | `fade` of the reload op |
| place name | `FieldRoutine_PlaceName`, `:136552-136646` | a window when `(Field_Map_Index, Field_Map_Index_2)` is in the table | `PlaceNameWindow`, 124 frames |

The object clear is why this lane exists. `Cutscene_MeetingKyra` ends with
`RefreshMap`, `Vehicle_Index = Ice Digger`, `bset #0, Map_Load_Flags`, return 0.
On the cartridge the reload keeps the digger and spends the bit. The port used
to ignore the return, so bit 0 lingered until the next warp, where it wrongly
spared the vehicle: the campaign route halted in `esper-mansion` with
"vehicle is 2, expected 0".

## Frames

The mode's frames are map-load work (Kosinski and Nemesis decompression, the
chunk walk, `MapDataManager`) that the CPU spends between `VInt_Prepare` waits,
so they differ per map: **34 to 92 rows** across the pack's 361 maps. They are measured, not
modelled.

**Method.** `oracle/reload_frames.py` runs the oracle host over tape 35's field
prefix (the fixture `tools/certify.py` `TAPE_35` pins). Frame 7000 loads the map
(mode 8); frame 7200 puts the field in Events with `Event_Index = $8000`, whose
`Event_NoEvent` is an immediate return with zero in `d0` (`CutscenePtrs[0]`,
`:120764`; `:144598`). The rows the log spends in game mode 8 from frame 7200
are the reload, `Pal_FadeIn` included. Two runs per map:

- **A**: `Saved_Sound_Index` equals the map's raw music id, so the music branch
  is not taken.
- **B**: the saved word differs, so the branch writes it. `B - A` is 1 on 347
  maps (the `$FFFFECED` frame) and 0 on 14, where the load's own alignment
  absorbs it; with bit 0 set B equals A. Both counts are tabled.

**Anchors against organic captures.**

| Capture | Map | Organic | Fixture |
|---|---|---:|---|
| tape 07, `Cutscene_PiataPrincipal` end (rows 10692-10746, control 10747) | `$14` | 55 | A 54, B 55: the scene clears the saved word, so it takes branch B |
| tape 35, Mota to Zelan landing (`docs/scenes/41_InsideSpaceship.md`) | `$18D` | 36 + 16 | A 52, B 53 |
| tape 35, Zelan to Mota landing | `$BF` | 26 + 16 | A 42, B 43 |

Both landings took the music branch for real (`Saved_Sound_Index` goes `$9B` to
`$89` with `$FFFFECED` raised, frames 9086-9087 and 8831-8832 of tape 35, read
with two extra RAM-map rows over `oracle/ship_flight.py`'s two legs), and still spent 52 and 42: the extra frame
was absorbed by the load's alignment there, while the principal's own load
spent it. The table therefore pins those three maps' music column to the
organic counts (`ORGANIC_MUSIC_TAKEN` in `oracle/reload_frames.py`: 55, 52, 42).
Whether a load absorbs the frame depends on the load's alignment, and the
fixture shows it on 347 of 361 maps, both landings included, where the flights
did not spend it: the music column elsewhere is the fixture's and good to a
frame.

The fixture also shows bit 7 removing exactly the 16 fade frames (map `$14`,
54 to 38) and bit 0 removing the music frame (55 to 54).

**Evidence retention.** Each `run_one` attempt now has its own directory in
the requested output root, including retries at other cells and first-load
proxies. It keeps `frames.csv`, `input.tape`, `stdout.log` and `receipt.json`.
The receipt records the command, working directory, exit status, elapsed time
and SHA-256 of the ROM, core, host, RAM map, tape and pack manifest. A failed
host's stdout and receipt also remain. These are forced fixtures; retaining
them does not make them organic captures. The oracle test checks their actual
A/B counts (54/55, 52/53 and 42/43), independently of the table's organic
overrides. It does not replay tape 07's end or tape 35's landings.

**Coverage.** All 361 pack maps have a row. 330 were measured with the
cutscene firing at the fixture's own load cell; 14 needed another cell (the
default one is a map-entry trigger, which starts an event before the cutscene
can fire); 17 (`PROXY_MAPS`: `$0 $1 $17 $5E $93 $98 $99 $C0 $C4 $EC $F0 $F6 $FB
$FC $14D $171 $190`) start an entry event at every cell, so their row is the
fixture's own frame-7000 load of the map, the same routine. Where both could be
measured (27 maps: 24 at random and the three anchors) the two differ by up to **two
frames** (7 equal, 19 off by one, 1 off by two) because the first load starts from a different
state, so a proxy row, like every row, is good to about two frames; the three
anchors above are exact.

**The port's count** is the table's row for the map (the music-branch column
when the load wrote the saved word), split as `setup = rows - 16` black frames
and `fade = 16` (0 with bit 7, which also takes the 16 off the total), plus 124
for the place-name window when the travel table names the pair, which both
landings measured. `SceneEnded` comes on the last of them.

### Limits

- The fixture is tape 35's state (opening finished, Chaz alone at the
  Spaceport row). Map-load cost also moves with state the fixture does not
  vary (a map's flag-gated objects and layout writes, the party): changing only
  which load wrote the saved music word moved map `$BF` by one frame in a
  trial. The rows are the cartridge's counts for that state; in another state a
  load is good to about one to two frames. The three organic anchors are exact.
- The 17 proxy maps are listed above.
- The place-name window's 124 frames are the two measured landings; another
  name's draw may differ.
- `LoadTileAnimations` re-adding entries and `Sound_StopAll`'s effect on audio
  are not modelled. The shell plays map music from its own state.

### Inputs to an exact timing model

[#95](https://github.com/TusanHomichi/PSIV/issues/95) tracks replacing the
approximate rows. Object/chest counts and map-data lengths alone do not
determine the load's elapsed frames. The following work must be accounted for
before a formula can certify a row:

- `GameMode_LoadFieldMap` (`$51790`, `:107505`) retains object and animation
  memory with bit 2 set. `LoadMapObjects` (`$536C0`, `:110924`) scans past
  object records, while `LoadTreasureChests` (`$5373E`, `:110966`) still
  decompresses both chest art streams before skipping creation.
- `loc_51A7A` adds vehicle art decompression when `Vehicle_Index` is nonzero.
  Nemesis (`:84605`) and Kosinski (`:85262`) execute content-dependent paths;
  their decoded sizes alone are insufficient.
- `MapDataManager` (`:107815`) branches on story state; `LoadTileAnimations`
  (`:111029`) scans retained allocation slots. `Field_UpdateObjects`
  (`:88893`) runs the retained objects through `Field_RunObjects` (`:89477`).
  Camera position also selects branches in `RefreshPlaneFG/BG`
  (`:111815`, `:111854`).
- `VInt_Prepare` (`:85745`) waits for the next hardware VBlank, rather than
  consuming a fixed duration. Its palette comparison and `VInt` (`:549`)
  DMA, Z80 and sound work affect timing. A cycle model needs the CPU/VDP phase
  entering the load as well as the work between waits.

The preceding scene's full instruction history need not determine that entry
phase. On the non-battle return path, `FieldRoutine_Cutscene` ends with
`jmp VInt_Prepare` (`:120758`); `MainGameLoop` (`:411-416`) dispatches the load
after that wait. This is a synchronization boundary. The interrupt's remaining
work still varies: `VInt` processes queued DMA and calls `JumpTo_UpdateSound`
before `rte` (`:613-635`). Palette/DMA work and the sound driver's current
commands, tempo and tracks must be modelled, or bounded tightly enough to
prove they cannot move the load into another frame bucket.

The oracle's [instruction-boundary timing trace](../../oracle/CAPTURE.md#instruction-boundary-timing)
can observe that boundary and the stages of the load without replacing it with
a forced first load. Watching the load entry, decompressor entries, DMA/wait
entries and fade entry separates elapsed work into measured segments. Those
observations support deriving a formula; a trace or three matching totals by
itself does not certify a formula for all maps.

The counted mode-8 span also includes work **before** `GameMode_LoadFieldMap`
starts. The three organic returns distinguish these boundaries (tape frames
are 1-based; cycles are the oracle's frame-relative master cycles):

| Capture | After mode-8 write, `$5A558` | Before jump to wait, `$5A57A` | Load entry, `$51790` |
|---|---|---|---|
| tape 07, principal | 10693 / 895708 | 10694 / 47541 | 10695 / 56193 |
| tape 35, Mota to Zelan | 9086 / 426390 | 9086 / 427818 | 9087 / 72343 |
| tape 35, Zelan to Mota | 8831 / 185224 | 8831 / 186652 | 8832 / 99515 |

The pinned core's NTSC frame is 262 lines of 3,420 master cycles, or 896,040.
The principal's mode write finishes only 332 cycles before that frame ends;
VInt interrupts the short tail **before** it reaches `VInt_Prepare`. The wait
is armed in the next frame and dispatches the load in the frame after that.
The flights arm the wait in their mode-write frame. Thus the principal has
two logged mode-8 rows before load entry, the flights one. A formula for the
table's metric must include this return-to-load interval, not only work inside
the load routine. The observed totals remain 55, 52 and 42.

For interrupt-body observations, the retail VInt sound call is at `$632`
(`jsr $D0000`), its register restore at `$638`, and its common `rte` at `$63C`.
Watching these with the entry at `$524` partitions the handler. The entry-to-
RTE interval alone excludes exception entry and RTE execution overhead, so
subtracting it from a stage does not yet give intrinsic decompression cost.

Even the fade's fixed 16 is a property of the measured palettes.
`Pal_FadeIn` (`$421D4`, `:85897-85947`) clears the current palette and increments
each differing nibble toward the target. For maximum target nibble `M`, it
calls `VInt_Prepare` **`M + 2` times**: one in `VDP_EnableDisplay`, `M`
incrementing iterations and one unchanged iteration. Turning those waits into
elapsed frames additionally requires that the work between waits fits before
the next VBlank. The measured 16 corresponds to `M = 14`.

The addresses above were checked against the US retail ROM with SHA-256
`511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a`;
fork-only scene rewrites do not establish these timing rules. Neither a new
forced fixture nor agreement with the three table overrides removes the
17 proxies or certifies the other maps' organic timing.

## The port

- `rust/psiv-runtime/src/scene_return.rs`: reads the scene's `d0` at
  `Returned`, starts the reload for a cutscene's zero, owns the frames and
  emits the ops.
- `rust/psiv-runtime/src/map_change.rs`: `reload_field_after_cutscene` (state)
  and `rewalk_map_retaining_objects`, which the battle return shares (it is the
  same load with bit 0).
- `rust/psiv-core/src/scenes/field_reload.rs`: the measured table.
- `PresentationOp::FieldReload { setup, fade }` and
  `PresentationOp::PlaceNameWindow` are all the shell is told. It draws one
  `SceneFadeIn` of `setup + fade` frames; nothing at `SceneEnded`.
- `FlightFieldReload` and `FlightArrivalName` are gone: the flights' landing is
  this reload, with the same 36 and 26 setup frames from tape 35.

## What the shell changed, and the certified pairs

The shell's fixed 55-frame `SceneEnd` cover, started at `SceneEnded` for every
high-bit cutscene begun from an interaction, is retired
(`rust/psiv-godot/src/transitions.rs`, `runtime_events.rs`, `field_visuals.rs`,
`debug.rs`, `lib.rs`). A zero-returning cutscene's `FieldReload { setup, fade }`
op now tears the scene presentation down, draws the field map under black and
starts one `SceneFadeIn` of `setup + fade` frames
(`rust/psiv-godot/src/cutscene/ops.rs`); `SceneEnded` then only hands control
back and lifts the letterbox. A scene that does not reload (an event, a
non-zero return) finishes its presentation at `SceneEnded` as before, with no
cover. `FlightFieldReload` and `FlightArrivalName` are gone from `SceneOp`;
the arrival name is `PlaceNameWindow`, read by the session's flight view.

No certified pair crosses a cutscene end: `opening-p1` and `-p2` are inside
`Event_GameStart` (an event), `meeting-rika` is clone tick 160 of `$8007`, long
before its end, and `ship-menu` is the destination menu of `$800D`, before the
flight, so the certified pairs do not exercise this change. A cutscene
end itself has no pair; its shell timing is covered by the runtime test of the
frames and the transition unit test, not by a capture.
