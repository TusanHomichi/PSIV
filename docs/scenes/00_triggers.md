# Opening-act triggers

The trigger layer is `RunEvents` -> `RunEventsJmpTbl`, evaluated every rest
frame against the current map's event-id list. Each entry is a tiny formulaic
check: some event-flag tests, sometimes a position test, then `Event_Index = N`
and `d7 = 1` to flip the game mode.

## Where it lives in retail

| Thing | Retail address | How it was located |
|---|---|---|
| `RunEventsJmpTbl` | **`$560E8`** | the unique run of exactly 128 `bra.w` (`$6000`) opcodes at a 4-byte stride, scanning `$55000`–`$58000` |
| `RunEvent_NoEvent` | `$57620` | `moveq #0,d7 / rts`; the shared "condition not met" exit |
| `EventFlags_Test` | `$57624` | learned by aligning `RunEvent_FindingAlys` against retail |
| `EventFlags_Set` | `$57666` | learned by aligning `Event_MeetingHahn` against retail |
| `Interaction_GetEvent` | `$58A4C` | byte-pattern search for `andi.w #$FF,d4 / add.w d4,d4 / move.w (a0,d4.w),d4 / move.w d4,(Event_Index).w` |
| `Interaction_EventIndexes` | **`$64F22`** | the `lea` operand immediately preceding that body |

Trigger routine addresses were resolved by decoding each `bra.w` displacement in
the table, which needs nothing from the clone but the ordered list of names.

## Fork status: clean

The brief said these are not fork-contaminated, and that holds. `RunEventsJmpTbl`
itself is behind `if grand_cross=1 / include "script/runeventsjmptbl.asm" / else`
and **the include file does not exist**, so with `grand_cross=1` the clone cannot
assemble — but the `else` branch it falls back to for reading is retail.

Five opening-act trigger routines were nevertheless byte-verified against
cartridge bytes. All five match the clone's `else`-branch source instruction for
instruction, operand for operand:

- `RunEvent_Null00`/`01`/`02` @ `$562E8`/`$562EC`/`$562F0` — verified
- `RunEvent_FindingAlys` @ `$562F4` — verified
- `RunEvent_BasementContainers` @ `$5637E` — verified
- `RunEvent_SuspicionOnPrincipal` @ `$563AE` — verified
- `RunEvent_AfterIgglanova` @ `$56884` — verified
- `RunEvent_PiataChazAlone` @ `$5760A` — verified
- `RunEvent_ReenterPiata` @ `$57414` — verified
- `RunEvent_NoEvent` @ `$57620` — verified

No divergences found in the trigger layer.

## Which maps check which triggers

From the per-map event byte lists (`generated/maps.json`, field `events`; these
are `$FF`-terminated byte lists of *trigger indexes*, not event indexes). The
whole Piata area:

| Map | id | Tree | Trigger list |
|---|---|---|---|
| Piata | `$10` | 2 | `$00` (null) |
| PiataAcademy | `$11` | 1 | `$00` |
| PiataAcademyNearBasement | `$12` | 33 | `$00` |
| **PiataAcademy_F1** | `$13` | 1 | **`$03` FindingAlys, `$0A` SuspicionOnPrincipal, `$7C` PiataChazAlone** |
| AcademyPrincipalOffice | `$14` | 33 | `$04` (null) |
| AcademyBasement | `$15` | 33 | `$00` |
| AcademyBasement_B1 | `$16` | 33 | `$00` |
| **AcademyBasement_B2** | `$17` | 33 | **`$08` BasementContainers, `$13` AfterIgglanova**, `$00` |
| PiataDorm | `$18` | 2 | `$00` |
| PiataInn | `$19` | 1 | `$00` |
| PiataHouse1 | `$1A` | 1 | `$00` |
| PiataItemShop | `$1B` | 1 | `$00` |
| PiataHouse2 | `$1C` | 1 | `$00` |
| **Motavia (overworld)** | `$00` | — | `$06`, `$3A`, `$3B`, `$3C`, **`$5B` ReenterPiata** |

Two whole floors carry the entire act's positional logic. Everything else in
Piata enters scenes through dialogue, not position.

