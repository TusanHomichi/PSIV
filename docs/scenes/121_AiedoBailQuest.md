# The bail quest: `Event_GirlPrison`, `Event_GirlsBail`, `Event_PayingGirlsBail`, `Event_GirlsBailedOut`

The Hunter's Guild job "Stain in Life" (`HuntersGuildData` row 5: `EventFlag_StainInLife` `$78`,
`EventFlag_GirlsBailedOut` `$7D`, `EventFlag_StainInLifeFee` `$7E`) ends in Aiedo's prison
(`AiedoPrison` `$05C`, tree 11): the sisters caught at the supermarket (`EventFlag_GirlsCaught`
`$46`) are bailed out for 50,000 meseta and taken home to Uzo.

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0080` `Event_GirlPrison` | `$072C2E..$072C53` inclusive, 38 bytes | `EventPtrs[$80]` at `$05A2B4 + $200` | `aiedo_events.rs`, `GIRL_PRISON` (3 ops) |
| `$0081` `Event_GirlsBail` | `$072C54..$072C79` inclusive, 38 bytes | `EventPtrs[$81]` at `$05A2B4 + $204` | `aiedo_events.rs`, `GIRLS_BAIL` (3 ops) |
| `$0082` `Event_PayingGirlsBail` | `$072C7A..$072CDB` inclusive, 98 bytes | `EventPtrs[$82]` at `$05A2B4 + $208` | `aiedo_events.rs`, `PAYING_GIRLS_BAIL` (9 ops) |
| `$0083` `Event_GirlsBailedOut` | `$072CDC..$072D43` inclusive, 104 bytes | `EventPtrs[$83]` at `$05A2B4 + $20C` | `aiedo_events.rs`, `GIRLS_BAILED_OUT` (8 ops) |

- **Triggers:** dialogue `$F6` in `DialogueTree11`. (`$FA`'s operand is a forward delta:
  [trigger census](TRIGGER_CENSUS.md).)
  - The sister (object 1, dialogue `$58`): `$FA $7C +3`, `$FA $7A +2`, `$FA $79 +1`. With
    the mother spoken to (`EventFlag_MissingGirlsMom` `$79`) and nothing later, entry `$59`
    fires `$80`. Once the bail is paid (`$7C`) entry `$5B` fires `$83`.
  - The guard (object 0, dialogue `$4D`): `$FA $7D +8`, `$FA $7C +7`, `$FA $7B +19`,
    `$FA $7A +2`. After the sister has spoken (`$7A`) entry `$4F` fires `$81`. After the guard
    has named the bail (`$7B`) entry `$60` fires `$82`.
- **Flags** (`ps4.constants.asm:1562-1569`): `EventFlag_GirlPrison` `$7A`,
  `EventFlag_PrisonGuard` `$7B`, `EventFlag_BailPaid` `$7C`, `EventFlag_GirlsBailedOut` `$7D`,
  `EventFlag_Bail` `$7F`. Readers outside these four bodies: the guild (`$66`) reads `$7D` as
  the job's done flag, and the map-data routines of `AiedoPrison` and `UzoHouse3`
  (`ps4.asm:109167`, `:110551`; the pack's `map_effects`) hide or show the sisters on `$7D`.

## Clone audit

All four bodies were read from the US image through their `EventPtrs` slots, and every
instruction word matches `ps4.asm:152622-152705` on the US branch. Two hold `revision`
conditionals, and the US bytes take the `revision>0` side of both:

- `Event_PayingGirlsBail` compares `cmpi.b #$3A, -$2(a0)` (`0C28 003A FFFE` at `$072CC2`). The
  `revision=0` operand is `$AB`.
- `Event_GirlsBailedOut` turns object 0 after `RefreshMap`, at `$072D2A`. `revision=0` turns it
  before the dialogue.

None calls `DialogueTreesToRAM`. `$83`'s last line is entry `$0A` of `UzoHouse3`'s own tree 25,
which `RefreshMap` loads.

## Retail transcription

`$80` and `$81` share `Event_RockyOwner`'s shape ([120](120_RockyOwner.md)): face the leader,
one entry, one flag.

| Event | Op | ROM offsets | Retail primitive | Scene record |
|---|---:|---|---|---|
| `$80` | 0 | `$072C2E..$072C43` | `lea ($FFFFC340).w, a4` (object 1) … `jsr Event_UpdateObjFacing` | `FaceOppositeOf` object 1, of the leader |
| `$80` | 1-2 | `$072C44..$072C53` | `moveq #$5F, d0` / dialogue; `moveq #$7A, d0` / `jmp EventFlags_Set` | entry `$5F`; `SetFlag` `$7A` |
| `$81` | 0 | `$072C54..$072C69` | `lea (Field_Obj_Secondary).w, a4` (object 0) … `jsr Event_UpdateObjFacing` | `FaceOppositeOf` object 0, of the leader |
| `$81` | 1-2 | `$072C6A..$072C79` | `moveq #$50, d0` / dialogue; `moveq #$7B, d0` / `jmp EventFlags_Set` | entry `$50`; `SetFlag` `$7B` |

