# `Cutscene_InsideSpaceship`

- **Retail bytes:** `$07606C..$07607D` inclusive, 18 bytes.
- **Pointer:** `CutscenePtrs[$0D]` at `$05A580`; scene event `$800D`.
- **Dispatch:** `RunEventsJmpTbl[$21/$23/$24/$25]` — Mota Spaceport,
  Kuran, Air Castle and Silence Tower entry surfaces all call the same body.
- **Data:** `post_zio_cutscenes.rs`, `INSIDE_SPACESHIP` (12 ops).

## Clone audit

The retail `else` pointer at `ps4.asm:120777` selects the body at
`ps4.asm:155427`. It is only an init/fade jump into the shared helper at
`loc_63BC4`; the Grand Cross clone's hack-only menu bodies are not treated as
retail scene records.

## Retail dispatch table

The helper's `loc_64B02`, `loc_64B34` and `loc_64B5A` tables are the authority:

| Current map | First destination | Intermediate destination | Final map / start |
|---|---|---|---|
| Mota Spaceport `$0BF` | Motavia `$000`, `($68,$B4)` | Motavia then selected-world flight | Mota Spaceport on cancel |
| Dezo Spaceport `$0D4` | Dezolis `$001`, `($18,$90)` | Dezolis `($18,$70)` | Dezo Spaceport on cancel |
| Le Roof Room | Rykros `($80,$80)` | Rykros `($80,$60)` | Le Roof Room |
| Zelan `$18D` | Zelan Space `$18C`, `($43,$3C)` | Zelan Space `($43,$1C)` | Zelan `($3E,$5A)` |
| Kuran `$190` | Kuran Space `$18F`, `($43,$3C)` | Kuran Space `($43,$1C)` | Kuran `($3E,$5A)` |
| Air Castle | Air Castle Space | Air Castle Space | Air Castle |

The headless arc selects world index `3` (Zelan), so its explicit route is:

```text
MotaSpaceport -> Motavia ($68,$B4)
             -> ZelanSpace ($43,$1C)
             -> Zelan ($3E,$5A)
```

That is a route realization of a player menu, not a claim that the ROM always
chooses Zelan. The route table above preserves the other retail branches.

## Retail transcription

| Ops | ROM offsets / helper | Retail primitive | Scene record |
|---:|---|---|---|
| 0-3 | `$07606C..$07607D`, `loc_63BC4` entry / `loc_64568` | init, fade, Takeoff Landeel music and saved track | init/fade/music |
| 4-7 | `loc_64B02`, `loc_64568` | load Motavia; `224` flight iterations; SpaceshipPropelled SFX; remaining `195` iterations | `LoadMap`, waits/SFX |
| 8-9 | `loc_64B34`, `loc_64800` | load Zelan Space; `$20000..0` flight loop = `257` iterations | `LoadMap`, `Wait(257)` |
| 10-11 | `loc_64B5A` | load Zelan at `($3E,$5A)`; return `0` | `LoadMap`, return |

## Boundary

The menu's alternate worlds remain player-controlled field routes. The next
fixed story body is `Cutscene_SpaceshipSabotage` (`$800E`) after the Zelan
route and Wren/Canceller gates.
