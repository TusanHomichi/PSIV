# PSIV sound runtime

This crate is the live product path for the hybrid sound plan in
`docs/SOUND_SCOUT.md`. It deliberately does not read JSON or ROM bytes. The
future extraction lane maps its `runtime-pack/sound/` records into
`SoundSequence`:

- `SoundSequence.id` is the dispatched PSIV sound ID (`$81..$FA`), and
  `tempo` is the scheduler tempo byte.
- `fm_voices` contains the resolved 25-byte local voice records as
  `FmVoice::from_bytes` values.
- `psg_envelopes` contains normalized signed attenuation deltas.
- Each `SoundTrack` carries its raw command bytes, kind, optional resolved
  channel, and channel-local initial metadata. `fm_auto`/`psg_auto` leave
  allocation to the runtime's six-FM/three-PSG channel pools.

The stable comparison surface is `SoundMachine::take_register_log()`. Fixture
tests compare the ordered `(tick, chip, port, register, value)` writes. Future
oracle captures can replace expected vectors without changing the extraction
interface or renderer.

## YM core decision

`build.rs` compiles the repository's existing Nuked OPN2 translation unit at
`oracle/gpgx-src/core/sound/ym3438.c` with YM2612 mode enabled. A small C
wrapper hides the large chip state, converts register writes to the OPN2
address/data port protocol, clocks the 24 internal cycles per native 53.267
kHz chip sample, and linearly resamples that stream to the requested 44.1 kHz
output. This is cleaner and safer than a partial Rust rewrite of the
accuracy-critical envelope/phase core, and it keeps the oracle's existing
source and license provenance in one place. The source's LGPL notice remains
authoritative for any distributable build.

The SN76489 is implemented directly in `psg.rs`: latch/data writes, three
tone counters, attenuation, noise control, and the 15-bit LFSR are all native
Rust. Its model is intentionally an independently oracle-checkable seam.

## Interpreter boundary

The Rust interpreter recognizes the complete scout vocabulary: note/rest and
delay bytes; `E0..EF`; `F0..F9`; `FA..FE`; and `FF/00` metadata. FM/PSG
frequency, key, instrument, pan, volume, modulation, loop/subroutine, noise,
transpose, FM3, tempo, and nested sound requests emit or update state in
driver order. DAC commands (`E3`, `E4`, `ED`, `EE`, `FA`, `FC`, `FD`) consume
and retain their control values, but do not pretend to render samples until
the extraction lane supplies the byte-exact DAC records and the small DAC
stream path is added. Malformed lengths, jumps, stack use, channels, voices,
and envelopes surface as `SequenceError`.

The current fixture is intentionally hand-built. It is a boot/audition proof,
not a claim that retail music is extracted or that all channel stealing and
DAC behavior is finished.
