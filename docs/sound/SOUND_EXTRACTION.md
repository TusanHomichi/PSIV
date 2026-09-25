# Sound extraction

The sound section is a raw-data interface for a transcribed interpreter. It
does not convert the cartridge's modified SMPS dialect to MIDI, nor does it
invent a more convenient command language. Those conversions lose the
driver-specific timing, DAC controls, relative control flow, PSG envelopes and
FM3 behavior that the interpreter needs.

The implementation is split between:

* `psiv_tools.sound_defs`: source-labelled ids, command definitions and the
  fixed provenance regions.
* `psiv_tools.sound`: byte verification, control-flow census and deterministic
  emission.
* `psiv_tools.pack`: additive pack integration.

Run the standalone extractor with:

```text
python3 -m psiv_tools sound "Phantasy Star IV (USA).md" runtime-pack/
```

`python3 -m psiv_tools pack ... runtime-pack/` emits the same `sound/` tree
as part of the pack. `runtime-pack/` is generated and ignored; the raw blobs
are the canonical output, not the JSON's `raw_hex` convenience fields.

## Provenance and fail-closed behavior

The extractor accepts only the verified US retail image:

```text
sha256  511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a
size    3145728 bytes
```

Before a pointer is followed it verifies the structural anchors adjudicated in
`docs/sound/SOUND_SCOUT.md`, plus SHA-256 hashes for the pointer tables, fixed driver
constants, the embedded Z80 driver and the DAC bank table. The provenance
record in `sound/driver.json` names the checked-out source tree:
`reference/ps4disasm/sound` (`DefCFlag.txt`, `DefDrv.txt`, `Notes.txt`,
`Pointers.txt`, `ps4.sound_driver.asm`, and `ps4.dac_driver.asm`). A changed
clone or a ROM with a changed sound region raises `SoundError` before any
sound files are emitted. An unknown command, truncated operand, invalid
relative target, empty RETURN, invalid voice pointer or invalid DAC range is
also a hard failure. There are no placeholder records.

## Output

The sound subtree contains:

```text
sound/
  index.json                 # pack-facing inventory and census
  driver.json                # timing, command vocabulary, constants, envelopes, DAC
  music.json                 # all 0x81..0xB4 records and track metadata
  sfx.json                   # 0xB5..0xF7 and special 0xF8..0xFA records
  raw/
    music/*.bin              # complete pointer-delimited music records
    sfx/*.bin                # complete pointer-delimited regular SFX records
    special/*.bin            # complete pointer-delimited special SFX records
    driver/*.bin             # pointer tables, constants, envelopes and Z80 driver
    dac/*.bin                # bank tables and exact sample byte ranges
```

Every JSON file is sorted-key JSON with a trailing newline. Every raw file is
written from an exact ROM slice and is listed with size and SHA-256 in the
returned pack fragment and `sound/index.json`. No timestamp, filesystem order,
or generated audio format participates in the output.

## Record formats

### Music

The music id space is the 52 entries in `MusicPtrs` at `$D1C40`, ids `$81` to
`$B4`. Each record runs from its pointer to the next pointer, with the last
record ending at `$DE4B6` (the regular SFX pointer table). The six-byte header
is:

| Offset | Meaning |
| --- | --- |
| `+0` | big-endian word, record-relative FM voice-table pointer |
| `+2` | FM channel count |
| `+3` | PSG channel count |
| `+4` | per-channel tick multiplier |
| `+5` | global tempo reload |

Each FM descriptor is four bytes: a record-relative track pointer followed by
the initial transpose/volume word. Each PSG descriptor is six bytes: track
pointer, transpose/volume word, modulation-envelope index, and volume-envelope
index. In this image every music record has six FM and three PSG descriptors,
for 468 music tracks total, but the counts are read from each header rather
than assumed.

### Regular and special SFX

Regular SFX ids `$B5..$F7` come from the 67-entry table at `$DE4B6`; the three
special ids `$F8..$FA` come from `$DE5C2`. Regular data ends at the first
special record `$DF68A`; special data ends at `$DF78C`. The header is:

