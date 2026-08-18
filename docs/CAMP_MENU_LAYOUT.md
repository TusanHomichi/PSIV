# Retail camp-menu layout

Scouted 2026-08-15 from the retail USA ROM with the headless oracle. The
implementation receipt was closed 2026-08-17. The capture starts from
Tape 08's deterministic new-game route, then follows the root menu into the
empty-inventory ITEM message, the STATE chooser, and the one-member STATUS
screen.

The screen-corrected geometry is in
[`oracle/layouts/camp_menu_camera_decode.json`](../oracle/layouts/camp_menu_camera_decode.json).
The lossless raw state dumps and the direct outputs of the unmodified decoder
are listed in the evidence table below.

## Frame and coordinate rules

| property | value |
|---|---|
| visible screen | **320 × 224 px**, 40 × 28 cells |
| cell | 8 × 8 px |
| plane buffers | 64 × 32 cells, Plane A `$FFFF8000`, Plane B `$FFFF9000` |
| Plane A | windows, menu text, portrait/stat tile maps |
| Plane B | field background underneath the menu |
| geometry convention | every rectangle below is the **outer border**, including its border cells |

The field camera is not at zero in these captures. The oracle log reports
`cam_x_fg_px=600` and `cam_y_fg_px=232` at every captured menu frame. The
plane-buffer cell offset is therefore `(11,29)`:

```text
screen_plane_a[x,y] = plane_a_buffer[(x + 11) mod 64, (y + 29) mod 32]
```

That wrap is the important detail. `oracle/decode_layout.py` is intentionally
unchanged and assumes that the raw buffer's top-left 40 × 28 cells are the
visible screen. Its JSON remains useful and lossless, but it reports raw
buffer coordinates and misses the camera-wrapped root window and ITEM
message. The companion JSON applies only this documented camera mapping; it
does not alter the oracle or decoder.

Window-group width and height bytes are one cell smaller than the rendered
outer rectangle. The retail routines add one before loading window tiles
(`reference/ps4disasm/ps4.asm:117204-117220`, `117482-117512`). The group
records and child-window routing are at `ps4.asm:140181-140202`,
`140338-140342`, `140770-140798`, and `117911-117919`.

## Tape route and captured frames

[`oracle/tapes/22_camp_menu_layout.tape`](../oracle/tapes/22_camp_menu_layout.tape)
reuses Tape 08's prelude and then sends edge-triggered directional presses with
release frames between them. The input mapping is the retail mapping in
`oracle/README.md`: C/Speak and A/Camp confirm, B/Cancel backs out, and Start
exits the empty ITEM message.

| screen | mark | frame | retail state |
|---|---|---:|---|
| root | `camp_root_idle` | **7675** | ITEM selected, one party member |
| ITEM | `camp_item_idle` | **7797** | empty inventory message fully drawn |
| root after ITEM | `root_before_state` | **7923** | root reopened, ITEM selected |
| STATE chooser | `camp_state_idle` | **8054** | STATUS/ORDER chooser open, STATE retained underneath |
| STATUS | `camp_stats_idle` | **8176** | Chaz status page, one party member |

The authoritative frame marks and camera fields are in
[`oracle/logs/camp_menu_layout.csv`](../oracle/logs/camp_menu_layout.csv).

## Root menu

### Windows

| window | screen cells `(x,y,w,h)` | pixels `(x,y,w,h)` | retail source |
|---|---:|---:|---|
| root options | `(4,2,9,15)` | `(32,16,72,120)` | `WinGroup_Menu[0]`, `$08,$0E,$04,$02` |
| Chaz summary | `(26,1,12,6)` | `(208,8,96,48)` | `WinGroup_Menu[6]`, one-member `CalcWinIDAndCharNum` result |
| meseta | `(3,23,13,3)` | `(24,184,104,24)` | `WinGroup_Menu[1]`, `$0C,$02,$03,$17` |

Evidence: `oracle/states/camp_root_layout.json`,
`oracle/frames/frame_7675.png`, and
`oracle/layouts/camp_menu_camera_decode.json` (`camp_root`). The direct
decoder output is `oracle/layouts/camp_root.json`; because the root window
wraps through the plane buffer it detects only the meseta window and `MST` at
raw buffer `(23,21)`.

### Text runs

Positions are the first cell of each run; pixel positions are simply cell × 8.

