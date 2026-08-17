# Retail Dezo boundary census

This is an audit record, not a registered scene. It keeps retail bodies that
were enumerated but deliberately left outside the typed chain visible instead
of laundering them into a fake implementation.

## Provenance

The reference source is a Grand Cross hack build: `reference/ps4.options.asm:10`
sets `grand_cross = 1`. The retail branches of `EventPtrs`, `CutscenePtrs` and
`RunEventsJmpTbl` in `reference/ps4disasm/ps4.asm` were read against the ROM
`Phantasy Star IV (USA).md`; hack-only `script/scenes` bodies are excluded.

## Enumerated but not registered

| Table entry | Retail body | ROM bytes | Reason left out |
|---|---|---|---|
| `CutscenePtrs[$12]` / `$8012` | `Cutscene_RajaSick` | `$07734C..$077787` | direct player/dialogue handoff outside `$33..$54`; not needed by the typed event body |
| `EventPtrs[$49..$4B]` | Musk Cats / Elder / Penguin Owner | `$07069E..$070773` | side interaction content, not a `$33..$54` chain writer |
| `EventPtrs[$4F]` | `Event_EsperGuardPermission` | `$0709A2..$070A29` | Inner Sanctuary permission interaction; direct dialogue/control surface |
| `EventPtrs[$51..$52]` | Inner Sanctuary guards | `$070A4E..$070A9D` | direct guard dialogue gates around Elsydeon |
| `EventPtrs[$5B..$5C]` | Raja Sick / Gyuna | `$070C30..$070CB3` | player-controlled Raja/Gyuna interaction prelude |
| `CutscenePtrs[$1B]` / `$801B` | `Cutscene_Rykros` | `$07818E..$078345` | later Aero Prism/Rykros player gate, outside the `$33..$54` dispatch delta |
| `EventPtrs[$62]` | `Event_AngerTowerAlys` | `$07148A..$071579` | direct Alys interaction and battle `$18`; its post-battle handoff is not a RunEvents `$33..$54` body |
| `EventPtrs[$66..$68]` | Hunters Guild / Rune healing / fifth-character selection | `$0718E6..$0721CB` | optional guild/roster interactions, not the fixed endgame dispatch chain |
| `CutscenePtrs[$21]` / `$8021` | `Cutscene_Ending` | `$078F3E..$0FFFFFF` | final ending/credits presentation; typed chain intentionally stops at `$8020` battle request |

The omitted bodies remain enumerated in the pointer census in
`12_ArcTriggerCensus.md`. No Grand Cross-only body is counted as a retail
scene, and no missing scene is hidden behind a placeholder registry entry.

