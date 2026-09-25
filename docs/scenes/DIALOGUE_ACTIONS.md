# Dialogue-embedded actions

Retail dialogue uses control byte `$F2` for an inline action. The extracted
dialogue JSON preserves the action id and raw operands; the audit view is
reproducible with:

```sh
python3 -m psiv_tools dialogue-census generated/dialogue.json --format markdown
```

The census below is from all **2,736** extracted entries. Locations are
`DialogueTree:entry`. A repeated action at one location is marked `Nx`.

## Retail timing and semantics

The disassembly settles the timing question. `RunText_CharacterLoop` reads a
byte at `reference/ps4disasm/ps4.asm:142564`; bytes at or above `$F0` jump
through `TextCtrlCodesJmpTbl`, and the normal character path waits for the
message-speed/input cadence before returning to the loop. The action handler
at `:142652` dispatches `TextActionsOffs` and branches straight back to the
character loop. An action therefore fires at its byte position, after the
preceding character's wait and before the next character. It is not deferred
to page end.

`TextActionsOffs` at `:143118` contains the retail Grand Cross=0 handlers
`$00..$0C`; the conditional `$0D/$0E` entries are Grand Cross-only additions
and do not occur in this corpus. The handler provenance is:

| ActionKind | Retail handler | Retail operation | Runtime path |
|---|---:|---|---|
| `LoadPanel` | `:143139` | Reads a big-endian word, calls `Panel_Create`, then `DMAPlanes_VInt`. | `DialogueWindow` action gate → `CutsceneLayer::panel_create` → `dma_planes`. The panel stack is the same presentation stack used by scene `Panel_Create`; the dialogue operand is widened to the retail word id. |
| `DestroyLastPanel` | `:143151` | Stack-pops the last panel, DMA if the stack was non-empty. | `CutsceneLayer::panel_destroy_last` → `dma_planes`. |
| `DestroyAllPanels` | `:143161` | Pops every panel, then DMA. | `CutsceneLayer::panel_destroy_all` → `dma_planes`. |
| `LoadSound` | `:143175` | Writes `Sound_Index` only when the byte is nonzero, then prepares VInt. | Existing `AudioOutput::play` path when nonzero. |
| `LoadSound2` | `:143185` | Always writes `Sound_Index`, then prepares VInt. | Existing `AudioOutput::play` path. |
| `UpdatePalette` | `:143195` | Reloads the live map palette and prepares VInt. | Cutscene layer redraw; panel textures carry the decoded retail palette. |
| `ZioEyesRed` | `:143203` | Writes Brose (`$CB`) and performs the synchronous red palette sequence. | Audio path plus the cutscene layer's red-palette overlay. |
| `PauseMusic` | `:143238` | Writes the retail pause command to `$FF5007`, then prepares VInt. | `AudioOutput::pause_music`; PCM filling is held until resume. |
| `ResumeMusic` | `:143245` | Writes the retail resume command to `$FF5007`, then prepares VInt. | `AudioOutput::resume_music`. |
| `SabotageAlarmRedPalette` | `:143252` | Plays Alarm (`$DB`) through three red/fade cycles. | Alarm through `AudioOutput`, red overlay through `CutsceneLayer`. |
| `SetEventFlag` | `:143275` | Calls `EventFlags_Set` immediately with the operand byte. | `Runtime::set_event_flag` writes `GameState`; the live `TextFlow` flag bank is updated before it resumes. |
| `ElsydeonBroken` | `:143348` | Runs the synchronous Elsydeon palette-break sequence. | Cutscene layer red-palette effect. |

`Panel_Create` itself is at `:144284`; `Panel_Destroy` is at `:144449`; and
the word-id lookup is `Panel_GetData` at `:144570`. `Panel_GetData` uses the
upper six bits to select one of seven pointer-table parts
(`PanelPtrs`, `:144586-:144593`) and the lower six bits to select the 30-byte
record. The presentation pack now decodes the non-empty retail banks at:

| Panel id range | Record base |
|---|---:|
| `$000..$03F` | `$07B000` |
| `$040..$07F` | `$09F940` |
| `$080..$0BF` | `$2B0000` |
| `$100..$13F` | `$2B9010` |
| `$140..$17F` | `$2D6750` |
| `$180..$1BF` | `$2EECD0` |

The `$0C0..$0FF` Part 4 range has no action-referenced records and is left
explicitly absent rather than decoded from padding.