> **Note for the pack**: the runtime pack's per-map JSON **does not carry the
> event list**, although `psiv_tools/maps/records.py` already extracts it
> (`records.py:685`, emitted at `:733`). The interpreter needs it. This is a
> pack-emit gap, filed in the final report.

## The trigger table, uniform form

Condition format: all listed flags must hold, then the position predicate, then
`Event_Index` is set and the field mode flips to `$C`. Flags are event-flag ids
in the ~174-flag space; `set`/`clear` is the required state.

| Trigger | Retail | Flags required | Position predicate | Sets `Event_Index` |
|---|---|---|---|---|
| `$00` `RunEvent_Null00` | `$562E8` | — | — | *(none — `d7=0`)* |
| `$01` `RunEvent_Null01` | `$562EC` | — | — | *(none)* |
| `$02` `RunEvent_Null02` | `$562F0` | — | — | *(none)* |
| `$03` `RunEvent_FindingAlys` | `$562F4` | `$08 AlysFound` **clear** | `curr_y_pos >= $F0` **and** `curr_x_pos == $260` | `$03` |
| `$04` `RunEvent_Null04` | `$56328` | — | — | *(none)* |
| `$08` `RunEvent_BasementContainers` | `$5637E` | `$0D BasementContainers` **clear** | *(none — fires on entering the map)* | `$0C` |
| `$0A` `RunEvent_SuspicionOnPrincipal` | `$563AE` | `$09 PrincipalMeeting` **set**, `$0E PrincipalSuspicious` **clear** | *(none)* | `$0F` |
| `$13` `RunEvent_AfterIgglanova` | `$56884` | `$0F AfterIgglanova` **clear**, `$0B Igglanova` **set** | *(none)* | `$25` |
| `$5B` `RunEvent_ReenterPiata` | `$57414` | `$0C PrincipalConfession` **clear** | *(none)* | `$9E` |
| `$7C` `RunEvent_PiataChazAlone` | `$5760A` | `$15 PiataChazControl` **clear** | *(none)* | `$A0` |

Several of these predicates are worth calling out because they are weaker than
they look:

- `RunEvent_BasementContainers`, `RunEvent_SuspicionOnPrincipal`,
  `RunEvent_AfterIgglanova`, `RunEvent_PiataChazAlone` and
  `RunEvent_ReenterPiata` have **no position test at all**. They fire on the first rest frame after the map's event list
  starts being scanned — effectively "on entering this map, if the flags say
  so". The flag the scene sets is the only thing that stops them repeating.
- `RunEvent_FindingAlys` is an **exact X match**, not a range: `cmpi.w #$260 /
  bne`. Only the `y` test is a range (`bcs` on `$F0`).
- `RunEvent_ReenterPiata` fires anywhere on the Motavia overworld. It is a
  blanket "you may not be out here yet" gate, not a gate on a particular exit.

## Trigger positions are pixels; map placements are 8-pixel units

`RunEvent_FindingAlys` compares `curr_x_pos`/`curr_y_pos`, which are pixel
values. `Map_Start_X_Pos`/`Map_Start_Y_Pos`, which scenes write when they load a
map, are in **8-pixel units** — `loc_535D4` does `lsl.w #3` on both before
storing them into the character objects. Do not mix the two.

Worked example, `Event_GameStart` -> `RunEvent_FindingAlys`: GameStart writes
`Map_Start = ($60, $24)`, so Chaz is placed at `curr = ($300, $120)` pixels.
The Alys trigger wants `curr_x_pos == $260` with `curr_y_pos >= $F0` — so Chaz
walks left along a row at or below `y = $F0`.

## Non-positional entry paths

Most Piata scenes are entered from the dialogue system, not the trigger table.
Two mechanisms:

### 1. Interaction areas, `interaction_type = 2`

`Interaction_ChkMapAreas` returns byte 9 of the 10-byte area record in `d4`;
`interaction_type` (byte 8) selects the handler. Type 2 is
`Interaction_GetEvent` (`$58A4C`), which does **not** use the parameter as an
event index directly — it indexes the word table
**`Interaction_EventIndexes` @ `$64F22`**:

