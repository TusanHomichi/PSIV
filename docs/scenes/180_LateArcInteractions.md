# Late-arc interactions: the musk cats, the penguin keeper, Raja, Rune and Hijammer

Six direct interactions: each runs when the party talks to one object, and branches on the state at
that moment. The census grouped them as "Dezolis late arc"; read against the cartridge they are five
places.

| Event | Retail bytes | Fired by | Data |
|---|---|---|---|
| `$0049` `Event_MuskCatsGuarding` | `$07069E..$0706BF` | `$F6`, entry 48 of tree 17: both `MuskCatGuard`s on `MystVale_Part2` `$159` | `late_arc.rs`, `MUSK_CATS_GUARDING` (4 ops) |
| `$004A` `Event_MuskCatElder` | `$0706C0..$070701` | `$F6`, entry 44 of tree 17 (after its `$FA $91 $03`): the chief and his two invisible blocks on `MystVale_Part5` `$15C` | `MUSK_CAT_ELDER` (6 ops) |
| `$004B` `Event_PenguinOwner` | `$070702..$070773` | `$F6`, entry 17 of tree 17: object 16 of `Zosa` `$126` | `PENGUIN_OWNER` (12 ops) |
| `$005B` `Event_RajaSick` | `$070C30..$070C81` | `$F6`, entry 101 of tree 18: `RajaInBed` and two invisible blocks on `MeeseClinic_F1` `$134` | `RAJA_SICK` (11 ops) |
| `$0067` `Event_RuneHealingChaz` | `$071E56..$071E65` | `$F6`, entry 55 of tree 20: `NPCRune` on `InnerSanctuary_B1` `$16F` | `RUNE_HEALING_CHAZ` (17 ops) |
| `$006D` `Event_Hijammer` | `$072428..$0724A3` | interaction area `$0D`, the first chest of `Kuran_F1_Part2` `$193`, gated on chest flag `$5E` | `HIJAMMER` (14 ops) |

## Clone audit

Each body was read from the US image through `EventPtrs` (`$05A2B4`); the instruction words match
`ps4.asm:149861-149929`, `:150224-150250`, `:151675-151680` and `:152099-152125`. The only
conditional is `$4B`'s `if revision=0` at `ps4.asm:149915`: the US image at `$070752` is
`0C28 003A FFFE`, `cmpi.b #$3A, -$2(a0)`. Nothing here calls `DialogueTreesToRAM`.

## Retail transcription

### `Event_MuskCatsGuarding`

Entry `$1C`, then `move.b #$1D` into `$14` and `$54` off `Field_Obj_Secondary` (`SetNpcDialogue` on
objects 0 and 1), then `EventFlag_MuskCats` `$90`. `FieldObj_MuskCatGuard` (`ps4.asm:99535`) tests the
flag every frame and walks aside (`BespokeFlag::MuskCats`); `MapDataMan_MystValePart2` seats them aside
on a later load.