| text | cell | pixel |
|---|---:|---:|
| `Chaz` | `(27,2)` | `(216,16)` |
| `LV  : 1` | `(33,2)` | `(264,16)` |
| `HP: 25/ 25` | `(27,4)` | `(216,32)` |
| `TP: 10/ 10` | `(27,5)` | `(216,40)` |
| `ITEM` | `(7,3)` | `(56,24)` |
| `TECH` | `(7,5)` | `(56,40)` |
| `SKILL` | `(7,7)` | `(56,56)` |
| `EQUIP` | `(7,9)` | `(56,72)` |
| `STATE` | `(7,11)` | `(56,88)` |
| `MUMBL` | `(7,13)` | `(56,104)` |
| `MACRO` | `(7,15)` | `(56,120)` |
| `500 MST` | `(8,24)` | `(64,192)` |

The menu option strings and the seven-option cursor loop are retail data/code,
not OCR guesses: `Win_MenuOptions` and `MenuOptionsOffs` are at
`ps4.asm:117201-117318`; the single-party `MUMBL` substitution is at
`117216-117219`.

### Cursor and party summary

At root idle the selected ITEM cursor is one 8 × 8 SAT cell at pixel `(40,24)`
(cell `(5,3)`). It uses pattern `0x6E8`, palette line 2, priority set. The
direct decoder preserves it as SAT entry 0 in `oracle/layouts/camp_root.json`:
`raw=00980001C6E800A8`, `screen_x=40`, `screen_y=24`.

Retail also emits a hollow selector cell before **every** root entry. The
seven cells are pattern `0x6E7`, palette line 2, at
`(5,3),(5,5),...,(5,15)`; the selected red `0x6E8` cell overlays the active
one. The same `0x6E7` form is used by the STATE, item, target, and save-slot
child lists. A clone that draws only the active cursor is visibly wrong even
when its selected row is correct.

The summary's `HP`/`TP` values are a split charset case. The label and colon
use ordinary menu text, but current/max digits use source font cells 36..45
(`DecimalVRAMOffset2`, patterns `$7E4..$7ED`). The separator is raw window
byte `$77`, loaded from the window strip as pattern `0x6F7`; it is not the
menu-font glyph `?`.

The summary contains one party member:

```text
    Chaz   LV 1   HP 25/ 25   TP 10/ 10
```

The numeric cells use the retail decimal tile patterns, not just the `$680`
window font. That distinction matters for a future renderer.

The camera-corrected raw Plane A words for the summary's row 2, screen
columns 26..37 are:

```text
C6F3 C683 C6C0 C6B9 C6D2 C680 C680 C68C C696 C680 C7DB CEF3
```

The right border is column 37. `LV` is at `(33,2)`, the ordinary menu-font
glyphs are the `$68C/$696` words, the blank at `(35,2)` is `$680`, and the
level value at `(36,2)` uses the decimal run's `$7DB` tile. A renderer must
compose those runs separately; drawing the semantic string `LV  : 1` with
ordinary glyphs writes into the border and selects the wrong numeric tile
family.

The same split is used by the STATUS character-info line at its decoded
`STATUS_TEXT[2]` anchor. The HP/TP labels are also window-charset words:
`HP:` is `$6F8/$6F9/$6B4` and `TP:` is `$6FA/$6F9/$6B4`, followed by a
`$680` blank. Their values use the second numeric run and the separator uses
the window-charset slash `$6F7`.

### Field sprites visible under the root window

Tape 22's fresh SAT walk gives the visible party/NPC anchors independently of
the Plane A menu. Chaz is `(152,86)`, 16×32, tile `$534`, attribute `$08`;
the visible left NPC is `(-8,6)`, tile `$3E7`; and the visible top NPC is
`(136,-8)`, tile `$287`, attribute `$0C`. The complete VDP/SAT dump is
`oracle/states/camp_root_idle_vdp_7675.json`.

The debug map sets those objects in runtime pixel space, so the view layer
must initialize each NPC node from the live cell plus sub-cell offset rather
than the packed map record's original spawn. The frozen top type-2 object is
on walk-down frame 1 in this receipt. The camp field-party anchor likewise
uses the live pixel position without the ordinary field renderer's one-cell
standing subtraction; otherwise Chaz lands 16 pixels above the SAT.

## ITEM: empty inventory message

Tape 22 deliberately captures the retail no-inventory branch. The inventory
slots `inv0..inv7` are all zero in the captured log; the retail code counts
all 40 inventory bytes and selects `Win_MenuItemMessage` when the count is
zero (`ps4.asm:117321-117358`).

### Geometry and text

The root and Chaz summary remain visible. The child message window is:

