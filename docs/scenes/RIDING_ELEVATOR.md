# `Event_RidingElevator`

- **Retail bytes:** `$06C33E..$06C477` inclusive: code through the `rts` at
  `$06C46C`, then the arrival-door table `$06C46E..$06C477`.
- **Pointer:** `EventPtrs[$14]`; map trigger `$0D` tests live chunk `$53`.
- **Data:** `bioplant.rs`, `ELEVATOR_RIDE` (17 ops).

## Fade and frame ownership

The verified US image calls `PalFadeOut_ClrSpriteTbl` at `$06C346`, then
`DoMapTransitionData` at `$06C350` and `RefreshMap` at `$06C356`. Those
helpers select and load the destination; the caller owns both fades.
`Pal_FadeIn` is at `$06C398`, after the arrival chunk `$53` is written and
`DMAPlanes_VInt` at `$06C392` has spent one frame. There is no
`InitVRAMAndCRAM` call here.

The movement loop `$06C3AE..$06C3F2` spends one `DMAPlane_A_VInt` per pass
until the leader finishes the sixteen-pixel step. The four closing-door
stages spend one DMA frame each, through `$06C44E` or `$06C45C` according
to the collision plane. Their `$01` table counts feed the CPU-only `dbf`
at `$06C466`, not a second VBlank. Thus the five `WaitFrames { frames: 1 }`
ops are DMA waits, not estimates of either palette ramp. Keep them when
the fixed CRAM fades spend their own frames. Camera behavior belongs to
[#565](https://github.com/TusanHomichi/PSIV/issues/565).

The local US-image census checks the range and the two calls; runtime
regressions trace the fades, map transition and closing chunks, and reject
an additional fade-length wait.
