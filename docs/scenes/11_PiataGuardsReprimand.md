# 11 — `Event_PiataGuardsReprimand`

Try to leave Piata before the basement mission is done and the guards throw you
back in. This is the **act boundary** — the scene that stops existing once
`EventFlag_PrincipalConfession` is set.

| | |
|---|---|
| Event index | `$9E` (`EventPtrs[$9E]`) |
| Retail range | **`$0738D2` .. `$073945`** (116 bytes) |
| Entered from | trigger `$5B` `RunEvent_ReenterPiata`, map `$00` **Motavia overworld** |
| Condition | `EventFlag_PrincipalConfession ($0C)` **clear** |
| Runs on | the overworld, then warps to map `$10` Piata |

## Fork status: rewritten in place

The clone's version is fork-edited and says so in its own comment — *"Edited for
Grand Cross; now reuses piata's map dialog tree"* — replacing the whole tree
swap with `moveq #2, d0 / jsr Event_GetAndRunDialogue`.

**Retail does not do that.** Retail loads `DialogueTree43` from `$1FD7A0` into
the dialogue buffer, runs entry `$10`, and then reloads `$1DF600`. Transcribing
from the clone here gives you the wrong line from the wrong tree.

## Retail disassembly

```
; === Event_PiataGuardsReprimand @ 0738D2 .. 073946 (116 bytes) ===
  0738D2  13fc00fbffff500a move.b #$FB, $FFFF500A.l  ; Sound_Index = Sound_StopMusic
  0738DA  4eb90004223e     jsr    $4223E.l           ; PalFadeOut_ClrSpriteTbl
; --- warp into Piata ---
  0738E0  31fc0010ec28     move.w #$10, $EC28.w      ; Field_Map_Index = MapID_Piata
  0738E6  31fc0000ec2a     move.w #$0,  $EC2A.w      ; Field_Map_Index_2 = $00 (from Motavia)
  0738EC  31fc003eec48     move.w #$3E, $EC48.w      ; Map_Start_X_Pos ($3E * 8 = $1F0 px)
  0738F2  31fc0056ec4a     move.w #$56, $EC4A.w      ; Map_Start_Y_Pos ($56 * 8 = $2B0 px)
  0738F8  31fc0000ec44     move.w #$0,  $EC44.w      ; Map_Start_Facing_Dir = DOWN
  0738FE  31fc0008ec46     move.w #$8,  $EC46.w      ; Map_Start_Char_Align = 8
  073904  08b80003ec4e     bclr.b #$3,  $EC4E.w      ; Map_Load_Flags bit 3 clear
  07390A  4eb90005ae98     jsr    $5AE98.l           ; RefreshMap
  073910  13fc0084ffff500a move.b #$84, $FFFF500A.l  ; Sound_Index = MusicID_MotabiaTown
  073918  11fc0084ecec     move.b #$84, $ECEC.w      ; Saved_Sound_Index = MusicID_MotabiaTown
  07391E  4eb9000421d4     jsr    $421D4.l           ; Pal_FadeIn
; --- swap in tree 43, say the line, swap tree 1 back ---
  073924  203c001fd7a0     move.l #$1FD7A0, d0       ; DialogueTree43
  07392A  4eb900053f00     jsr    $53F00.l           ; DialogueTreesToRAM -> $FFFF3000
  073930  7010             moveq  #$10, d0           ; entry $10
  073932  4eb90005ac66     jsr    $5AC66.l           ; Event_GetAndRunDialogue
  073938  203c001df600     move.l #$1DF600, d0       ; DialogueTree1
  07393E  4eb900053f00     jsr    $53F00.l           ; DialogueTreesToRAM
  073944  4e75             rts
```

## Transcription

```
PlaySound{Sound_StopMusic}                  ; $FB
FadeOut{}
LoadMap{map: $10 Piata, prev_map: $00,
        start_x: $3E, start_y: $56,         ; 8-pixel units -> ($1F0, $2B0) px
        facing: Down, align: 8,
        load_flags: clear bit 3}
PlaySound{MusicID_MotabiaTown}              ; $84
SetSavedMusic{MusicID_MotabiaTown}
FadeIn{}
SetDialogueTree{DialogueTree43 @ $1FD7A0}
RunDialogue{tree: 43, entry: $10}
SetDialogueTree{DialogueTree1 @ $1DF600}
Return{}
```

