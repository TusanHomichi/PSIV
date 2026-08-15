# 08 — `Event_IgglanovaBattle`

The Igglanova fight in the academy basement. This scene has no staging at all —
it is a guard, a flag, and a hand-off into the battle system.

| | |
|---|---|
| Event index | `$6B` (`EventPtrs[$6B]`) |
| Retail range | **`$0722D2` .. `$0722F5`** (36 bytes) |
| Entered from | **interaction area**, not the trigger table |
| Reached via | map `$17` area at tile (30, 18), range `XPlus20_YPlus10`, `interaction_type = 2`, parameter `$0B` -> `Interaction_EventIndexes[$0B] = $6B` |
| Runs on map | `$17` AcademyBasement_B2 |

## Fork status: clean

`ps4.asm:152018` carries the body with no include and no guard.

## Retail disassembly

```
; === Event_IgglanovaBattle @ 0722D2 .. 0722F6 (36 bytes) ===
  0722D2  700b           moveq  #$B, d0             ; EventFlag_Igglanova
  0722D4  4eb900057624   jsr    $57624.l            ; EventFlags_Test
  0722DA  6702           beq.b  $722DE              ; not set -> fight
  0722DC  4e75           rts                        ; already fought -> return (d0 != 0)
loc_722DE:
  0722DE  700b           moveq  #$B, d0
  0722E0  4eb900057666   jsr    $57666.l            ; EventFlags_Set
  0722E6  11fc0000ecfc   move.b #$0, $ECFC.w        ; Event_Battle_Index = 0
  0722EC  08f80003ec27   bset.b #$3, $EC27.w        ; Routine_Exit_Flags bit 3
  0722F2  7001           moveq  #$1, d0             ; return 1
  0722F4  4e75           rts
```

## Transcription

```
BranchFlag{ if EventFlag_Igglanova set -> Return{d0: nonzero} }
SetFlag{EventFlag_Igglanova}
StartBattle{event_battle_index: 0}
Return{d0: 1}
```

4 ops. The only `BranchFlag` in the opening act that is a genuine flag branch
inside a scene rather than in a trigger.

## How the battle actually starts

`bset #3, (Routine_Exit_Flags).w` is the mechanism. Both dispatchers check it on
the way out:

```asm
bclr    #3, (Routine_Exit_Flags).w
bne.w   RunEventBattle
```

`RunEventBattle` then stops all sound, picks music from `EventBattleMusicData`
indexed by `Event_Battle_Index`, sets `Battle_Type = 1` (cleared instead if the
index is `$1A`), and jumps to `FieldRoutine_Battle2`.

So `StartBattle` is not "call the battle system" — it is "set a flag that the
event dispatcher's epilogue turns into a battle." The scene *returns normally*
first. An interpreter that models it as a direct call will run the battle before
the scene's own return value is consumed, which matters because that return
value is also what suppresses the map reload.

**`Event_Battle_Index = 0`** selects `EventBattleMusicData[0]` =
`MusicID_DefeatAtABlow`.

## The flag is set *before* the fight

`EventFlag_Igglanova ($0B)` is set on the way in, not on victory. Consequences,
all of which the interpreter must reproduce:

- Losing or fleeing the Igglanova fight does not let you re-trigger it by
  stepping on the area again — the guard at the top returns immediately.
- `RunEvent_AfterIgglanova` requires `$0B` set and `$0F` clear, so it becomes
  live the moment the battle is *entered*.
- Tree 33 entry `$00`'s `$FA` chain routes the principal to
  `Event_PrincipalConfession` on flag `$0B`, so the confession is reachable from
  the moment the fight starts too.

Whether any of that is observable depends on whether the fight is escapable,
which is a battle-lane question.

## References

| Kind | Value | Resolved |
|---|---|---|
| Flag set | `$0B` | `EventFlag_Igglanova` — "Set when you fight the Igglanova in the Piata basement" |
| Flag read | `$0B` | same, as the re-entry guard |
| RAM | `$FFFFECFC` | `Event_Battle_Index` (byte) |
| RAM | `$FFFFEC27` | `Routine_Exit_Flags` — bit 3 = "run an event battle on exit" |
| Table | `$5A63E` | `EventBattleMusicData`; index 0 = `MusicID_DefeatAtABlow` |
| Map object | map `$17` NPC 0 | `Igglanova` at cell (15, 10); NPCs 1 and 2 are `InvisibleBlock` at (16, 10) and (15, 10) |
| Dialogue | *(none)* | this scene shows no text |

## Open questions

1. **Where the on-screen Igglanova sprite goes after the win.** The scene never
   despawns map `$17` NPC 0, and nothing in `Event_AfterIgglanova` clears it
   either. Presumably a flag-gated `MapDataManager` entry or an NPC-despawn
   extraction removes it on the next map build. Oracle claim: *after the battle,
   with `EventFlag_Igglanova` set, map `$17`'s NPC 0 is absent from the rebuilt
   object list.* Worth confirming with the maps lane — if it is not flag-gated,
   the boss stands there forever and something else removes it.
2. **Battle-type semantics.** `Battle_Type = 1` for every event battle except
   index `$1A`. Not chased; battle lane's problem.