| Offset | Meaning |
| --- | --- |
| `+0` | big-endian word, record-relative FM voice-table pointer |
| `+2` | per-channel tick multiplier |
| `+3` | track count |
| `+4` | first six-byte channel descriptor |

An SFX descriptor is the raw channel/init word, a record-relative track
pointer, and a transpose/volume word. The raw channel word is retained because
the driver uses its encoded channel byte to select BGM/SFX channel state.

Every reachable `EF nn` selects FM voice `nn`; every reachable `F5 nn` selects
the PSG envelope/instrument index `nn`. Voice records are 25 bytes: one
algorithm/feedback byte, twenty operator register bytes, and four operator
total-level bytes. Each track's `instruments_used` and
`psg_instruments_used` lists are part of the census, so a consumer never has
to infer voice usage by scanning a converted format. The extractor emits every
25-byte definition in each record's voice table, including definitions not
referenced by a track; the current image has 234 music voices, 88 regular-SFX
voices and 5 special-SFX voices. A one-byte zero alignment pad is retained in
the record blob and reported separately when present.

## Timing and byte grammar

The driver entry is `$D0008` (`UpdateSound`). On each update, a nonzero global
tempo reload decrements `$1(a6)`; when it reaches zero it reloads from `$2(a6)`
and increments the active channels' `$E` counters. `TickMultiplier` multiplies
the note/delay token by channel `$2(a5)` and loads `$E`/`$F`. Music header `+5`
seeds the global reload and header `+4` seeds each channel multiplier. SFX
header `+2` seeds the channel multiplier. A zero global reload disables that
global tick path.

Track bytes below `$E0` are not commands:

* `$00..$7F` is a delay token and consumes one byte.
* `$80` is rest; `$81..$DF` are note tokens.
* A note/rest's following byte below `$80` is its delay and is consumed. If
  that byte has bit 7 set, the driver backs up and it is the next token.

The control-flow census follows every reachable path. `F7` contributes both
its taken loop edge and its exhausted fallthrough; `F8/F9` use a static return
stack. A track with no statically reachable `F2` is not reported as undecodable
when its loop/goto paths are valid; the count is retained as
`tracks_without_static_track_end` for interpreter authors.

## Complete command vocabulary

This table is the complete primary vocabulary from `DefCFlag.txt`, not merely
the commands that happened to occur. `Seen` is the census over all 557 tracks
(468 music, 86 regular SFX, 3 special SFX) in this ROM. The observed primary
vocabulary is 24 opcodes. All 32 primary entries are documented here, and the
only meta escape present is also documented below.

| Opcode | Type | Subtype | Bytes | Seen |
| --- | --- | --- | ---: | :---: |
| `E0` | `PANAFMS` | `PAFMS_PAN` | 2 | yes |
| `E1` | `DETUNE` | — | 2 | yes |
| `E2` | `SET_COMM` | — | 2 | no |
| `E3` | `DAC_PS4` | `PS4_VOLCTRL` | 2 | yes |
| `E4` | `DAC_PS4` | `PS4_LOOP` | 2 | yes |
| `E5` | `VOLUME` | `VOL_NN_FMP` | 3 | yes |
| `E6` | `VOLUME` | `VOL_NN_FM` | 2 | yes |
| `E7` | `HOLD` | — | 1 | yes |
| `E8` | `NOTE_STOP` | `NSTOP_NORMAL` | 2 | yes |
| `E9` | `SET_LFO` | `LFO_AMSEN` | 3 | no |
| `EA` | `TEMPO` | `TEMPO_SET` | 2 | no |
| `EB` | `SND_CMD` | — | 2 | no |
| `EC` | `VOLUME` | `VOL_NN_PSG` | 2 | yes |
| `ED` | `PANAFMS` | `PAFMS_PAN` (DAC feature) | 2 | yes |
| `EE` | `DAC_PS4` | `PS4_SET_SND` | 2 | yes |
| `EF` | `INSTRUMENT` | `INS_N_FM` | 2 | yes |
| `F0` | `MOD_SETUP` | — | 5 | yes |
| `F1` | `MOD_ENV` | `MENV_FMP` | 3 | no |
| `F2` | `TRK_END` | `TEND_STD` | 1 | yes |
| `F3` | `PSG_NOISE` | `PNOIS_SET` | 2 | yes |
| `F4` | `MOD_ENV` | `MENV_GEN` | 2 | no |
| `F5` | `INSTRUMENT` | `INS_N_PSG` | 2 | yes |
| `F6` | `GOTO` | — | 3 | yes |
| `F7` | `LOOP` | — | 5 | yes |
| `F8` | `GOSUB` | — | 3 | yes |
| `F9` | `RETURN` | — | 1 | yes |
| `FA` | `DAC_PS4` | `PS4_REVERSE` | 2 | yes |
| `FB` | `TRANSPOSE` | `TRNSP_ADD` | 2 | yes |
| `FC` | `DAC_PS4` | `PS4_VOLUME` | 2 | yes |
| `FD` | `DAC_PS4` | `PS4_TRKMODE` | 2 | no |
| `FE` | `SPC_FM3` | — | 5 | no |
| `FF` | `META_CF` | — | 1 + meta | yes |

