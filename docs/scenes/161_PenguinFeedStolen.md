# The Pengu Feed thief: `Event_PenguinFeedStolen`

Zosa's item shop (`ZosaItemShop` `$12A`, tree 17). Walking onto the row above the door with a
Pengu Feed in the bag starts it. The music changes, a thief runs in under the leader, spins, runs
off to the right, and every Pengu Feed `GetItem` can find is taken.

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0096` `Event_PenguinFeedStolen` | `$0735F6..$0736E9` inclusive, 244 bytes | `EventPtrs[$96]` at `$05A2B4 + $258` | `zosa_events.rs`, `PENGUIN_FEED_STOLEN` (58 ops) |

- **Trigger:** map trigger `$7B`, `RunEvent_PenguFeedStolen` (`ps4.asm:116572`, `$0575DC` in the
  US image). It calls `GetItem` for `ItemID_PenguFeed` (`$92`), restores registers with a
  `movem.l` that leaves the flags alone (`$0575EA`), and `bne.w` to `RunEvent_NoEvent` at
  `$0575EE`. `GetItem` returns zero with Z set when it finds the item (`ps4.asm:121975`), so the
  trigger fires only while the Feed is in the bag, and then only with the leader's `curr_y_pos` at
  `$260`. The port had this the wrong way round, firing when the Feed was missing;
  `trigger_custom.rs` now reads it as the cartridge does, and its two tests say so.
- **Flags:** none. The scene takes the item; nothing records that it ran, so it runs again
  whenever the party walks back onto the row with another Feed.

## Clone audit

The body was read from the US image through its `EventPtrs` slot, and every instruction word
matches `ps4.asm:153259-153328`. It has no `revision` or `grand_cross` conditional and loads no
dialogue tree; entry `$10` is the shop's own tree 17. The thief's routine is
`FieldObjectsJmpTbl` `$368`, `loc_4B30C` (`ps4.asm:98895`): its init clears `render_flags` bit 3,
so the thief neither blocks nor answers the talk probe, and copies its position into `dest`, so
the creation frame does not move it. After that it walks to `dest` through `FieldObj_GetInput`
and `FieldObj_Move`.

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0-2 | `$0735F6..$07360D` | `Sound_StopMusic`, `DoMapUpdateLoop($10)`, `MusicID_DezorisDeDon` (`$B3`) | `PlaySound`, `Wait(17)`, `PlaySound` |
| 3-5 | `$07360E..$07363F` | `lea ($FFFFC380).w, a4`: routine `$368`, tile `$342`, (`$150`,`$270`); `RunSingleObject`, `Field_LoadSprites`, `Field_BuildSprites`, `VInt_Prepare` | `Object(ObjectOp::Spawn)` slot 2, `RebuildSprites`, `WaitFrames(1)` |
| 6 | `$073640..$073655` | `Event_MoveSingleObject` to (`Character_1`'s `$30`, `$270`) | `MoveActorToActorAxis` X, waits |
| 7-46 | `$073656..$07368B` | `loc_7365E`, `d7 = $13`: facing `d1 * 4`, two `VInt_Prepare`, `d1` wraps at 4 | `Face`, `WaitFrames(2)` x 20: down, up, right, left, five times |
| 47-48 | `$07368C..$07369B` | facing 0, `DoMainUpdatesLoop($1D)` | `Face`, `Wait(30)` |
| 49-50 | `$07369C..$0736AF` | `Event_MoveSingleObject` to (`$2D0`,`$270`), `clr.w (a4)` | `MoveActorTo`, waits; `DespawnNpc` slot 2 |
| 51 | `$0736B0..$0736B9` | entry `$10` | `RunDialogue` |
| 52-54 | `$0736BA..$0736D1` | `Sound_StopMusic`, `DoMapUpdateLoop($10)`, `MusicID_NowOnSale` (`$87`) | `PlaySound`, `Wait(17)`, `PlaySound` |
| 55-57 | `$0736D2..$0736E9` | `loc_736D2`: `GetItem` `$92`, `bne.s loc_736E8`, `clr.b (a0)`, `ReorderInventory`, `bra.s loc_736D2`; `rts` | `Item(ItemOp::Take)`, `Jump`, `End` |

**The temporary object.** `ZosaItemShop`'s record places two objects, the fire (slot 0) and the
shopkeeper (slot 1). The thief is written straight into slot 2 (`$FFFFC380`). `ObjectOp::Spawn`
does the same: the slot joins the scene's cast as `ActorRef::Npc(2)`, so `MoveActorToActorAxis`,
`Face` and `MoveActorTo` drive it as they drive a map's own objects. The runtime places it on the
field map at index 2 (`FieldMap::place_object`), filling any slots between with absent objects
so no index moves. The `clr.w (a4)` after the second run is `DespawnNpc`, and the next map load
drops the slot, as `LoadMapObjects` rebuilds the object area. A spawn into a slot the cast
already holds moves that object and resets it rather than adding a second one.

**The take loop.** `ItemOp::Take` is the missing student's op (`$78`,
[160](160_MissingStudent.md)). Here it loops: each pass takes one Feed and closes the bag up,
until `GetItem` finds none. A Feed behind a battle's hole stays, because `GetItem` counts the
occupied slots and searches only that many.

**What is derived.** The thief's walk uses the scene cast's step rate, eight frames a cell, which
is not measured against `loc_4B30C`'s own step, and its art (tile `$342`) has no sprite sheet in
the pack. Nothing draws it yet, as with the strip club's dancers (#241).

## Evidence

- State: `rust/psiv-core/tests/penguin_feed_scene.rs`: the spawn into slot 2 with the thief not
  solid, the runs to the leader's X and to `$2D0`, the clear before entry `$10`, the twenty
  facings, the bag emptied of every findable Feed, a Feed behind a hole staying, and a second
  spawn reusing its slot.
- `rust/psiv-core/src/trigger_custom.rs`: the trigger fires with the Feed carried at `y == $260`
  and not one pixel off, and stays quiet with the Feed out of `GetItem`'s reach.
- Pad-driven in the headless runtime on the real map (`rust/psiv-campaign/tests/zosa_side.rs`):
  standing on the row with two Feeds runs the scene, empties the bag of Feeds, writes slot 2 and
  clears it at `$2D0`. The negative control, an empty bag, starts nothing and writes no slot; it
  fails under the old trigger.
- No campaign chapter: the route carries no Pengu Feed into this shop. No oracle comparison, no
  visual claim and no persistence claim.
