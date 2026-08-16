# Next-arc trigger census: Piata gate through Birth Valley

Scouted from the retail per-map event lists in `generated/maps.json` and the
packed map records. Trigger ids below are `RunEventsJmpTbl` indices; the event
on the right is the `Event_Index` written by the routine. The pointer table
authority is retail `EventPtrs` at `$05A2B4` (161 `$A1` entries) and
`CutscenePtrs` at `$05A580`.

## Map lists

| Map | id | tree | per-map events | dispatch |
|---|---:|---:|---|---|
| Zema | `$24` / 36 | 4 | `$15,$17,$74` | `$8005`, `$8006`, `$008A` |
| BirthValley | `$2B` / 43 | 3 | `$00` | null; no scene |
| BirthValley B1 | `$2C` / 44 | 3 | `$00` | null; no scene |
| KrupKindergarten | `$3A` / 58 | 5 | `$16` | `$000D` / Saya |
| TonoeStorageRoom | `$42` / 66 | 7 | `$00` | null; direct door control below |
| TonoeGryzHouse | `$43` / 67 | 7 | `$14` | `$8004` after Dorin flag |
| TonoeBasement B3 | `$4A` / 74 | 7 | `$18` | `$0028` / Alshline found |
| ValleyMaze parts | `$9B..$A1` / 155..161 | 7 | `$00` | null |
| ValleyMazeOutside | `$D8` / 216 | 7 | `$30` | `$0027` / Rune Flaeli |
| ValleyMazeOutside2 | `$D9` / 217 | 7 | `$00` | null |

The list entries are not scene ids. For example, Zema's `$15` calls
`RunEvent_UsingAlshline`, which writes cutscene `$8005`; treating `$15` as an
event routine would dispatch the wrong pointer. Birth Valley's `$00` is the
retail null routine and is intentionally not represented by a scene.

## Direct dialogue controls on the same route

The map lists only cover `RunEvents`. Dialogue controls are a second dispatch
surface and are part of the playable arc:

| map tree | entry | control | event / scene |
|---|---:|---|---|
| tree 3 | `$6A` (106) | flag `$10`, then F6 | `$8002` `Cutscene_ProfHolt` |
| tree 7 | `$02` | F6 | `$8003` `Cutscene_MeetingRune` |
| tree 7 | `$17` (23) | flag `$65`, then F6 | `$0032` `Event_MeetingDorin` |
| tree 7 | `$29` (41) | flag `$65`, then F6 | `$0033` `Event_TonoeBasementDoor` |
| tree 4 | `$4C` (76) | F6 | `$008B` `Event_ZemaOldMan` |
| tree 4 | `$4E` (78) | F6 | `$008C` `Event_ZemaOldManAfterMission` |

## Trigger formulas transcribed in `trigger_table`

| trigger | retail routine | required state | event |
|---:|---|---|---|
| `$14` | `RunEvent_Dorin` | Dorin `$36` set, Gryz `$30` clear | `$8004` |
| `$15` | `RunEvent_UsingAlshline` | Alshline `$32` set, Zema Igglanova `$33` clear | `$8005` |
| `$16` | `RunEvent_MeetingSaya` | Saya `$12` clear, standing y `<=$280` | `$000D` |
| `$17` | `RunEvent_ZemaIgglanovaDefeated` | Zema Igglanova `$33` set, after-beat `$37` clear | `$8006` |
| `$18` | `RunEvent_FindingAlshline` | chest `$08` set, Alshline `$32` clear | `$0028` |
| `$30` | `RunEvent_RuneFlaeli` | Rune `$11` set, Tonoe path `$13` clear | `$0027` |
| `$74` | `RunEvent_ZemaServantBattle` | Silver Soldier `$B1` set, servants `$B2` clear | `$008A` |

The formulas are covered by the focused census test in
`rust/psiv-core/src/trigger_table.rs`. No `trigger_custom` addition was
needed: every trigger in these maps is already a flags/position condition.

## Fork and byte authority

`reference/ps4disasm/ps4.asm` is the Grand Cross clone, not a retail oracle.
For the retail-only scene includes (`MeetingSaya`, `RuneFlaeli`, `Alshline`,
`ZemaIgglanovaDefeated`, and the other conditional scene bodies), the
`grand_cross=1` branch is an absent `script/scenes/...` include; the retail
bytes below are the only executable authority. The surviving `grand_cross=0`
branches were checked against the retail pointer ranges before they were
expressed as `SceneOp` data. No clone executable is checked in, so “byte diff”
means retail disassembly versus the clone's surviving source branch, never a
pretend binary comparison.