The runtime action path is deliberately staged: `TextFlow` stops at one
action, `DialogueWindow` releases it only after the preceding visible glyphs
are revealed, `Field::service_dialogue_actions` applies it, then the flow
resumes. Consecutive actions drain back-to-back, matching the retail loop.

## Census totals

| id | ActionKind | occurrences | distinct entries |
|---:|---|---:|---:|
| `0` | `LoadPanel` | 165 | 48 |
| `1` | `DestroyLastPanel` | 35 | 16 |
| `2` | `DestroyAllPanels` | 7 | 5 |
| `3` | `LoadSound` | 35 | 17 |
| `4` | `LoadSound2` | 14 | 4 |
| `6` | `UpdatePalette` | 4 | 1 |
| `7` | `ZioEyesRed` | 1 | 1 |
| `8` | `PauseMusic` | 1 | 1 |
| `9` | `ResumeMusic` | 1 | 1 |
| `10` | `SabotageAlarmRedPalette` | 1 | 1 |
| `11` | `SetEventFlag` | 1 | 1 |
| `12` | `ElsydeonBroken` | 1 | 1 |

Total: **266** `$F2` actions in **55** distinct entries.

### LoadPanel payloads and locations

There are **163 distinct panel ids**. This table is the complete payload-to-entry
map; `0x186 (2x)` and `0x78 (2x)` are the only repeated panel payloads.

| Entry | Panel payloads |
|---|---|
| `DialogueTree13:65` | `0x50` |
| `DialogueTree13:66` | `0x3F`, `0x43`, `0x44`, `0x45`, `0x46` |
| `DialogueTree13:67` | `0x41`, `0x47`, `0x48`, `0x49` |
| `DialogueTree14:36` | `0x8C`, `0x8D` |
| `DialogueTree14:8` | `0x89`, `0x8A` |
| `DialogueTree20:47` | `0x100`, `0x101`, `0x102`, `0x103`, `0x104`, `0x105`, `0x106`, `0x9F` |
| `DialogueTree20:48` | `0x128` |
| `DialogueTree20:49` | `0x12A`, `0x137`, `0x138` |
| `DialogueTree20:50` | `0x13C` |
| `DialogueTree22:56` | `0x108`, `0x109`, `0x10B`, `0x10C` |
| `DialogueTree22:59` | `0x111`, `0x112`, `0x113`, `0x114`, `0x115` |
| `DialogueTree33:17` | `0x04`, `0x05` |
| `DialogueTree33:19` | `0x06`, `0x07`, `0x08`, `0x09` |
| `DialogueTree33:23` | `0x00`, `0x01`, `0x02`, `0x03` |
| `DialogueTree34:0` | `0x30`, `0x31`, `0x32` |
| `DialogueTree34:1` | `0x35`, `0x36`, `0x37`, `0x38` |
| `DialogueTree34:3` | `0x39`, `0x3A` |
| `DialogueTree34:4` | `0x3D` |
| `DialogueTree34:5` | `0x53`, `0x54`, `0x55` |
| `DialogueTree34:9` | `0x4E` |
| `DialogueTree35:1` | `0x77`, `0x78 (2x)`, `0x79` |
| `DialogueTree35:2` | `0x7B`, `0x7F` |
| `DialogueTree35:3` | `0x81`, `0x82`, `0x83`, `0x84`, `0x85` |
| `DialogueTree35:7` | `0x180`, `0x8F`, `0x90`, `0x91` |
| `DialogueTree35:8` | `0x94`, `0x95`, `0x96`, `0x97` |
| `DialogueTree36:0` | `0x4F`, `0x6E`, `0x6F`, `0x71`, `0x72` |
| `DialogueTree37:40` | `0x9A`, `0x9C`, `0x9D` |
| `DialogueTree39:0` | `0xA6` |
| `DialogueTree39:1` | `0xA7` |
| `DialogueTree39:23` | `0x117`, `0x186 (2x)` |
| `DialogueTree39:24` | `0x181` |
| `DialogueTree3:102` | `0x0A`, `0x0B`, `0x0C`, `0x0D`, `0x0E`, `0x0F` |
| `DialogueTree3:103` | `0x23`, `0x24`, `0x27`, `0x28`, `0x29`, `0x2A` |
| `DialogueTree3:104` | `0x2B` |
| `DialogueTree3:105` | `0x2C`, `0x2D`, `0x2E`, `0x2F` |
| `DialogueTree40:11` | `0x11C`, `0x11D`, `0x11E`, `0x11F`, `0x120`, `0x121`, `0x122`, `0x123`, `0x124` |
| `DialogueTree40:12` | `0x126`, `0x127` |
| `DialogueTree42:0` | `0x119`, `0x18D` |
| `DialogueTree42:1` | `0xA1`, `0xA2`, `0xA3` |
| `DialogueTree42:11` | `0x15B`, `0x15C`, `0x15E` |
| `DialogueTree42:12` | `0x172`, `0x174`, `0x176`, `0x178`, `0x17A`, `0x17B`, `0x17D`, `0x17F` |
| `DialogueTree42:9` | `0x141`, `0x142`, `0x143`, `0x144`, `0x145`, `0x146` |
| `DialogueTree6:45` | `0x4B`, `0x4C`, `0x4D` |
| `DialogueTree6:46` | `0x183`, `0x184`, `0x185`, `0x58`, `0x59`, `0x5C`, `0x68`, `0x6B` |
| `DialogueTree7:3` | `0x10`, `0x11`, `0x12`, `0x13`, `0x14` |
| `DialogueTree7:37` | `0x15`, `0x16`, `0x17`, `0x18` |
| `DialogueTree7:39` | `0x19` |
| `DialogueTree8:71` | `0x3E` |

