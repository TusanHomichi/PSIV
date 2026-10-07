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
| `LoadTreasureChests` | `:110966-110983` | bit 2: skipped; lids stay | `attach_chests` keeps the live lids |
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

**Method.** `tests/reload_frames.py` runs the oracle host over tape 35's field
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
organic counts (`ORGANIC_MUSIC_TAKEN` in `tests/reload_frames.py`: 55, 52, 42).
Whether a load absorbs the frame depends on the load's alignment, and the
fixture shows it on 347 of 361 maps, both landings included, where the flights
did not spend it: the music column elsewhere is the fixture's and good to a
frame.

The fixture also shows bit 7 removing exactly the 16 fade frames (map `$14`,
54 to 38) and bit 0 removing the music frame (55 to 54).

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
flight. All twelve pairs were run on this change under the heavy lock
(`python3 tools/certify.py --no-build`, receipt
`build/certify/20261006T230810Z-1b01ee7/`): 12/12 at `rmse=0.000000`. A cutscene
end itself has no pair; its shell timing is covered by the runtime test of the
frames and the transition unit test, not by a capture.