| window | screen cells | pixels | retail source |
|---|---:|---:|---|
| root options | `(4,2,9,15)` | `(32,16,72,120)` | `WinGroup_Menu[0]` |
| Chaz summary | `(26,1,12,6)` | `(208,8,96,48)` | `WinGroup_Menu[6]` |
| item message | `(7,21,26,5)` | `(56,168,208,40)` | `WinGroup_Menu[$35]`, `$19,$04,$07,$15` |

The complete run is `Can't have any items!` at cell `(8,22)`, pixel
`(64,176)`. The source string is `loc_2AA0E6` at
`ps4.asm:340398-340400`; the visual proof is
[`oracle/frames/frame_7797.png`](../oracle/frames/frame_7797.png).

There is no item-list cursor because there is no item list. The root selection
cursor remains visible as a one-cell plane cursor at `(5,3)` / `(40,24)`;
the camera-corrected Plane A word is `0xC6E8`. The direct decoder's empty
text result in `oracle/layouts/camp_item.json` is a known camera-origin gap,
not evidence that the retail message is absent.

Start takes the retail `Win_MenuItemMessage` exit branch
(`ps4.asm:122780-122814`). The tape then reopens the root and captures the
same root geometry at frame 7923.

## STATE chooser

The root STATE option is selected by four edge-triggered Down presses. C/Speak
opens the child chooser. The child group is `$C4`:
`$09,$04,$02,$05`, which renders as a 10 × 5 outer window.

### Geometry and text

| window | screen cells | pixels | retail source |
|---|---:|---:|---|
| root options | `(4,2,9,15)` | `(32,16,72,120)` | `WinGroup_Menu[0]` |
| Chaz summary | `(26,1,12,6)` | `(208,8,96,48)` | `WinGroup_Menu[6]` |
| STATE child | `(2,5,10,5)` | `(16,40,80,40)` | `WinGroup_Menu[$C4]` |
| meseta | `(3,23,13,3)` | `(24,184,104,24)` | `WinGroup_Menu[1]` |

| text | cell | pixel |
|---|---:|---:|
| `STATUS` | `(5,6)` | `(40,48)` |
| `ORDER` | `(5,8)` | `(40,64)` |
| retained root `STATE` | `(7,11)` | `(56,88)` |
| `500 MST` | `(8,24)` | `(64,192)` |

The child cursor is a hollow one-cell cursor at cell `(3,6)` / pixel
`(24,48)`, pattern `0x6E7`, palette line 2. The selected root STATE cursor is
the red one-cell `0x6E8` plane cursor at `(5,11)` / `(40,88)`. Both are
visible in [`oracle/frames/frame_8054.png`](../oracle/frames/frame_8054.png)
and preserved in `oracle/states/camp_state_layout.json`.

The direct decoder sees this child window at raw buffer `(13,2,10,5)` and
finds `STATUS`, `ORDER`, and `MST`; the camera companion gives their actual
screen positions above. Its official self-check was run with all three
expected strings.

## STATUS screen

Selecting STATUS with one party member skips the character-choice list and
opens the status page. The status routine creates groups `$C9` through `$CD`
(`ps4.asm:124499-124710`), then loads the portrait and the text/stat tiles.

### Windows and portrait

| window | screen cells | pixels | retail source |
|---|---:|---:|---|
| portrait frame/tile block | `(3,2,10,10)` | `(24,16,80,80)` | group `$C9`; portrait `PlaneMapToRAM` at `ps4.asm:124507-124529` |
| character info | `(13,2,12,12)` | `(104,16,96,96)` | group `$CA` |
| combat stats | `(25,2,13,13)` | `(200,16,104,104)` | group `$CB` |
| equipment | `(3,14,12,9)` | `(24,112,96,72)` | group `$CC` |
| EXP/NX | `(25,21,12,5)` | `(200,168,96,40)` | group `$CD` |
| meseta | `(3,23,13,3)` | `(24,184,104,24)` | group `[1]`, retained |

The portrait is a 10 × 10 Plane A tile block, not a SAT sprite. This Chaz
capture uses the `0x580`-family portrait patterns and occupies exactly the
`(3,2)`–`(12,11)` cell block. There is no visible selection cursor on this
screen.

### Text and values

