# `Event_KuranArrival`

- **Retail bytes:** `$06FAD4..$06FAE5` inclusive, 18 bytes.
- **Pointer:** `EventPtrs[$3D]` at `$05A2B4`; event `$003D`.
- **Trigger:** Kuran `$190`, `RunEventsJmpTbl[$2C]`, Kuran `$86` clear.
- **Data:** `post_zio_cutscenes.rs`, `KURAN_ARRIVAL` (2 ops).

## Clone audit

The retail `else` EventPtrs table at `ps4.asm:120635` names
`Event_KuranArrival`; the body at `ps4.asm:149124` is outside the Grand Cross
hack-only includes. Actual-ROM pointer range: `$06FAD4..$06FAE6` exclusive end.

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06FAD4..$06FADD` | dialogue entry `4` | standard dialogue |
| 1 | `$06FADE..$06FAE5` | set `EventFlag_Kuran=$86` | flag |

## In the route (C6)

The `kuran-arrival` chapter flies from the Dezo spaceport (`board`, `to: 4`,
destination mask `$D8` once `$82` is set) and lands on Kuran `$190` at (31,46).
`RunEvent_EnterKuran` (`$2C`) has no position test: it fires on the first field
frame after the landing, runs this scene and sets `$86`. Without `$82` the list
has no Kuran row (`kuran_arc.rs`, the negative control). The route then opens the
elevator door at (30..31,17) and rides Kuran's chain of regions to F3
([RUNNER_LOG_KURAN.md](../campaign/RUNNER_LOG_KURAN.md#the-elevator-chain)).
