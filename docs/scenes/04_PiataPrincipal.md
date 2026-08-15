# 04 — `Cutscene_PiataPrincipal`

The principal of Motavia Academy briefs Alys and Chaz on the monsters in the
basement and hires them. This is a **cutscene**, not a plain event: it is
reached through `CutscenePtrs`, and its return value controls whether the map
gets reloaded afterwards.

| | |
|---|---|
| Cutscene index | `$01` (`CutscenePtrs[$01]`), so `Event_Index = $8001` |
| Retail range | **`$073EE0` .. `$073F21`** (66 bytes) |
| Entered from | dialogue `$F6 $8001`, tree 33 entry `$15` |
| Reached via | tree 33 entry `$00` (the principal, map `$14` NPC 0, dlg id `$00`), `$FA` flag `$08` chain |
| Runs on map | `$14` AcademyPrincipalOffice (dialogue tree **33**) |

## Fork status: clean

`ps4.asm:153464` carries the full body with no include and no `if grand_cross`
guard, and it matches retail. The clone does carry a stale comment —
`; TODO: overriden by event code, remove` — which is wrong for retail: the
retail `CutscenePtrs[$01]` really does point here.

## Retail disassembly

```
; === Cutscene_PiataPrincipal @ 073EE0 .. 073F22 (66 bytes) ===
  073EE0  13fc00fbffff500a move.b #$FB, $FFFF500A.l   ; Sound_Index = Sound_StopMusic
  073EE8  4eb90005a658     jsr    $5A658.l            ; InitVRAMAndCRAM
  073EEE  13fc009fffff500a move.b #$9F, $FFFF500A.l   ; Sound_Index = MusicID_Suspicion
  073EF6  4eb9000421d4     jsr    $421D4.l            ; Pal_FadeIn
  073EFC  7017             moveq  #$17, d0            ; dialogue entry $17
  073EFE  11fc0001ecfd     move.b #$1, $ECFD.w        ; Render_Sprites_In_Cutscenes = 1
  073F04  4eb90005adf8     jsr    $5ADF8.l            ; Event_GetAndRunDialogue5
  073F0A  7009             moveq  #$9, d0             ; EventFlag_PrincipalMeeting
  073F0C  4eb900057666     jsr    $57666.l            ; EventFlags_Set
  073F12  13fc00fbffff500a move.b #$FB, $FFFF500A.l   ; Sound_Index = Sound_StopMusic
  073F1A  4238ecec         clr.b  $ECEC.w             ; Saved_Sound_Index = 0
  073F1E  7000             moveq  #$0, d0             ; return 0 -> caller reloads the map
  073F20  4e75             rts
```

## The return value matters

`FieldRoutine_Cutscene` does:

```asm
jsr     (a0)
bne.s   .skipmapreload
bset    #2, (Map_Load_Flags).w      ; reload objects
move.w  #8, (Game_Mode_Index).w     ; GameMode_LoadFieldMap
.skipmapreload:
```

This scene returns **`d0 = 0`**, so the map *is* reloaded with the reload-objects
bit set. That is how the principal's office comes back with the post-briefing
NPC state, and it is why the scene can afford to blow away VRAM/CRAM with
`InitVRAMAndCRAM`. Any cutscene transcription must carry the return value; it
is a real part of the contract, not a C convention.

## Transcription

```
PlaySound{Sound_StopMusic}                  ; $FB
InitVRAMAndCRAM{}
PlaySound{MusicID_Suspicion}                ; $9F
FadeIn{}
SetRenderSpritesInCutscene{true}
RunDialogue{tree: 33, entry: $17}           ; via Event_GetAndRunDialogue5
SetFlag{EventFlag_PrincipalMeeting}
PlaySound{Sound_StopMusic}                  ; $FB
SetSavedMusic{0}
ReturnFromCutscene{reload_map: true}        ; d0 = 0
```

10 ops.

`InitVRAMAndCRAM` (`$5A658`) is not in the base vocabulary and is genuinely
presentation: it fades the palette out, sets VDP register `$8B00`, clears the
`$800`-word Plane A buffer and re-uploads. Model it as
`ClearPlaneA{} + FadeOut{}` rather than as an engine op, or leave it to the
renderer entirely — the engine-visible consequence is only the fade.

`Event_GetAndRunDialogue5` (`$5ADF8`) is a different dialogue entry point from
the `$5AC66` the other scenes use. Both start with
`jsr GetDialogueByID` and then a `RunDialogue*` variant; variant 5 (`$5ADFE`)
differs from variant 1 (`$5AC6C`) in its window setup, which matters for the
renderer but not for which text plays.

## References

| Kind | Value | Resolved |
|---|---|---|
| Dialogue | tree 33, entry `$17` | *"Welcome. / I'm the principal of this academy. …"* through *"…Don't forget, I'm paying you dearly for your services!"* (982 bytes) |
| Flag set | `$09` | `EventFlag_PrincipalMeeting` — "Set after talking to the principal the first time" |
| Flag read | `$08` | `EventFlag_AlysFound` — read by tree 33 entry `$00`'s `$FA` chain, not by this routine |
| Sound | `$FB` | `Sound_StopMusic` |
| Sound | `$9F` | `MusicID_Suspicion` |
| RAM | `$FFFF500A` | `Sound_Index` (long-addressed, as everywhere in the event code) |
| RAM | `$FFFFECEC` | `Saved_Sound_Index` |
| RAM | `$FFFFECFD` | `Render_Sprites_In_Cutscenes` |

## Open questions

1. **`Saved_Sound_Index` semantics.** This scene clears it; `Event_GameStart`
   and `Event_PiataGuardsReprimand` set it to `$84`. It looks like "the music to
   restore after a battle or menu", but that was not chased. Oracle claim:
   *after this cutscene, walking into a random encounter and winning leaves the
   field silent rather than resuming `MusicID_Suspicion`.*
2. **Whether the map reload is observable.** `bset #2, Map_Load_Flags` +
   `GameMode_LoadFieldMap` should re-place the party from `Map_Start_*`, which
   this scene never wrote. Oracle claim: *the party's `curr_x_pos`/`curr_y_pos`
   are unchanged across the reload* — if false, the reload path preserves
   position some other way and psiv-core must reproduce it.