The relative control-flow vocabulary is exact:

* `F6`: signed big-endian word at `opcode+1`; target is
  `opcode+2+signed_offset`.
* `F7`: index byte, count byte, signed big-endian word at `opcode+3`; taken
  target is `opcode+4+signed_offset`, exhausted fallthrough is `opcode+5`.
* `F8`: signed big-endian word at `opcode+1`; target is
  `opcode+2+signed_offset`, return address is `opcode+3`.
* `F9`: pops the most recent `F8` return address. An empty return stack is an
  undecodable record.

`FF` must be followed by meta opcode `00` (`PAN_ANIM`). The bytes are
`FF 00 00` when the selector is zero, or `FF 00 <nonzero> <four payload bytes>`
for a seven-byte form. Any other meta opcode or truncated form fails closed.

The machine-readable command table is in `sound/driver.json`; the exact
observed sets and counts are in `sound/index.json` under `census`. The
extraction tests assert that every observed primary opcode has a row in this
document's vocabulary and that no unresolved sequence bytes were emitted.

## Envelopes, constants and DAC

The raw envelope streams are emitted from the pointer tables at `$D1A80`
(eight modulation entries) and `$D1B44` (ten volume entries). Modulation
controls are `$80 RESET`, `$81 HOLD`, `$82 LOOP`, `$83 STOP`, and `$84 CHG_MULT`
followed by one multiplier byte. Volume controls are `$80 RESET`, `$81 HOLD`,
`$82 JUMP2IDX` followed by one index byte, and `$83 OFF`. Envelope spans and
terminal controls are recorded in `driver.json`; shared/overlapping streams
remain shared raw slices rather than being deduplicated into a new format.

The driver JSON also carries byte-verified FM/PSG frequency tables, FM init
bytes, PSG init bytes, FM operator register order, algorithm masks, volume
registers, FM3 frequency values, pan animation sequences, sound priorities,
all three pointer tables, and the embedded Z80 DAC driver from `$D153E` to
`$D1A3E`.

The DAC bank selector at `$D1A3E` maps sound ids `$81..$91` to four ROM banks:
`$E0000`, `$E8000`, `$F0000`, `$F8000`. The Z80 reads each table word as
little-endian, so the extractor reports the swapped Z80 address/length while
preserving the table and sample bytes exactly. Every sample range is checked
to remain inside its selected 32 KiB bank.

## Current census

The verified image currently emits 52 music records, 67 regular SFX records,
3 special SFX records, and 557 tracks. It sees 24 primary command opcodes and
meta opcode `00`; `unresolved` is empty. Map extraction passes all 361 map
music ids to the sound census, of which 23 distinct music ids are used by map
records. Unused music records are still emitted because the pointer table is
the complete id space.