### `Event_PayingGirlsBail`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072C7A..$072C81` | `moveq #$7F, d0` / `jsr EventFlags_Clear` | `SetFlag` `$7F` false |
| 1 | `$072C82..$072C8D` | `move.l (Current_Money).w, d0` / `subi.l #50000, d0` / `bmi.s loc_72C96` | `BranchIfMoneyBelow` 50,000 |
| 2 | `$072C8E..$072C95` | `moveq #$7F, d0` / `jsr EventFlags_Set` | `SetFlag` `$7F` |
| 3 | `$072C96..$072CAB` | `lea (Field_Obj_Secondary).w, a4` … `jsr Event_UpdateObjFacing` | `FaceOppositeOf` object 0, of the leader |
| 4 | `$072CAC..$072CB3` | `moveq #$51, d0` / `jsr Event_GetAndRunDialogue` | entry `$51` |
| 5 | `$072CB4..$072CB9` | `tst.b (Yes_No_Option).w` / `bne.s loc_72CDA` | `BranchChoice`: NO ends |
| 6 | `$072CBA..$072CC9` | `popdlg` / `cmpi.b #$3A, -$2(a0)` / `bne.s loc_72CDA` | `BranchIfSavedDialogueByte` `Last`, `$3A` |
| 7 | `$072CCA..$072CD1` | `subi.l #50000, (Current_Money).w` | `SubtractMoney` 50,000 |
| 8 | `$072CD2..$072CD9` | `moveq #$7C, d0` / `jsr EventFlags_Set` | `SetFlag` `$7C` |
| - | `$072CDA` | `rts` | end of ops |

Entry `$51` names the bail and asks with `$F5 $01 $00`. YES counts one terminator on, to entry
`$52`, and NO continues inside `$51`. Entry `$52` opens `$FA $7F $01`, so a covered purse goes
on to `$53`. Of the three texts the question can end in (`$51`'s NO tail, `$52`, `$53`), only
`$53`'s last byte before its terminator is `$3A` (`!`); the other two end on `$35` (`.`). After
`popdlg`, `a0` is past the terminator, so `-$2(a0)` is that last byte. The guard therefore takes
the money only on a YES with `$7F` set, which the body set from the purse a moment before.

Two ops are new here, and the saved-byte branch gains a displacement:

- `SceneOp::BranchIfMoneyBelow` reads the sign of the longword difference, as `bmi` does. A
  purse of exactly 50,000 pays.
- `SceneOp::SubtractMoney` is the unchecked `subi.l`.
- `BranchIfSavedDialogueByte`'s `at: SavedByte::Last` reads `-$2(a0)`.
  [`Event_Gyuna`](98_Gyuna.md)'s `(a0)` is `SavedByte::Next`. The runtime reports both bytes
  when a dialogue closes. `psiv-runtime`'s `dialogue/stop_byte.rs` rebuilds the last one from
  the pack's segments (a glyph, or a control's last operand), and
  `every_entry_closes_on_the_byte_its_segments_give` and `every_f7_closes_on_the_byte_before_it`
  hold that rebuild to every entry's `raw_hex`.

### `Event_GirlsBailedOut`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072CDC..$072CE3` | `moveq #$61, d0` / `jsr Event_GetAndRunDialogue` | entry `$61` |
| 1 | `$072CE4..$072CEB` | `moveq #$7D, d0` / `jsr EventFlags_Set` | `SetFlag` `$7D` |
| 2 | `$072CEC..$072CF3` | `move.b #MusicID_AHappySettlement, (Sound_Index).l` (`$9E`) | `PlaySound` `$9E` |
| 3 | `$072CF4..$072CF9` | `jsr PalFadeOut_ClrSpriteTbl` | `FadeOut` |
| 4 | `$072CFA..$072D29` | `Field_Map_Index = $E4` (`UzoHouse3`), `Field_Map_Index_2 = $E0` (Uzo), start `($38, $46)`, facing 4 (up), align 4, `bclr #3, Map_Load_Flags`, `jsr RefreshMap` | `LoadMap` |
| 5 | `$072D2A..$072D35` | `lea (Field_Obj_Secondary).w, a4` / `moveq #0, d0` / `jsr Event_UpdateObjFacing` | `Face` object 0 down |
| 6 | `$072D36..$072D3B` | `jsr Pal_FadeIn` | `FadeIn` |
| 7 | `$072D3C..$072D43` | `moveq #$A, d0` / `jmp Event_GetAndRunDialogue` | entry `$0A` (tree 25) |

## Tests

- `rust/psiv-core/tests/aiedo_scenes.rs`: the two talks face the leader, speak and set their
  flags. The bail pays on YES and `$3A`, including a purse of exactly 50,000. A short purse
  clears a stale `$7F`. Three negative controls pay nothing: NO with `$3A`, and YES with `$35`
  or no stop. The bailed-out scene loads `UzoHouse3` with the cited start and turns object 0
  after the load.
- `rust/psiv-runtime/src/dialogue/stop_byte/tests.rs`: with the real tree 11, YES with `$7F` set
  ends on `$3A`. YES without it, and NO either way, end on `$35`.
- `rust/psiv-campaign/tests/aiedo_side.rs`, `the_bail_quest_runs_from_the_cell_to_uzo`: from a
  hand-built `AiedoPrison` save (`$46`, `$78`, `$79`, 60,000 meseta) the driver talks to the
  sister, then the guard twice, answers YES, talks to the sister again and lands in `UzoHouse3`
  with 10,000 meseta and `$7A`..`$7D` set. `a_short_purse_pays_nothing` and `no_pays_nothing`
  are its controls. With `SavedByte::Next` in place of `Last` the quest test fails at `$7C`.
