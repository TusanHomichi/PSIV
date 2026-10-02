# `Event_ZioFortBarrier`

- **Retail bytes:** `$06EC62..$06EE3F` inclusive, 477 bytes.
- **Pointer:** `EventPtrs[$30]` at `$05A2B4`; event `$0030`.
- **Reach:** Zio Fort `$82`, the courtyard's four `InvisibleBlock` objects at
  `(46..49,55)` each carry dialogue 73; tree 13 entry 73's action runs this
  event. No `RunEvent_*` writer starts it.
- **Data:** `post_rika_events.rs`, `ZIO_FORT_BARRIER` (35 ops).

## Clone audit

Retail body disassembled from the image at `$06EC62`; the clone's
`grand_cross=0` copy at `ps4.asm:148155` was read afterwards and agrees
instruction for instruction, so it is corroboration and not the source. Every
op below cites the address its bytes occupy. The Grand Cross branch of the
same label is not used.

## Retail transcription

| Op(s) | ROM offset | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0 | `$06EC62..$06EC6C` | `moveq #$63,d0` / `EventFlags_Test` / `beq.w $6EE0E` — After Alys Death gates the whole event | `BranchFlag($63)` |
| 1 | `$06EC6E..$06EC74` | dialogue `$44` (`Event_GetAndRunDialogue`) | `RunDialogue($44)` |
| 2-6 | `$06EC76..$06ECD9` | five `Event_GetCharacter` + `move.w #x,$38(a4)` / `move.w #y,$3A(a4)` pairs: Chaz `($2F0,$380)`, Rune `($2F0,$370)`, Gryz `($310,$380)`, Rika `($2E0,$380)`, Demi `($300,$380)` | five `MoveActorTo` (non-blocking) |
| 7 | `$06ECDA` | `FieldObj_Step_Offset = 0` | `SetStepOffset(0)` |
| 8 | `$06ECE0..$06ECEA` | `bset #0` (follow chain off) and `bset #2` (scene owns the camera) on `Char_Move_Flags` | `SetFollowMode(0b101)` |
| 9 | `$06ECEC..$06ED00` | `Event_MoveObjectStatic` on Rune to `($2F0,$370)`: destination plus a spin of *all* field objects, which is how the other four walk their own targets | `MoveActorTo(Rune, wait)` |
| 10-14 | `$06ED02..$06ED08` | `loc_5A97C` with `d0 = 4`: `Event_UpdateObjFacing` on each character object — all five face up | five `Face` |
| 15 | `$06ED0A` | `DoMainUpdatesLoop($27)` | `Wait(40)` |
| 16 | `$06ED34` | `SFXID_Tandle` `$CE` | `PlaySound($CE)` |
| 17 | `$06ED42..$06ED6C` | `Palette_Line_4` (`$FFFFFB60`) ramped from `loc_6EE16` over seven steps, each waiting `DoMapUpdateLoop($1D)`: 3 words per step, `$0CCC→$0000`, `$0000→$0020`, `$0088→$0000` | `PaletteRampFromTable{7, 30}` |
| 18 | `$06ED6E..$06ED74` | `DoMapUpdateLoop($3B)` | `Wait(60)` |
| 19 | `$06ED76..$06ED84` | `clr.w` over eight object slots from `$FFFFC400`: this map's record objects 4 to 11 — `BarrierBeam1..4` and the four `InvisibleBlock` triggers | `DespawnNpc{4, 8}` |
| 20-23 | `$06ED8C..$06EDC0` | face Rune down; `Event_MoveSingleObject` Demi to `($300,$370)`; face her down; `DoMapUpdateLoop($1D)` | face/move/face/`Wait(30)` |
| 24 | `$06EDC2..$06EDC8` | dialogue `$45` | `RunDialogue($45)` |
| 25-28 | `$06EDCA..$06EDEE` | `bclr #0`; `Character_1` — the leader — to `($2F0,$350)`; `bclr #2`; `DoMainUpdatesLoop($3B)` | `SetFollowMode(0b100)`, `MoveActorTo(leader)`, `SetFollowMode(0b000)`, `Wait(60)` |
| 29 | `$06EDF0..$06EDFA` | `Event_MoveCamera` reading the leader's live `$30/$34`, speed 1 | `CameraToActor(leader, 1)` |
| 30-31 | `$06EE00..$06EE08` | `FieldObj_Step_Offset = 1`; `EventFlags_Set($64)` — Zio Fort Barrier | `SetStepOffset(1)`, `SetFlag($64)` |
| 32 | `$06EE06..$06EE08` | `jmp (EventFlags_Set)` restores `d0`, so the event returns the flag id | `Return(1)` |
| 33-34 | `$06EE0E..$06EE14` | `loc_6EE0E`: dialogue `$4A`, then the dialogue routine's return | `RunDialogue($4A)`, `Return(0)` |

### The ring

The five destinations are the cartridge's own, in its own order, and the
leader's final walk is separate — `Character_1` at `$06EDD0`, not a character
id. Cells are the pixel target divided by 16 plus the one-row standing shift
every field walk uses:

| Character | `$38/$3A` destination | standing cell | when |
|---|---|---|---|
| Rune | `($2F0,$370)` | (47,56) | op 9, the blocking move |
| Demi | `($300,$380)`, then `($300,$370)` | (48,56) | op 21, `Event_MoveSingleObject` |
| Chaz | `($2F0,$380)` | (47,57) | op 2 |
| Gryz | `($310,$380)` | (49,57) | op 4 |
| Rika | `($2E0,$380)` | (46,57) | op 5 |
| the leader (`Character_1`) | `($2F0,$350)` | (47,54) | op 26 |

`SetFollowMode` bit 0 off is what makes the ring a ring: with the follow chain
running, each member's own destination is replaced every frame by the cell the
member ahead is vacating, so only the leader reaches his target.

### State edges

Zio Fort Barrier `$64` is written only here, and Zio Fort `$82`'s effect entry
`$42` despawns record objects 4 to 11 on `$64` set — the same eight the scene
clears live, which is why the courtyard's warp 8 at `(47,49)` to Nurvus `$D7`
works both immediately and after any reload.

## Verification

`rust/psiv-runtime/src/suites/next_arc_scenes.rs`,
`the_zio_fort_barrier_walks_its_ring_and_opens_the_courtyard`: starts `$0030`
on Zio Fort `$82` with the campaign's roster and `$63` set, asserts both
dialogues in order, the five ring cells *and* the leader's `(47,54)` when
`$45` opens, flag `$64`, all eight objects cleared, and then walks up into the
courtyard to arrive on Nurvus `$D7` at `(32,35)`.
