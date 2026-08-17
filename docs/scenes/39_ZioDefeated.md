# `Cutscene_ZioDefeated`

- **Retail bytes:** `$075A12..$075FC7` inclusive, 1,462 bytes.
- **Pointer:** `CutscenePtrs[$0B]` at `$05A580`; scene event is `$800B`.
- **Trigger:** Nurvus B4 Part2 `$D3`, `RunEvent_ZioDefeated` (`$20`): Zio
  Nurvus `$65` set and Gryz Gone `$68` clear.
- **Data:** `post_rika_cutscenes.rs`, `ZIO_DEFEATED` (43 ops).

## Clone audit

The source range is the retail `grand_cross=0` body. Grand Cross-only scene
content is excluded. The final party removals and the retail bug-fix tail are
state operations, not presentation guesses.

## Retail transcription

| Op(s) | ROM offset | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-12 | `$075A12..$075AA6` | init/fade, panel `$6D`, Tandle `$CE` pulses, waits, raw dialogue entry 0 | panel/sound/wait/dialogue |
| 13-18 | `$075ABE..$075B10` | map update 60, tone ramp, destroy/chunks, map update 60, fade out | waits/presentation |
| 19-23 | `$075B22..$075D1A` | intermediate Motavia load from BioPlant B4 Part2, place Demi, camera/facing, elevator choreography | `LoadMap` + typed presentation |
| 24-32 | `$075D72..$075DEA` | panel sequence `$73/$74/$75`, waits 30/60/20, DialogueTree36, resume dialogue | panel/tree/dialogue |
| 33-37 | `$075E30..$075E8C` | clear field objects; remove Gryz `$04`, clear status; remove Demi `$06`, clear status | party/roster writes |
| 38 | `$075E8C` | retail bug-fix: clear Chaz status and revive from zero HP | `ReviveIfDead(Chaz)` |
| 39-42 | `$075E92..$075EE0` | final Motavia load from BioPlant B4 Part2 at `($6C,$B8)`, down; set `$68,$66,$61`; return 1 | `LoadMap`, flags, return |

The intermediate load is Motavia `$00`, previous BioPlant B4 Part2 `$AC`,
start `($70,$BC)`. The final load is the same map/previous map with start
`($6C,$B8)`. Removing Gryz and then Demi compacts the post-Rune party to
`[Chaz, Rika, Rune]`.

## Verification

The end-to-end arc test resolves battle 6, runs this cutscene, and asserts the
final map `$00`, party composition, Gryz Gone `$68`, Mota Spaceport `$66`,
Plate Engine `$61`, and Chaz's revive/status result.

## Chain stop

The next relevant trigger is `RunEvent_Reunion` (`$50`), which writes
`$801F` only after Elsydeon `$D9` is set and Reunion `$DA` is clear. That is a
later Dezo Spaceport/Kuran beat, not an automatic continuation of this Zio
arc. Molcum `$40` still has only the null map-event slot, so there is no retail
Molcum scene to insert between these records.
