# Video and scene captures

[Oracle setup and index](README.md)

### Instruction-boundary timing

The host's optional `--pc-trace <path.csv>` records the instruction boundaries
selected by repeated `--pc-watch <hex>` arguments. It requires a core rebuilt
with `oracle/build_core.sh`; older cores remain usable when the option is off.
Use `--pc-trace-from <frame>` to leave tracing disabled during a long tape's
prefix. Frames are positive, 1-based tape frames, and the start must be within
the tape. Watches must be distinct, even, 24-bit addresses, at most 64.

The CSV has `frame,pc,cycles`: `pc` is the watched instruction's address before
opcode fetch, and `cycles` is Genesis Plus GX's frame-relative **master-cycle**
counter, not elapsed host time or 68000 clock ticks. Preserve record order:
the counter may include instruction overshoot carried from the preceding
emulated frame. A difference within one frame measures elapsed emulated work
between those boundaries, including intervening stalls and interrupts; it
does not isolate decompression from them. Do not subtract counters across a
frame boundary without accounting for the core's frame length.

This is a read-only observation hook. It neither patches the cartridge nor
changes its cycle counter. Missing hooks, buffer overflow, unreadable records
and output failures make the host fail. `--cpu-trace` remains the separate
frame-end register/stack diagnostic. For field reload boundaries and the
timing model these observations support, see
[the reload note](../docs/scenes/FIELD_RELOAD.md#inputs-to-an-exact-timing-model).

### Video frame capture

The host can save exact, player-visible reference frames directly from the
libretro video callback:

```sh
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/07_first_battle.tape \
    --out /dev/null \
    --dump-frames 25000,31000 \
    --dump-frames-dir oracle/frames
```

`--dump-frames` is a comma-separated list of explicit, 1-based
`retro_run()` frame numbers. `--dump-frames-dir` is required with it; the
host creates that directory when it is absent and writes
`frame_<N>.png`. The list may contain at most 256 unique positive numbers.
The host fails if a requested frame is never emitted, rather than silently
producing a stale image.

The core negotiates `RETRO_PIXEL_FORMAT_RGB565` for this build. The sink also
decodes libretro's `0RGB1555` and `XRGB8888` software formats, converting all
three to dependency-free 8-bit RGB PNGs. `retro_get_system_av_info()` reports
the core's reset-time geometry as **256x192**, then Genesis Plus GX switches
the VDP to the retail **320x224** viewport on the first frame; the sink
ignores that one reset-mode callback and rejects any dumped callback that is
not exactly 320x224. The observed callback pitch is 1440 bytes, and no
overscan/max-width pixels are included.

The first checked-in reference set is:

| file | source | depicted moment |
|---|---|---|
| `oracle/frames/frame_25000.png` | tape 07 | battle command menu idle (`COMD` selected), two Zoran Bults and the three-party status bar |
| `oracle/frames/frame_31000.png` | tape 07 | post-battle field message while the text is still mid-draw (`Stop wasting time, there's not`) |
| `oracle/frames/frame_7000.png` | tape 02 | Piata field, Chaz facing the town interior and its NPCs |
| `oracle/frames/frame_7400.png` | tape 03 | NPC dialogue window with the first page fully visible |

`--groups` selects which RAM-map groups are logged, and on a long tape it is
the difference between a 2MB log and a 60MB one. Available groups, with their
field counts:

| group | n | group | n | group | n |
|---|---|---|---|---|---|
| `core` | 9 | `party` | 8 | `chars` | 87 |
| `input` | 4 | `flags` | 12 | `objects` | 384 |
| `pos` | 12 | `rng` | 5 | `bcmd` | 27 |
| `pos2` | 3 | `battle` | 29 | `text` | 24 |
| `collision` | 7 | `bhit` | 19 | `camera` | 18 |
| `window` | 16 | `enemy` | 56 | `flagbytes` | 52 |

Omitting `--groups` logs all 772 fields.

`--dump-ram <frame>:<path>` writes all 64KB of work RAM at one frame, in 68000
byte order, so the file offset of a byte is its address minus `$FFFF0000`.
Diffing two dumps is the tool for "where did that write go" when the RAM map
does not already have a column for it — which is exactly how the chest flag
bank was found after a mapped-column search returned a confident wrong answer.

`--dump-state <frame>:<path>` (up to 128 frame receipts per oracle run) writes
the named plane/CRAM/sprite buffers plus
the scroll receipt used by `oracle/decode_layout.py`: camera position and step
counters, H-int state, the generated H-scroll work buffer, the
`Chunk_Table`/VSRAM-shadow source, VDP registers, VSRAM, and the active VDP
H-scroll table and a raw `vdp_vram` 64 KiB receipt. The VDP words are emitted in Genesis big-endian order and the
state header names the address space and region byte order. The host resolves
the pinned Genesis Plus GX local VDP symbols from the loaded core ELF, so no
third-party core ABI or emulation behavior is modified.

For the MeetingRika receipt, add this flag to the Tape 28 command below:

```sh
--dump-ram 7250:/tmp/psiv-scroll-check.ram \
--dump-state 7250:/tmp/psiv-scroll-check.json
python3 -m oracle.decode_layout \
  /tmp/psiv-scroll-check.json --label meeting-rika-7250 \
  --output /tmp/psiv-scroll-check-layout.json --grand-cross 0
```

At frame 7250 the decoded receipt is: all camera and step words zero;
`HInt_Addr=0x00000758` (the retail RTE), H-int split disabled; VDP H-scroll
full-screen at `$F400` with zero Plane A/B columns; VSRAM Plane A/B zero; and
the generated `$FFFF60E0` H-scroll buffer zero. The `$FFFF6000` bytes are
reported as an inactive `Chunk_Table`/VSRAM-shadow source, not falsely called
live VSRAM. The window-region report is scanlines 160..223 and the placement
provenance records `grand_cross=0` plus the measured `(1,1)` plane residue.

### Deterministic scene fixtures

`--ram-patch <frame>:<68000-address>:<hex-bytes>` is an explicit oracle-fixture
operation. At the start of the named emulated frame it writes the bytes in
retail 68000 address order, validates that the address is inside the 64KB work
RAM and that the frame exists, and fails at the end if any requested patch was
not reached. It is not tape syntax and is not a general cheat interface.

Tape 28 uses this to enter the retail MeetingRika scene without pretending
that a power-on input tape naturally begins there:

```sh
oracle/bin/psiv_oracle \
    --core oracle/core/genesis_plus_gx_libretro.so \
    --rom  "Phantasy Star IV (USA).md" \
    --map  oracle/ram_map.tsv \
    --tape oracle/tapes/28_meeting_rika_retail_probe.tape \
    --out /dev/null \
    --ram-patch 7000:FFFFEC28:00AC \
    --ram-patch 7000:FFFFEC2A:0000 \
    --ram-patch 7000:FFFFEC4E:02 \
    --ram-patch 7000:FFFFEF00:0008 \
    --ram-patch 7000:FFFFF406:01F0 \
    --ram-patch 7000:FFFFF408:01A0 \
    --ram-patch 7000:FFFFF40A:00010203 \
    --ram-patch 7200:FFFFECA8:8007 \
    --ram-patch 7200:FFFFEF00:000C \
    --dump-frames 7200,7250,7300,7400,7600 \
    --dump-frames-dir /tmp/psiv-meeting-rika-oracle
```

The observed retail outputs include the MeetingRika dialogue frame at 7200
(partially typed `Professor! Thank goodness y...`) and frame 7250 (the
fully-typed `Professor! Thank goodness you’re safe!`). Those are genuine
libretro video callbacks from the pinned ROM/core fixture; they are the oracle
side of the clone RMSE pair, not clone screenshots. In the final replay,
`frame_7200.png` hashed to
`5e166d2c0d985d5e073ded24b3862b404dd0ac55501b96fb145c4a100411b3c2` and
`frame_7250.png` hashed to
`d8fc26ae6987e416ee75c02cd10ea4975e9be22485b8feeda84161aa489888c9`.

### Vehicle tapes 29 and 31

Tape 29 is a fixture for the mounted field and battle surface (RAM patches
install the Land Rover), and tape 31 its natural-input companion from power-on.
Their commands and what they show are in
[VEHICLES.md](../docs/field/VEHICLES.md#vehicle-tapes-29-and-31).