### Sound payloads

`LoadSound` payloads: `$89` at `DialogueTree35:8`; `$92` at
`DialogueTree13:66, DialogueTree34:0`; `$94` at `DialogueTree34:6`; `$9E` at
`DialogueTree12:39, DialogueTree24:51, DialogueTree36:0`; `$9F` at
`DialogueTree13:72, DialogueTree35:8, DialogueTree3:102`; `$A8` at
`DialogueTree13:66`; `$A9` at `DialogueTree20:50, DialogueTree3:103,
DialogueTree43:4`; `$AD` six times at `DialogueTree42:9`; `$B9` three times at
`DialogueTree36:0`; `$E6` at `DialogueTree42:9`; `$ED` and `$F5` at
`DialogueTree22:56`; `$F4` at `DialogueTree7:37`; `$F8` twice at
`DialogueTree39:24`; `$FB` at `DialogueTree34:5`; `$FD` twice at
`DialogueTree39:24`; `$FE` at `DialogueTree13:66, DialogueTree3:103,
DialogueTree7:37`.

`LoadSound2` payloads: `$BD` at `DialogueTree13:66, DialogueTree13:67`; `$CC`
at `DialogueTree13:66`; `$CE` twice at `DialogueTree13:67`; `$DD` eight
times at `DialogueTree22:56`; `$F8` at `DialogueTree42:1`.

### Payload-free actions and flag payload

| ActionKind | Entries |
|---|---|
| `DestroyLastPanel` | `DialogueTree13:65`; `DialogueTree13:66` (2x); `DialogueTree33:17`; `DialogueTree33:19` (4x); `DialogueTree34:1` (3x); `DialogueTree34:9`; `DialogueTree35:1` (2x); `DialogueTree35:8` (3x); `DialogueTree36:0`; `DialogueTree39:23` (2x); `DialogueTree3:102`; `DialogueTree6:45` (3x); `DialogueTree6:46` (8x); `DialogueTree7:3`; `DialogueTree7:37`; `DialogueTree7:39` |
| `DestroyAllPanels` | `DialogueTree20:47` (2x); `DialogueTree22:59`; `DialogueTree35:3`; `DialogueTree35:7`; `DialogueTree40:11` (2x) |
| `UpdatePalette` | `DialogueTree33:19` (4x) |
| `ZioEyesRed` | `DialogueTree13:66` |
| `PauseMusic` | `DialogueTree14:6` |
| `ResumeMusic` | `DialogueTree14:6` |
| `SabotageAlarmRedPalette` | `DialogueTree35:2` |
| `SetEventFlag` | flag `$45` at `DialogueTree10:29` |
| `ElsydeonBroken` | `DialogueTree42:9` |

## Panel extraction result

`presentation_pack.py` emits **228** panel records: the original 15
scene-owned records, the **163** distinct `$F2 LoadPanel` ids above, and the
50 `$8021` ending-only records. Every action-referenced id has a decoded PNG
and a manifest record; `$30` is `presentation/panels/panel_30.png`, record
offset `$07B5A0`, decoded size 160x112. `tests/test_presentation_pack.py`
checks the banked offsets, manifest coverage, both Enigma planes, and the
known scene-panel geometry.