`Event_PosiBolt` also sets `$90` (`ps4.asm:152176`, the US bytes agree): a retail quirk the guards read,
kept as it is ([111](111_WrenSkillChests.md#event_posibolt)).

### `Event_MuskCatElder`

The routine counts the non-zero bytes of `Inventory` and `bcs` on fewer than 40. Full: entry `$2E`, which
stops before the gift, and nothing is written, so the talk offers the tusk again. Otherwise entry `$2D`, the
first zero byte takes `ItemID_SilvrTusk` `$53` (`AddItem`), and `EventFlag_SilverTusk` `$91` is set, which
entry 44's `$FA $91 $03` reads: a second talk reads entry `$2F` and never reaches the event.

`BranchIfInventoryFull` is new: no other scene asks.

### `Event_PenguinOwner`

| Ops | ROM | Retail | Scene |
|---:|---|---|---|
| 0-1 | `$070702..$070715` | `lea $FFFFC700, a4`, the leader's facing, `bchg #2`, `Event_UpdateObjFacing` | `FaceOppositeOf` object 16, `WaitFrames` 1 |
| 2-5 | `$070716..$070737` | `cmpi.l #500, (Current_Money).w / bcc.s`: below it sets `EventFlag_PenguinNoMoney` `$8C`, otherwise clears it | `BranchIfMoneyBelow` `Unsigned`, `SetFlag`, `SkipOps`, `SetFlag` |
| 6 | `$070738..$07073F` | `moveq #$12, d0 / jsr Event_GetAndRunDialogue` | `standard($12)` |
| 7 | `$070740..$070745` | `tst.b (Yes_No_Option).w / bne.s` | `BranchChoice` |
| 8 | `$070746..$070757` | `popdlg / cmpi.b #$3A, -$2(a0) / bne.s` | `BranchIfSavedDialogueByte`, `Last`, `$3A` |
| 9-11 | `$070758..$070771` | `subi.l #500`, `move.b #$15, $14(a0)`, `EventFlag_Penguin` `$8A` | `SubtractMoney`, `SetNpcDialogue`, `SetFlag` |

US retail's `Event_UpdateObjFacing` ends in `jsr VInt_Prepare` (`$05A972`): one frame that runs no
map update but whose VInt stirs the seed and counts `Main_Frame_Count`. The port's facing ops take no
frame (#164), so both of this record's turns, the keeper's and Raja's, are followed by `WaitFrames` 1,
and neither scene is a frame short.

The entry does the choosing: its `$F5` takes NO to the entry two on, and its `$FA $8C` takes a short
YES to the next. Only the paid YES ends on `!` (`$3A`), and that byte is what the routine reads.
`bcc` is unsigned, so the op runs `MoneyTest::Unsigned`: a purse of `$800001F4` covers the fee, where
the bail scene's `bmi` (`MoneyTest::SignOfDifference`) would call it short.

`EventFlag_Penguin` is what `FieldObj_FellowPenguin` waits on (`ps4.asm:98848`): object 10 follows the
party's last member (`BespokeKind::FlaggedFollow`). Its first frame on a load clears the flag, so leaving
town or going indoors loses the penguin and the 500 meseta with it.

### `Event_RajaSick`

The object at `$FFFFC400` turns to the leader, which spends the facing's frame. `EventFlag_DarkForce2` `$9E` set: entry `$60`. Clear:
`jsr UpdateRNGSeed / move.w (RNG_Seed).w, d0 / andi.w #3, d0` picks `$5E` on 0, `$5F` on 1 and `$5D` on 2 or 3.
The test comes before the draw, so after Dark Force 2 the talk leaves the stream alone. The draw comes
after the facing frame's VInt and the next frame's, as on the cartridge.

`BranchOnRngLow2` is new. The draw is one `Lcg41::step` of the runtime's own stream
(`rust/psiv-core/src/battle/rng.rs`), the stream every other consumer reads, taken within the frame
the op runs: the runtime lends the stream to the runner around `tick_with`
(`rust/psiv-runtime/src/scene_runtime.rs`), so no frame is spent on the request. A runner with none lent
faults (`SceneFault::NoRng`).

### `Event_RuneHealingChaz`

Entry `$2B`, then `jsr Event_Recovery` (`$06D37C`, the census's direct call from `$67` to `$21`). The
scene's ops are `standard($2B)` and the 16 of [`Event_Recovery`](102_Recovery.md), shared as
`RECOVERY_OPS`, so the two cannot drift apart: the flashes, both sounds and `RecoverStats`.

### `Event_Hijammer`

The shape of [`Event_Burstroc`](111_WrenSkillChests.md): the chest's lid (`moveq #4, d0` on `$FFFFC340`,
object 1, the slot after the map's one object), 40 frames, entry `$0A` stopping at its `$F7`, then the
implant sounds (40, 40, 20 and 60 frames, each `d0 + 1`), `popdlg` and `Event_RunDialogue` for the rest.
`move.b #SkillID_Hijammer, skills+4(a0)` on `Wren_Stats` is `SetCharacterSkill` slot 4, skill `$20`, and
`EventFlag_Hijammer` is `$5E`. The chest's own record names a Dagger, which is never awarded: the area
fires first, and its chest-flag gate closes it.

## Flags another event reads

| Flag | Set here by | Read by |
|---|---|---|
| `$90` `MuskCats` | `$49` (and `Event_PosiBolt`) | `FieldObj_MuskCatGuard`, `MapDataMan_MystValePart2` |
| `$91` `SilverTusk` | `$4A` | entry 44's `$FA` |
| `$8A` `Penguin` | `$4B` | `FieldObj_FellowPenguin` |
| `$8C` `PenguinNoMoney` | `$4B` | entry `$12`'s `$FA` |
| `$5E` `Hijammer` | `$6D` | the area's chest-flag gate |

## Tests

- `rust/psiv-core/tests/late_arc_scenes.rs`: every branch of each scene from the state that takes it,
  including the full bag, both purse sides, a NO, each of the four draws and the case that draws nothing,
  with a negative control for a runner with no stream.
- `rust/psiv-campaign/tests/late_arc.rs`: each event fired by pads on its real map from a hand-built
  save, since the route reaches none of these side talks and the clinic and Kuran's chest sit past most of
  the game. State and native-input-path evidence in the headless runtime; no visual claim.
