# `Cutscene_ZioDefeated`

- **Retail bytes:** `$075A12..$075FC7` inclusive, 1,462 bytes.
- **Pointer:** `CutscenePtrs[$0B]` at `$05A580`; scene event is `$800B`.
- **Trigger:** Nurvus B4 Part2 `$D3`, `RunEvent_ZioDefeated` (`$20`): Zio
  Nurvus `$65` set and Gryz Gone `$68` clear.
- **Data:** `post_rika_cutscenes.rs`, `ZIO_DEFEATED` (129 ops).

## Clone audit

The source range is the retail `grand_cross=0` body. Grand Cross-only scene
content is excluded. The final party removals are retail state operations.
The fork's `bugfixes=1` Chaz revival is absent from the US image and is not
ported; see the [source correction](../source-notes/disassembly-discrepancies.md#ziodefeateds-fork-only-chaz-revival).

## Retail transcription

| Op(s) | ROM offset | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-12 | `$075A12..$075AA6` | init/fade, panel `$6D`, Tandle `$CE` pulses, waits, raw dialogue entry 0 | panel/sound/wait/dialogue |
| 13-17 | `$075ABE..$075B15` | map update 60, tone ramp, destroy/chunks, map update 60, fade out at `$075B10` | waits/presentation |
| 18-28 | `$075B16..$075BA5` | clear `$FFFFC300`/`$FFFFC340`, `RefreshMap` (`$D3`, bit 3 set), place Demi at `($1E8,$1C0)`, fade in, pan camera to Demi (`speed=1`), turn Demi left, map update 10, turn Demi down, sprites off and `Event_RunDialogue` (`$075BA0`) | `DespawnNpc`/`LoadMap`/`PlaceActor`/`PanCamera`/`Face`/`Wait`/dialogue |
| 29-80 | `$075BA6..$075C55` | set follow mode, pan camera to Demi, `Event_StepObject` 16px up backwards (`$075BD8`), face up, `Event_StepObject` 32px up (`$075BF4`), map update 60, `SFXID_RidingElevator` (`$075C04`), `loc_75EE0` (10 FG chunk triples `$AD..$C0` at `(14..=16,12)` × 8 map-update frames), map update 60, `Event_StepObject` 48px up (`$075C2A`), map update 60, `$E0` (`$075C3A`), `loc_75F54` (reverse 10 triples × 6 map-update frames), map update 60 | `SetFollowMode`/`PanCamera`/`StepObjectAlongFacing`/`WriteMapChunks`/`Wait` |
| 81-96 | `$075C56..$075CE9` | `Event_RunDialogue` (`$075C56`); `SFXID_PowerDown` (`$075C5C`), map update 120, `Palette_Line_2` words `$0C`/`$0D` := `$024`/`$240` (`$075C70`), 60 frames; `Event_RunDialogue` (`$075C94`); a fresh screen, panel `$70`, 20 frames, sprites on and `Event_RunDialogue5` (`$075CD4`); stop music, map update 20 | fade/dialogue/sound/palette/panel |
| 97-101 | `$075CEA..$075D71` | Motavia load, music `$8C`, fade in at `$075D66`, `jsr (Event_MotaSpaceportAppearing)` at `$075D6C` — whose `loc_6B7FA` writes BG chunk (26,45)←`$3F` and sets Mota Spaceport `$66` (op 101, `ps4.asm:144825-144826`: before the second `RefreshMap`, so that load carries the spaceport's page hook) | sound/fade/`WriteMapChunks` |
| 102-107 | `$075D72..$075D9B` | fade out, init, fade in; load `DialogueTree36` at `$075D84`; a second init and fade in at `$075D90/$075D96` | fade/tree |
| 108-118 | `$075D9C..$075DFD` | panel sequence `$73/$74/$75`, waits 30/60/20, sprites on, resume dialogue | panel/dialogue |
| 119-122 | `$075E30..$075E8B` | clear field objects; remove Gryz `$04`, clear status; remove Demi `$06`, clear status | party/roster writes |
| 123-128 | `$075E8C..$075EDF` | final Motavia load from BioPlant B4 Part2 at `($6C,$B8)`, down; `RefreshMap` at `$075EB6`, fade-in at `$075EBC`; set `$68,$66,$61`; return 1, `rts` at `$075EDE` | `LoadMap`, flags, return |

The intermediate load is Motavia `$00`, previous BioPlant B4 Part2 `$AC`,
start `($70,$BC)`. The final load is the same map/previous map with start
`($6C,$B8)`. Removing Gryz and then Demi compacts the post-Rune party to
`[Chaz, Rika, Rune]`.

The fixed-fade call order is Init, In, Out, In, Out, Init, In, In,
Out, Init, In, Init, In, In. `$075B10` follows the second 60-tick map
update, before the display-off boundary. The three calls at
`$075D72..$075D7E` run after the spaceport helper returns, and the two at
`$075D90/$075D96` run after the tree load and before panel `$73`.
The elevator helpers (`$075EE0..$075FC7`) and the spaceport helper
(`$06B6F4..$06B86D`, before its table) contain no fixed-fade calls.
`InitVRAMAndCRAM` owns its internal fade-out; `Event_RunDialogue5` owns its
closing fade before Gryz and Demi leave. Neither is another scene fade op.
DMA boundaries remain tracked separately ([#605](https://github.com/TusanHomichi/PSIV/issues/605)).

## Tree loads

`$075D84` loads `DialogueTree36` between the Motavia fade-in and the
`$73`/`$74`/`$75` panels, and the `popdlg` after them resumes a line from that
tree. The load is not inside a revision block, so it was a plain transcription
gap rather than the missed-revision-block class, and the port's resume read the map's own tree
until it was added. See the [revision audit](REVISION_AUDIT.md).

## Verification

`scenes::fixed_fades_image` checks the US call sites;
`scenes::zio_fade_boundaries` traces the fades around maps, dialogue, panels
and party removals, with missing/misplaced fade controls.
The runtime's synthetic arc test resolves battle 6, runs this cutscene, and asserts the
final map `$00`, party composition, Gryz Gone `$68`, Mota Spaceport `$66`,
Plate Engine `$61`, and preservation of Chaz's HP/status. The focused core
test `zio_defeated_scene` runs the scene with dead and living-statused Chaz,
checks his record is unchanged, and checks only Gryz/Demi's statuses clear.

## Chain stop

The next relevant trigger is `RunEvent_Reunion` (`$50`), which writes
`$801F` only after Elsydeon `$D9` is set and Reunion `$DA` is clear. That is a
later Dezo Spaceport/Kuran beat, not an automatic continuation of this Zio
arc. Molcum `$40` still has only the null map-event slot, so there is no retail
Molcum scene to insert between these records.
