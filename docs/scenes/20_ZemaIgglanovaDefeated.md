# `Cutscene_ZemaIgglanovaDefeated`

Native continuation repairs and current evidence are in
[`SCENE_DIALOGUE.md`](SCENE_DIALOGUE.md). The 11-op transcription now
includes the original panel-dialogue flag write at `$074588`; the full map
reload restores all seven residents after the temporary battle staging.

- **Retail bytes:** `$074556..$0745DD` inclusive, 136 bytes.
- **Pointer:** `CutscenePtrs[$06]` from `$05A580`; scene event `$8006`.
- **Trigger:** Zema `$24`, trigger `$17`, flag `$33` set and `$37` clear.
- **Data:** `next_arc.rs`, `ZEMA_IGGLANOVA_DEFEATED_OPS`.

## Clone audit

The clone's `grand_cross=1` side includes the absent
`script/scenes/ZemaIgglanovaDefeated/event.asm`; the retail body survives in
the `grand_cross=0` branch. The 136-byte retail stream was byte-diffed against
that surviving branch. The fork was not used to infer the post-battle map
placement.

## Retail transcription

1. If saved music is not `Music_MotabiaTown` (`$84`), play and save `$84`.
2. Initialise VRAM/CRAM, fade in, load tree 3 and run cutscene entry `$69`.
3. Add 1000 meseta and set `EventFlag_AfterIgglanovaZema` (`$37`).
4. Refresh Zema (`$24`) with previous map `$FFFF`, start `($3C,$20)`, face
   up, alignment 4, and clear map-load flag bit 3.
5. Return `d0 = 0`, allowing the cutscene caller to reload the field.

The runtime map-load barrier makes step 4 a real recast rather than a request
followed by scene ops still addressing the old map.

## Verification

The arc test resolves the Alshline battle, drives this scene to its zero-return
edge, and asserts Zema plus flag `$37`.
