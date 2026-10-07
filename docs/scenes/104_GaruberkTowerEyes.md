# The Garuberk Tower's eyes

| Event | Routine | Retail bytes | Reached by | Data |
|---|---|---|---|---|
| `$0039` | `Event_GaruberkTwEyeAction1` | `$06F8A4..$06F995` | interaction area on `GaruberkTower_Part2` `$19A` (20..21,30..31), parameter 5, temp flag `$14` | `EYE_ACTION_1` (8 ops) |
| `$003A` | `Event_GaruberkTwEyeAction2` | `$06F996..$06FA33` | interaction area on `GaruberkTower_Part5` `$19D` (56..57,34..35), parameter 6, temp flag `$16` | `EYE_ACTION_2` (4 ops) |

- **Pointers:** `EventPtrs[$39]`, `EventPtrs[$3A]` at `$05A2B4`.
- **Data:** `garuberk_events.rs`.

## Clone audit

Both bodies were read from the US image; every instruction word matches
`ps4.asm:148951-149073`.

## Retail transcription

`Event_GaruberkTwEyeAction1`:

| Ops | Retail primitive | Scene record |
|---:|---|---|
| 0-2 | `KosDecomp` of `$1C7460`, `$1C79F0`, `$1C7FE0` to `RAM_Start`, `$FFFF09E0`, `$FFFF13C0`: the eye's three frames | `Presentation` `LoadSceneAsset` |
| 3 | `move.b #SFXID_Fusion, (Sound_Index).l` | `PlaySound` `$D9` |
| 4 | `moveq #TempEveFlag_GrbkTwEyeball, d0` / `jsr TempEveFlags_Set` | `SetFlag` temp `$14` |
| 5-6 | `moveq #$3B, d7`: sixty passes of `loc_6F91E` and `VInt_Prepare` | `Presentation` `GaruberkEyeArtCycle` 60, `WaitFrames` 60 |
| 7 | `KosDecomp` of `$1C8F7A` into `Map_Layout_FG` and of `$1C92BA` into `Map_Layout_BG` | `ReplaceMapLayout` |

`Event_GaruberkTwEyeAction2`: `SFXID_Fusion`, 120 passes of `loc_6F9BC` (the same art cycle) and
`VInt_Prepare`, then `TempEveFlag_GrbkTwEyeball2`.

The flags are what the maps read at load: `MapDataMan_GaruberkTowerPart2` (`ps4.asm:109346-109358`)
decompresses the same two layouts while `$14` is set, and `MapDataMan_GaruberkTowerPart6`
(`:109363-109375`) the Part6 pair (`$1CA2AE`, `$1CA54E`) while `$16` is set. The pack carries both as
`layout_replace` effects and layout variants. Each pair decompresses one plane that is the map's own
layout (Part2's FG): that write reproduces the base, and the runtime's effect walk now takes it as
such instead of refusing the map (`layout_replace_variant`, `rust/psiv-runtime/src/effects.rs`).

The first eye changes the live map: `ReplaceMapLayout` is a new scene op. The runtime installs the
variant decoded from the two sources and keeps every object where it stands
(`replace_scene_map_layout`). The second eye changes nothing on Part5; its flag opens Part6 the next
time the party enters it.

`GaruberkEyeArtCycle` is a new presentation record (the eye frames' ping-pong DMA, `$9E0` bytes per
frame, `$4F0` words to VRAM command `$4060` on even frames); Godot logs it with the other records it
does not draw (`rust/psiv-godot/src/cutscene/ops.rs`).

## In the route (C8)

`garuberk-tower` touches both eyes: the first opens Part2's way to its far door, the second opens
Part6's way to the stairs to Part7.
