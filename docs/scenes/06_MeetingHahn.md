# 06 — `Event_MeetingHahn`

Hahn Mahlay hires himself onto the party outside the basement. He walks to line
up with the party leader, faces down, the hiring scene plays, and he joins in
**slot 3** — with 100 meseta up front.

| | |
|---|---|
| Event index | `$04` (`EventPtrs[$04]`) |
| Retail range | **`$06B3BA` .. `$06B4AF`** (246 bytes) |
| Entered from | dialogue `$F6 $0004`, tree 33 entry `$14` |
| Reached via | tree 33 entry `$10` (Hahn, map `$12` NPC 0, dlg id `$10`), `$FA` flag `$09` chain |
| Runs on map | `$12` PiataAcademyNearBasement (dialogue tree **33**) |

## Fork status: clean

`ps4.asm:144617` carries the full body, no include, no guard. Aligned against
retail instruction for instruction and operand for operand — **byte-exact
match, zero divergences**. This scene was used to bootstrap the primitive
symbol map (`Event_UpdateObjFacing`, `Event_GetAndRunDialogue`, `Event_AddMacro`,
`EventFlags_Set`, `Field_LoadSprites`, `Field_BuildSprites`, `VInt_Prepare`),
precisely because it is uncontaminated.

## Retail disassembly

```
; === Event_MeetingHahn @ 06B3BA .. 06B4B0 (246 bytes) ===
; --- let any in-progress motion finish ---
Event_MeetingHahn:                                  ; loop target is the routine head
  06B3BA  49f8c300       lea.l  $C300.w, a4         ; Hahn_Near_Basement (map $12 NPC 0)
  06B3BE  4eb90004690e   jsr    $4690E.l            ; FieldObj_NPCHahnNearBasement
  06B3C4  4eb900044954   jsr    $44954.l            ; Field_LoadSprites
  06B3CA  4eb9000447fe   jsr    $447FE.l            ; Field_BuildSprites
  06B3D0  4eb90004204c   jsr    $4204C.l            ; VInt_Prepare
  06B3D6  49f8c300       lea.l  $C300.w, a4
  06B3DA  302c0028       move.w $28(a4), d0         ; x_step_duration
  06B3DE  806c002a       or.w   $2A(a4), d0         ; | y_step_duration
  06B3E2  66d6           bne.b  $6B3BA
; --- walk Hahn horizontally onto the leader's column ---
  06B3E4  49f8c300       lea.l  $C300.w, a4
  06B3E8  47f8c000       lea.l  $C000.w, a3         ; Character_1 (party leader = Alys)
  06B3EC  302b0030       move.w $30(a3), d0         ; leader curr_x_pos
  06B3F0  b06c0030       cmp.w  $30(a4), d0         ; vs Hahn curr_x_pos
  06B3F4  67000034       beq.w  $6B42A              ; aligned -> skip
  06B3F8  6204           bhi.b  $6B3FE
  06B3FA  7008           moveq  #$8, d0             ; movement command 8
  06B3FC  6002           bra.b  $6B400
  06B3FE  7004           moveq  #$4, d0             ; movement command 4
loc_6B400:                                          ; spin until the step completes
  06B400  49f8c300       lea.l  $C300.w, a4
  06B404  4eb90004690e   jsr    $4690E.l
  06B40A  4eb900044954   jsr    $44954.l
  06B410  4eb9000447fe   jsr    $447FE.l
  06B416  4eb90004204c   jsr    $4204C.l
  06B41C  49f8c300       lea.l  $C300.w, a4
  06B420  302c0028       move.w $28(a4), d0
  06B424  806c002a       or.w   $2A(a4), d0
  06B428  66d6           bne.b  $6B400
; --- face down, talk ---
loc_6B42A:
  06B42A  49f8c300       lea.l  $C300.w, a4
  06B42E  7000           moveq  #$0, d0             ; FacingDir_Down
  06B430  4eb90005a936   jsr    $5A936.l            ; Event_UpdateObjFacing
  06B436  7011           moveq  #$11, d0            ; dialogue entry $11
  06B438  4eb90005ac66   jsr    $5AC66.l            ; Event_GetAndRunDialogue
; --- join in slot 3 ---
  06B43E  11fc0002f40c   move.b #$2, $F40C.w        ; Current_Party_Slot_3 = CharID_Hahn
  06B444  49f8c080       lea.l  $C080.w, a4         ; Character_3
  06B448  47f8c300       lea.l  $C300.w, a3         ; NPC Hahn (source of position)
  06B44C  38bc000c       move.w #$C, (a4)           ; obj id $C = FieldObj_Hahn
  06B450  397c00000006   move.w #$0, $6(a4)         ; facing_dir = DOWN
  06B456  397c05440016   move.w #$544, $16(a4)      ; art_tile
  06B45C  396b00300030   move.w $30(a3), $30(a4)    ; curr_x_pos
  06B462  396b00340034   move.w $34(a3), $34(a4)    ; curr_y_pos
  06B468  08f80001ecfe   bset.b #$1, $ECFE.w        ; Char_Move_Flags bit 1
  06B46E  4eb900046884   jsr    $46884.l            ; FieldObj_Hahn (init)
  06B474  7002           moveq  #$2, d0             ; CharID_Hahn
  06B476  4eb900063b82   jsr    $63B82.l            ; Event_AddMacro
  06B47C  41f8c300       lea.l  $C300.w, a0         ; clear the NPC slot
  06B480  3e3c002f       move.w #$2F, d7
  06B484  4e40           trap   #$0                 ; clear 48 longs = 192 bytes
; --- payment and palette ---
  06B486  06b8...0064f438 addi.l #100, $F438.w      ; Current_Money += 100
  06B48E  2078ecd2       movea.l $ECD2.w, a0        ; Map_Palettes_Addr
  06B492  2050           movea.l (a0), a0
  06B494  43f8fb00       lea.l  $FB00.w, a1         ; Palette_Table_Buffer
  06B498  3e3c001f       move.w #$1F, d7
  06B49C  4e41           trap   #$1                 ; copy 32 words
  06B49E  43e90020       lea.l  $20(a1), a1
  06B4A2  3e3c000f       move.w #$F, d7
  06B4A6  4e41           trap   #$1                 ; copy 16 more words
  06B4A8  700a           moveq  #$A, d0             ; EventFlag_HahnJoined
  06B4AA  4ef900057666   jmp    $57666.l            ; EventFlags_Set
```