10 ops.

## The tree it restores is the wrong one — flag, don't fix

The scene finishes by loading **`DialogueTree1`** (`$1DF600`) back into the
dialogue buffer. But it is standing on **map `$10` Piata, which binds tree 2**
(`$1E0190`). So on return, every NPC in Piata town is being read out of tree 1's
entry list until something reloads the map.

Two readings, and this pass did not settle which:

1. **Original bug.** The author reached for "the default tree" and got tree 1
   rather than the current map's tree. Every Piata townsperson would talk
   nonsense until the next map load.
2. **Harmless because a reload follows.** This is a plain event (not a
   cutscene), so it returns through `loc_5A27A`, which does *not* have
   `FieldRoutine_Cutscene`'s "reload the map if `d0 == 0`" path. But the scene
   already called `RefreshMap` and changed `Field_Map_Index`, so a map load may
   be in flight and about to re-bind the tree correctly anyway.

Under the fidelity policy this is **not** an "obvious bug" to fix on sight —
it is only obvious if reading 1 is right, and the difference is directly
observable. Oracle claim, stated so it can be settled in one run:

> Ryuka onto the Motavia overworld with `EventFlag_PrincipalConfession` clear.
> After the guards' line, without changing maps, talk to any Piata townsperson.
> If the text is that NPC's normal line, `$FFFF3000` was re-bound to tree 2 by a
> map load and reading 2 holds. If the text is a *different* NPC's line, reading
> 1 holds and this is a retail bug to reproduce.

Filed in the final report either way.

## References

| Kind | Value | Resolved |
|---|---|---|
| Dialogue | **tree 43**, entry `$10` | *"For crying out loud, / I'm telling you, / you can't go outside!"* |
| Flag read | `$0C` | `EventFlag_PrincipalConfession` — by the trigger, not by this routine |
| Flag set | *(none)* | this scene sets no flag; it is pure gating |
| Sound | `$FB` | `Sound_StopMusic` |
| Sound | `$84` | `MusicID_MotabiaTown` |
| ROM | `$1FD7A0` | `DialogueTree43` |
| ROM | `$1DF600` | `DialogueTree1` |
| RAM | `$FFFFEC28` / `$FFFFEC2A` | `Field_Map_Index` / `Field_Map_Index_2` |
| RAM | `$FFFFEC48` / `$FFFFEC4A` | `Map_Start_X_Pos` / `Map_Start_Y_Pos` — **8-pixel units** |
| RAM | `$FFFFEC44` / `$FFFFEC46` | `Map_Start_Facing_Dir` / `Map_Start_Char_Align` |
| RAM | `$FFFFEC4E` | `Map_Load_Flags` |
| RAM | `$FFFFECEC` | `Saved_Sound_Index` |
| Routine | `$53F00` | `DialogueTreesToRAM` — `a0 = d0`, `a1 = $FFFF3000`, Kosinski-decompress |
| Routine | `$5AE98` | `RefreshMap` — clears `$400` longs at `Character_1` and `$100` at `Tile_Anim_Memory`, gated on `Map_Load_Flags` bit 3 |

Landing spot in Piata: `Map_Start = ($3E, $56)` -> `curr = ($1F0, $2B0)` pixels
= cell (31, 43), facing down. That is inside the town, just past the gate.

## Open questions

1. **The tree-restore question above.** Highest-value oracle item in the act.
2. **`Map_Start_Char_Align = 8`.** Every other `LoadMap` in the act uses `0` or
   `$C`; this one uses `8`. The value is consumed by `loc_535D4` alongside
   facing when placing party members. Not decoded. It presumably controls how
   followers stack behind the leader on arrival.
3. **Whether the trigger can fire mid-step on the overworld.** `RunEvents`
   returns early while either step duration is non-zero, so it should be
   landing-evaluated. Oracle claim: *the warp fires on the first landing on the
   overworld, not partway through the step off Piata's exit.*
