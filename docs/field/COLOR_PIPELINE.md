# Color pipeline

## Decision

The pack widens Genesis CRAM levels with the ramp measured from the GPGX
oracle's RGB565 output:

| CRAM level | red | green | blue |
|---:|---:|---:|---:|
| 0 | 0 | 0 | 0 |
| 1 | 32 | 32 | 32 |
| 2 | 65 | 68 | 65 |
| 3 | 98 | 101 | 98 |
| 4 | 139 | 137 | 139 |
| 5 | 172 | 170 | 172 |
| 6 | 205 | 206 | 205 |
| 7 | 238 | 238 | 238 |

This is the normal CRAM palette path. It is deliberately neither the old
linear widening (`level * 255 / 7`) nor bit replication. In particular,
CRAM levels `(0, 1, 3)` render as `(0, 32, 98)`, and level 5 is `172` on red
and `170` on green, not `255`.

The single implementation choke point is `psiv_tools.gfx.palette_rgb`.
`decode_color` records the logical levels and calls that function; all pack
emitters that turn decoded CRAM into pixels must call `palette_rgb` as well.
The receipt-backed constants are `GPGX_RGB565_RAMP` in `psiv_tools/gfx.py`.

## Receipt derivation

The oracle state dump's `regions.cram` is the game-owned
`Palette_Table_Buffer` shadow: 64 big-endian CRAM words at `$FFFFFB00`, not a
guessed palette reconstructed from GPGX source. The paired PNG is from the
same oracle frame. `oracle.scroll_state.decode_cram_shadow` preserves every
raw word plus its `(r, g, b)` 0–7 levels, and `decode_scroll_state` includes it
under `cram_shadow`.

The receipts below are the evidence used to pin the table. Each listed word
is present in the state dump and the listed RGB triplet is an exact pixel in
the paired PNG.

| receipt pair | CRAM word | levels `(r,g,b)` | observed pixel | first matching PNG coordinate |
|---|---:|---:|---:|---:|
| `oracle/states/opening/frame_4000.json` + `oracle/frames/opening/frame_4000.png` | `0x000A` | `(5,0,0)` | `(172,0,0)` | `(236,108)` |
| same opening pair | `0x0020` | `(0,1,0)` | `(0,32,0)` | `(184,90)` |
| same opening pair | `0x0248` | `(4,2,1)` | `(139,68,32)` | `(240,99)` |
| same opening pair | `0x0488` | `(4,4,2)` | `(139,137,65)` | `(56,77)` |
| same opening pair | `0x0AA8` | `(4,5,5)` | `(139,170,172)` | `(0,40)` |
| same opening pair | `0x00CC` | `(6,6,0)` | `(205,206,0)` | `(74,72)` |
| same opening pair | `0x0EEE` | `(7,7,7)` | `(238,238,238)` | `(43,67)` |
| `oracle/states/camp_root_layout.json` + `oracle/frames/frame_7675.png` (`camp_root_idle`) | `0x0620` | `(0,1,3)` | `(0,32,98)` | `(208,8)` |
| same camp pair | `0x0246` | `(3,2,1)` | `(98,68,32)` | `(232,0)` |
| same camp pair | `0x068A` | `(5,4,3)` | `(172,137,98)` | `(2,0)` |
| same camp pair | `0x06AC` | `(6,5,3)` | `(205,170,98)` | `(1,0)` |
| same camp pair | `0x0CEE` | `(7,7,6)` | `(238,238,205)` | `(219,0)` |
| same camp pair | `0x0CC4` | `(2,6,6)` | `(65,206,205)` | `(1,16)` |
| `oracle/states/battle_command_idle.json` + `oracle/frames/frame_25000.png` | `0x0EA4` | `(2,5,7)` | `(65,170,238)` | `(96,98)` |
| same battle pair | `0x088C` | `(6,4,4)` | `(205,137,139)` | `(102,79)` |
| same battle pair | `0x062E` | `(7,1,3)` | `(238,32,98)` | `(36,50)` |

Together these receipts exercise every level on every channel, including the
six-bit green quantizer's distinct values at levels 2, 3, 4, 5 and 6. The
opening, camp and battle state dumps also carry the same raw CRAM format, so
the result is not tied to one scene's palette contents.

## Shadow/highlight handling

The target receipt pixels contain only the normal palette values in the table
above. No pixel in the opening, camp-root, battle-command, or title receipt
introduces a second shadow/highlight ramp. Shadow/highlight is therefore not
baked into pack palette widening: it remains a renderer-compositing mode, and
the pack's normal CRAM colors are widened exactly once through
`palette_rgb`. Adding a guessed shadow/highlight transform here would corrupt
ordinary palette entries and would not be supported by a receipt.

## Pack regeneration check

The installed `runtime-pack` was compared with the complete regenerated pack
at `/tmp/psiv-pack-clean-final.6eBvid` using:

```sh
python3 tools/pack_diff.py -q runtime-pack /tmp/psiv-pack-clean-final.6eBvid
```

The result was `Compared: 4315`, `Identical: 4315`, `Differing: 0`, `Only in
A: 0`, and `Only in B: 0`. The regenerated reference includes its manifest;
this is a full-pack comparison, not just a selected PNG or JSON subset.

## Reproducing the state-side receipt

For a fresh MeetingRika receipt, use the documented Grand Cross zero recipe
in `oracle/README.md` and `docs/scenes/SCENE_PRESENTATION.md`, including:

```sh
oracle/bin/psiv_oracle \
  --core oracle/core/genesis_plus_gx_libretro.so \
  --rom "Phantasy Star IV (USA).md" \
  --map oracle/ram_map.tsv \
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
  --dump-frames-dir /tmp/psiv-meeting-rika-oracle \
  --dump-ram 7250:/tmp/psiv-scroll-check.ram \
  --dump-state 7250:/tmp/psiv-scroll-check.json

PYTHONPATH=. python3 oracle/decode_layout.py \
  /tmp/psiv-scroll-check.json \
  --label meeting-rika-7250 \
  --output /tmp/psiv-scroll-check-layout.json \
  --grand-cross 0
```

The state dump and PNG frame must have the same oracle frame number. The
coordinates in the receipt table are the first exact RGB matches found by
scanning the paired 320x224 PNG; they are included so the CRAM word, frame,
and observed pixel can be rechecked without eyeballing a screenshot. The
`grand_cross=0` provenance is intentional: this workspace is the Grand Cross
build, but the color table is measured from the paired GPGX receipts rather
than inferred from that build's source.