## Transcription

```
MoveActor{who: NPCHahnNearBasement, cmd: none, wait: until_idle}   ; settle
BranchIf{ Hahn.curr_x_pos == leader.curr_x_pos -> skip 2 }
BranchIf{ leader.curr_x_pos > Hahn.curr_x_pos -> cmd 4 else cmd 8 }
MoveActor{who: NPCHahnNearBasement, cmd: <4 or 8>, wait: until_idle}
Face{who: NPCHahnNearBasement, dir: Down}
RunDialogue{tree: 33, entry: $11}
SetPartySlot{slot: 3, char: Hahn}                  ; byte write to $F40C
PromoteNpcToChar{npc: NPCHahnNearBasement, char_id: Hahn, slot: 3,
                 obj_id: $C, art_tile: $544, facing: Down,
                 position: from NPCHahnNearBasement}
SetFollowMode{set bit 1}                           ; update movement order
JoinParty{char: Hahn}                              ; Event_AddMacro
DespawnNpc{who: NPCHahnNearBasement}               ; trap #0, see the note
AddMoney{100}
ReloadMapPalette{}                                 ; Map_Palettes_Addr -> Palette_Table_Buffer
SetFlag{EventFlag_HahnJoined}
```

14 ops.

## The despawn clears three object slots, not one

`lea (Hahn_Near_Basement).w, a0 / move.w #$2F, d7 / trap #0` clears **48
longwords = 192 bytes** starting at `$FFFFC300`. One field-object struct is
`$40` bytes, so this wipes NPC slots **0, 1 and 2** of map `$12`.

