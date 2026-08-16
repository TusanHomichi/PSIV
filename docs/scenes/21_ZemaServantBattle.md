# `Event_ZemaServantBattle`

- **Retail bytes:** `$07311E..$07313D` inclusive, 32 bytes.
- **Pointer:** `EventPtrs[$8A]` from `$05A2B4`.
- **Trigger:** Zema `$24`, trigger `$74`, Silver Soldier `$B1` set and servants
  `$B2` clear.
- **Data:** `next_arc.rs`, `ZEMA_SERVANT_BATTLE_OPS`.

## Clone audit

This is an unconditional body in the clone's retail stream. The retail bytes
and clone source agree across all 32 bytes; no Grand Cross scene include is
allowed to replace it.

## Retail transcription

1. Set `EventFlag_Servants` (`$B2`).
2. Set `Map_Load_Flags` bit 3.
3. Write `Event_Battle_Index = $14`.
4. Set the routine-exit bit and return `d0 = 1`.

`StartBattle { index: 0x14 }` is the native equivalent of the battle hand-off;
the runtime owns formation lookup and returns `BattleFinished` to the blocked
scene.

## Verification

The runtime test enables the existing battle pack, observes
`SceneBattleStarted`, resolves the event battle, and asserts `$B2` without
adding a second battle implementation.
