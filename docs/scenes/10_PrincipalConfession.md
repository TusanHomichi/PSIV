# 10 — `Event_PrincipalConfession`

The party goes back to the principal, who admits what he knows about the
monsters, and pays them 300 meseta. Setting `EventFlag_PrincipalConfession`
here is what opens Piata's gates — it is the flag `RunEvent_ReenterPiata` tests.

| | |
|---|---|
| Event index | `$26` (`EventPtrs[$26]`) |
| Retail range | **`$06D784` .. `$06D7C1`** (62 bytes) |
| Entered from | dialogue `$F6 $0026`, tree 33 entry `$16` |
| Reached via | tree 33 entry `$00` (the principal, map `$14` NPC 0, dlg id `$00`), `$FA` flag `$0B` chain |
| Runs on map | `$14` AcademyPrincipalOffice (dialogue tree **33**) |

## Fork status: commented out to a bare `rts`

`ps4.asm:147145`:

```asm
Event_PrincipalConfession: ; Grand Cross unused
	;move.b	#MusicID_Suspicion, (Sound_Index).l
	;lea	(Field_Obj_Secondary).w, a4
	;... (entire body commented) ...
	;jmp	(EventFlags_Set).l
	rts
```

A different failure mode from the `include` stubs: the routine is present but
**dead**. On the fork, `EventPtrs[$26]` runs a single `rts`, so the confession
never happens, the party is never paid, and `EventFlag_PrincipalConfession` is
never set — which on the fork would also mean `RunEvent_ReenterPiata` bounces
you back into Piata forever.

The commented body matches retail line for line and was used as an independent
cross-check on the disassembly below. Good documentation, dead behaviour.

## Retail disassembly

```
; === Event_PrincipalConfession @ 06D784 .. 06D7C2 (62 bytes) ===
  06D784  13fc009fffff500a move.b #$9F, $FFFF500A.l  ; Sound_Index = MusicID_Suspicion
  06D78C  49f8c300         lea.l  $C300.w, a4        ; map $14 NPC 0 = the principal
  06D790  47f8c000         lea.l  $C000.w, a3        ; Character_1 (party leader)
  06D794  302b0006         move.w $6(a3), d0         ; leader facing_dir
  06D798  08400002         bchg.b #$2, d0            ; flip bit 2 -> the opposite facing
  06D79C  4eb90005a936     jsr    $5A936.l           ; Event_UpdateObjFacing
  06D7A2  7013             moveq  #$13, d0           ; dialogue entry $13
  06D7A4  4eb90005ac66     jsr    $5AC66.l           ; Event_GetAndRunDialogue
  06D7AA  06b8...012cf438  addi.l #300, $F438.w      ; Current_Money += 300
  06D7B2  13fc0084ffff500a move.b #$84, $FFFF500A.l  ; Sound_Index = MusicID_MotabiaTown
  06D7BA  700c             moveq  #$C, d0            ; EventFlag_PrincipalConfession
  06D7BC  4ef900057666     jmp    $57666.l           ; EventFlags_Set  (tail call)
```

## Transcription

```
PlaySound{MusicID_Suspicion}                          ; $9F
Face{who: npc(0), dir: opposite_of(leader.facing_dir)}
RunDialogue{tree: 33, entry: $13}
AddMoney{300}
PlaySound{MusicID_MotabiaTown}                        ; $84
SetFlag{EventFlag_PrincipalConfession}
```

6 ops.

## The facing flip

`move.w facing_dir(a3), d0 / bchg #2, d0` is a neat trick worth naming rather
than open-coding. The facing constants are `0` down, `4` up, `8` right, `$C`
left — bit 2 is the axis-flip bit within each pair:

| Leader faces | `d0` | after `bchg #2` | NPC faces |
|---|---|---|---|
| Down | `$0` | `$4` | Up |
| Up | `$4` | `$0` | Down |
| Right | `$8` | `$C` | Left |
| Left | `$C` | `$8` | Right |

So the principal turns to face the party leader head-on, from whichever side
they approached. `Face{dir: opposite_of(...)}` should be a first-class form in
the interpreter — this is a general "turn to face" and it will recur.

Note it uses the **leader's** facing, and after `Event_AlysFound` the leader is
Alys. It also uses `$FFFFC300`, which the clone calls `Field_Obj_Secondary` here
and `Hahn_Near_Basement` elsewhere; both names mean "NPC slot 0 of the current
map". On map `$14` that is the principal (`NPCType1`, cell (15, 14), dlg id `$00`).

## References

| Kind | Value | Resolved |
|---|---|---|
| Dialogue | tree 33, entry `$13` | *"Wh...what's wrong? …"* (2106 bytes — the longest entry in tree 33) |
| Flag set | `$0C` | `EventFlag_PrincipalConfession` — "Set after talking to the principal revealing the cause of the monsters' outbreak" |
| Flag read | `$0B` | `EventFlag_Igglanova` — by tree 33 entry `$00`'s `$FA` chain |
| Sound | `$9F` | `MusicID_Suspicion` |
| Sound | `$84` | `MusicID_MotabiaTown` — map `$14`'s own music, restored on the way out |
| RAM | `$FFFFC300` | map `$14` NPC index 0, the principal |
| RAM | `$FFFFF438` | `Current_Money` (long) |
| Object field | `$06` | `facing_dir` |

Total opening-act payout: **400 meseta** — 100 from `Event_MeetingHahn`, 300
here.

## Open questions

1. **No `VInt_Prepare` after the music change.** `Event_BasementContainers`
   writes `Sound_Index` and then burns a frame before doing anything else; this
   scene writes `$9F` and immediately does `lea`/`bchg`/`jsr`. Presumably the
   sound driver latches on the next vblank either way. Oracle claim: *`Suspicion`
   starts on the same frame here as it would with an intervening
   `VInt_Prepare`.* If not, the ordering is load-bearing and both scenes need
   frame-exact sound sequencing.
2. **Whether entry `$13` pauses.** At 2106 bytes it is longer than
   `Event_AfterIgglanova`'s `$12`, which pauses twice — but this scene never
   calls `popdlg`, so any pause code inside `$13` would strand it. Presumably
   there is none. Oracle claim: *`Saved_Dialogue_Addr` is written exactly once
   during this scene.*