| text/value | cell | pixel |
|---|---:|---:|
| `Chaz` | `(15,3)` | `(120,24)` |
| `HUNTER` | `(15,5)` | `(120,40)` |
| `LV  : 1` | `(15,7)` | `(120,56)` |
| `AGE : 16` | `(15,9)` | `(120,72)` |
| `HP: 25/ 25` | `(14,11)` | `(112,88)` |
| `TP: 10/ 10` | `(14,12)` | `(112,96)` |
| `STRNGTH: 8` | `(26,3)` | `(208,24)` |
| `MENTAL : 6` | `(26,5)` | `(208,40)` |
| `AGILITY: 7` | `(26,7)` | `(208,56)` |
| `DEXTRTY: 5` | `(26,9)` | `(208,72)` |
| `ATK POW: 18` | `(26,11)` | `(208,88)` |
| `DFS POW: 10` | `(26,13)` | `(208,104)` |
| `LTHR-HELM` | `(4,15)` | `(32,120)` |
| `HUNT-KNIFE` | `(4,17)` | `(32,136)` |
| `HUNT-KNIFE` | `(4,19)` | `(32,152)` |
| `LTHR-CLOTH` | `(4,21)` | `(32,168)` |
| `EX: 0` | `(26,22)` | `(208,176)` |
| `NX: 21` | `(26,24)` | `(208,192)` |
| `500 MST` | `(8,24)` | `(64,192)` |

The exact retail label spellings are `STRNGTH` and `DEXTRTY`, not corrected
English spellings. The source strings are at `ps4.asm:340630-340662`.
The numeric values are inserted with special decimal tile patterns; the
`$680` text decoder therefore sees the labels and equipment names but not
every number. The complete raw Plane A matrix is in
`oracle/states/camp_stats_layout.json`, and the visual proof is
[`oracle/frames/frame_8176.png`](../oracle/frames/frame_8176.png).

One retail quirk is part of the evidence: `MUMBL` remains visible at cell
`(7,13)` / pixel `(56,104)` above the equipment window after the root menu is
replaced. The companion JSON records it as retained underlying text rather
than silently deleting it from the observed frame.

## Decoder/self-check results

All five direct outputs were regenerated with the existing decoder. No source
under `oracle/host/` or `oracle/decode_layout.py` was edited.

| output | frame | official expected strings | self-check |
|---|---:|---|---|
| `oracle/layouts/camp_root.json` | 7675 | `MST` | **pass** |
| `oracle/layouts/camp_item.json` | 7797 | camera gap; companion checks `Can't have any items!` | **pass** |
| `oracle/layouts/camp_root_after_item.json` | 7923 | `MST` | **pass** |
| `oracle/layouts/camp_state.json` | 8054 | `STATUS`, `ORDER`, `MST` | **pass** |
| `oracle/layouts/camp_stats.json` | 8176 | `LTHR-HELM`, `HUNT-KNIFE`, `LTHR-CLOTH`, `MST` | **pass** |

The direct outputs' `self_check.expected_texts` are non-empty wherever the
raw decoder can see the relevant window. The camera companion's self-check
contains the complete screen strings, including the root labels and ITEM
message, so the result does not rely on a vacuous empty expected-text list.

## Gaps left by this prep slice

- The tape has one party member and zero inventory. Multi-party root/status
  heights, an actual item-list page, and inventory cursor placement are not
  decoded here.
- TECH, SKILL, EQUIP, TALK, MACRO, and the ORDER branch are not opened by this
  tape. STATE and STATUS are the two submenu families captured in full, with
  the empty ITEM message as the third child surface.
- The existing decoder has no camera-origin parameter and only a battle-shaped
  rectangle catalogue. That is reported and worked around in the companion
  JSON, not hidden by changing the tool.
- Decimal/stat values and the portrait use non-window tile data. Their exact
  positions and raw pattern families are recorded above, but the generic
  `$680` text-run self-check cannot validate those numeric glyphs as ordinary
  strings.

## State-matched certification fixture

Tape 22 mark `camp_root_idle` is frame **7675**. Its receipt is:

| field | value |
|---|---:|
| map index | `$13` |
| world / map-index-2 | `0 / $FFFF` |
| player position | `($2F0,$140)` pixels, cell `(47,20)` |
| settled camera | `($258,$E8)` pixels |
| party | Chaz only |
| Chaz HP/TP | `25/25`, `10/10` |
| money | `500 MST` |

`PSIV_DEBUG_CAMP=1` now boots that in-memory retail save. The required clone
pair is tick **60** against `oracle/frames/frame_7675.png`; the capture tick
and state are deterministic, but the post-fix RMSE remains unmeasured until
the Xvfb listener is available.

This is enough geometry and state to certify the camp root without guessing at
the retail rectangles, text origins, cursor variants, or single-member status
composition.