```
$64F22: 0000 0029 002D 0035 0036 0039 003A 0042   ; params $00-$07
        0008 0066 801E 006B 006C 006D 006E 006F   ; params $08-$0F
        0000 0000 0000 0013 0000 0000 0000 0000   ; params $10-$17
        0000 0000 0000 001B 001C 0000 0000 0000   ; params $18-$1F
```

Bit 15 set means a cutscene index; clear means a plain `Event_Index`.

Across the whole Piata area there is **exactly one** such area:

| Map | Tile | Range | Param | Resolves to |
|---|---|---|---|---|
| AcademyBasement_B2 `$17` | (30, 18) | `XPlus20_YPlus10` | `$0B` | `Event_Index = $6B` `Event_IgglanovaBattle` |

### 2. Dialogue `$F6` event codes

`$F6` inside a dialogue entry fires an event instead of showing a message; its
operand is read as a **word** (`$58C9C`), and bit 15 selects cutscene vs event.
The entries reached are bare 4-byte stubs (`F6 xx xx FF`) that a `$FA` flag
chain in another entry jumps to. Opening-act uses, all in **tree 33**:

| Tree | Entry | Bytes | Operand | Fires | Reached from |
|---|---|---|---|---|---|
| 33 | `$14` | `f6 00 04 ff` | `$0004` | `Event_MeetingHahn` | entry `$10` (Hahn, map `$12`, dlg id `$10`), via flag `$09` |
| 33 | `$15` | `f6 80 01 ff` | `$8001` | `Cutscene_PiataPrincipal` | entry `$00` (principal, map `$14`, dlg id `$00`), via flag `$08` |
| 33 | `$16` | `f6 00 26 ff` | `$0026` | `Event_PrincipalConfession` | entry `$00`, via flag `$0B` |

Tree 33's entry `$00` is a **pure router** — 31 bytes of `$FA` chain and no text
of its own. Its full chain, in evaluation order (first flag that is *set* wins):

```
$FA $DB +7 -> $07     $FA $0C +2  -> $02   (PrincipalConfession done)
$FA $DA +6 -> $06     $FA $0B +22 -> $16   (Igglanova beaten -> Event_PrincipalConfession)
$FA $63 +5 -> $05     $FA $0A +1  -> $01   (Hahn joined)
$FA $34 +4 -> $04     $FA $09 +24 -> $18   (principal met)
$FA $33 +3 -> $03     $FA $08 +21 -> $15   (Alys found -> Cutscene_PiataPrincipal)
```

> **`$FA`'s second operand is a forward *delta*, not an absolute entry id.**
> `Interaction_ProcessDialogueTree` (`$5887A`) calls `GetOffsetByID` (`$5917C`),
> which is `GetDialogueByID` **without** the `lea $FFFF3000` — it advances from
> wherever `a0` already is, i.e. from inside the current entry. So the target is
> `current_entry + N`. `psiv_tools/dialogue_pack/flow.py` calls this field
> `then_entry` and documents it as an absolute id; that is wrong and mis-wires
> every `$FA` branch in an entry whose own id is not 0. Filed in the final
> report.

`Event_PiataGuardsReprimand` (`$9E`) is fired by **neither** mechanism — it has
no `$F6` anywhere in any of the 43 trees. It comes from trigger `$5B` on the
overworld, as tabulated above.

## Open questions for the oracle

1. **Rest-frame semantics of a no-position trigger.** `RunEvents` returns early
   unless both `x_step_duration` and `y_step_duration` are zero, so triggers are
   landing-evaluated like transitions. For a trigger with *no* position test on
   a map you were just placed on: does it fire on the very first frame after
   `GameMode_LoadFieldMap`, before the player can input anything? Oracle claim:
   *entering AcademyBasement_B2 with flag `$0D` clear fires event `$0C` within N
   frames of the map load, with N ≤ 2 and no input required.*
2. **Whether `RunEvent_FindingAlys` can be walked past.** X is an exact compare
   and the check only runs at rest. Oracle claim: *walking left along row
   `y = $100` in map `$13`, the trigger fires on the landing at `x = $260` and
   at no other x.* Column `$260` (cell 38) is walkable only at rows 16 and 17
   per the extracted collision, so this should be unmissable — confirm.