That is not obviously a bug: map `$12`'s NPC list is exactly

| Index | Symbol | Cell |
|---|---|---|
| 0 | `NPCHahnNearBasement` | (13, 17) |
| 1 | `InvisibleBlock` | (13, 17) |
| 2 | `InvisibleBlock` | (12, 17) |

— Hahn plus the two invisible collision blocks that fence him in. Clearing all
three at once removes the blockers along with the NPC, which is exactly what
you want once he becomes a party member. Compare `Event_AlysFound`, which
clears `#$F` (one object) because NPC-Alys has no attached blockers.

**Reproduce the width, don't "fix" it.** An interpreter that models
`DespawnNpc` as "clear one object" will leave two invisible walls across the
basement corridor.

## References

| Kind | Value | Resolved |
|---|---|---|
| Dialogue | tree 33, entry `$11` | *"What? You're the hunters? / Really? / You don't think we can do the job? …"* through *"…OK, I'll pay...."* (1537 bytes, portrait `$F4 $03`) |
| Flag set | `$0A` | `EventFlag_HahnJoined` |
| Flag read | `$09` | `EventFlag_PrincipalMeeting` — by tree 33 entry `$10`'s `$FA` chain |
| RAM | `$FFFFC300` | `Hahn_Near_Basement` = map `$12` NPC index 0; the clone also calls this address `Field_Obj_Secondary` |
| RAM | `$FFFFC080` | `Character_3` |
| RAM | `$FFFFF40C` | `Current_Party_Slot_3` (byte) |
| RAM | `$FFFFF438` | `Current_Money` (long) |
| RAM | `$FFFFECD2` | `Map_Palettes_Addr` |
| RAM | `$FFFFFB00` | `Palette_Table_Buffer` |
| RAM | `$FFFFECFE` | `Char_Move_Flags` — bit 1 = update movement order (X first then Y, or vice versa) |
| Object id | `$C` | `FieldObj_Hahn` |
| Object field | `$16` | `art_tile` = `$544` |

**Dialogue-tree caveat**: the entry id `$11` is a bare literal, and
`Event_MeetingHahn` does **not** reload the dialogue tree. It therefore reads
whatever the current map bound — map `$12` binds **tree 33**. Tree 1 also has an
entry `$11` ("Hahn! What are you doing in the company of those uncivilized
animals?"), which is an unrelated academy NPC line and **not** this scene.
Getting the tree wrong here silently plays the wrong 154 bytes.

## Open questions

1. **What movement commands 4 and 8 mean for this NPC.** Hahn's behaviour class
   differs from Alys's, so the `$4A266` table read that decoded Alys's command 2
   does not apply. The clone's comments claim 8 = left and 4 = right; the branch
   is `cmp.w leader_x, hahn_x` then `bhi` -> 4, so 4 is taken when Hahn is to the
   **left** of the leader, which would make 4 = *move right*. The clone's
   comments are on the opposite convention and at least one of them is wrong.
   Decode Hahn's table before trusting either. Oracle claim: *approaching Hahn
   from his right, he walks left; from his left, he walks right; in both cases
   he ends on the leader's exact column.*
2. **The walk is one step, like Alys's.** The command is issued once. If the
   leader can stand more than one cell off Hahn's column, retail leaves him
   misaligned. Map `$12`'s geometry was not checked. Oracle claim: *there is no
   reachable tile from which the post-scene Hahn's `curr_x_pos` differs from the
   leader's.* If false, that is an original quirk to reproduce.
3. **The two-part palette copy.** `$1F` words then `$F` more, into
   `Palette_Table_Buffer` and `+$20`, from the map's palette pointer — 48 words,
   the three palette lines. Why the scene reloads the palette on a party join is
   unclear; likely because `FieldObj_Hahn` needs its sprite line. Harmless to
   reproduce as "reload the map palette".
