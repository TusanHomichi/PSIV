# `Cutscene_Alshline`

Native continuation repairs and current evidence are in
[`SCENE_DIALOGUE.md`](SCENE_DIALOGUE.md). The object entrance uses 64
half-pixel steps, and field dialogue clears the preceding panel stack on
the full map reload.

- **Retail bytes:** `$0741E6..$074555` inclusive, 880 bytes.
- **Pointer:** `CutscenePtrs[$05]` from `$05A580`; scene event `$8005`.
- **Trigger:** Zema `$24`, trigger `$15`, Alshline flag `$32` set and Zema
  Igglanova flag `$33` clear.
- **Data:** `next_arc.rs`, `ALSHLINE_OPS`.

## Clone audit

This is one of the known conditional scene rewrites. The retail body is the
`grand_cross=0` branch; `grand_cross=1` includes the missing
`script/scenes/Alshline/event.asm`. The 880-byte retail range was disassembled
and byte-diffed against the surviving retail branch. The Grand Cross branch
was not allowed to fill in the gaps.

## Retail transcription

The native op list preserves every panel/DMA and wait literal, with `dbf`
counts corrected to frame counts:

1. Initialise VRAM/CRAM, stop music, fade in.
2. Create panels `$1A,$1B,$1C,$1D,$1E`, DMA after each; wait
   `20,60,10,10,90` frames, playing `SFX_BarrierBroken` (`$E6`) on panels
   `$1B..$1E`.
3. Destroy all panels, DMA, wait 40; create `$1F`, DMA, play
   `Music_AHappySettlement` (`$9E`), wait 60; create `$20`, DMA, wait 120.
4. Destroy panels one-by-one in the retail loop (represented by the explicit
   destroy/DMA edge), create `$21` and `$22`, DMA each, play barrier SFX on
   `$22`, wait 90.
5. Load tree 3, enable cutscene sprites, run entry `$67`; copy the palette
   buffer, stop music, wait 60 map-update ticks, fade in and wait 90 frames.
6. Create panels `$25` and `$26`, DMA each; play `Music_MotabiaVillage`
   (`$83`), wait 60 then 10; resume the saved dialogue and call `RecoverStats`.
7. Write Zema as the current map, Motavia as previous, start `($44,$3E)`,
   face left, alignment `$10`, clear load flag bit 3 and refresh. Clear NPCs
   0 through 6.
8. Decompress `Art_Igglanova` from ROM `$12951A` to tile `$3A5`; stage object
   `$188` in NPC slot 7 at `($1E0,$A0)`. Move the leader to `($1E0,$100)`,
   move the camera to `($1E0,$A0)`, play fusion SFX (`$D9`), run the `$8000`
   step-object effect and move the camera to `($1E0,$E0)`.
9. Load tree 3, disable cutscene sprites, run entry `$68`; clear NPC 7, set
   `EventFlag_IgglanovaZema` (`$33`), remove item `$8D` Alshline and reorder
   the inventory; load tree 4 and save `Music_MotabiaTown` (`$84`).
10. Set map-load bits `$80` and `$08`, request battle index 1 through the
    existing `BattleRequested` path, then return `d0 = 1`.

The native interpreter adds no fake battle implementation here. The scene
blocks at `StartBattle` and resumes only after the runtime supplies
`SceneInput::BattleFinished`.

## Verification

The arc test drives the long panel/dialogue sequence, resolves the event
battle, and then asserts flag `$33`. `RemoveItem` is tested through the same
scene state path and safely no-ops when a fixture does not carry Alshline.
