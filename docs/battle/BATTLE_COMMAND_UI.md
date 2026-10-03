# Battle command windows, status panes and the fused MetaSlug

Decoded 2026-10-02 against the retail cartridge for issues #51, #52 and #64.
Every value cites a routine in `reference/ps4disasm/ps4.asm` (retail `loc_`
addresses, per [BATTLE_GEOMETRY.md](BATTLE_GEOMETRY.md)) and the oracle frame it
was read from. Five new pairs in `tools/certify.py` hold the result at
`rmse=0.000000`; the mid-round attack (#52) is certified only in part, see
[Mid-round attack](#mid-round-attack-52).

## The per-character command strip (#51)

`Battle_OpenCharComd` (`ps4.asm:2085`) opens a 16x4-cell window and writes five
2x2 icons into it; `Battle_CharCommand` (`ps4.asm:2192`) moves a sprite cursor.

| fact | value | source |
| --- | --- | --- |
| window | 16x4 cells at plane row 17, column 3, 15, 9, 9, 15 by `Battle_Total_Comd_Input` (fighter slot - 1) | `loc_17A4` `ps4.asm:2410`; plane dump of frame 25134 |
| icons | window-local columns 1, 4, 7, 10, 13 of row 1; ATTACK only when a hand holds a weapon, TECH/SKILL/ITEM only when their list is non-empty, DEFEND always | `ps4.asm:2108-2160` |
| icon words | `BattleTiles_*Icon` `$E15E..$E17A` (CRAM line 3, priority) | `ps4.asm:11258` |
| cursor | Left/Right through `Battle_UpdateCursor` (`ps4.asm:70646`), wrapping over five icons; Up/Down are not tested | `ps4.asm:2192-2212` |
| cursor sprite | 2x2 at x = first icon x + 24 * index, y 156, tile `$17C` then `$180`, six ticks each | `ps4.asm:70465`, `70494`; SAT of frames 25080-25160 |
| cursor lag | the VDP reads the sprite table a frame after the object writes it, so the frame on screen is the one the clock had a tick ago | video 25134 shows RAM state 25133 |
| other bodies | the plane clears the party rows as the window opens and draws only the acting body | `loc_E4C`, `loc_E64`, `loc_EB6` (`ps4.asm:1499-1503`) |

Cancel from a list window reopens the strip on the icon that opened it
(`Battle_BackFromTechs`, `ps4.asm:2777`) and clears that member's command byte.

## The technique, skill and item windows (#51)

`Battle_TechWindow` (`ps4.asm:2593`), `Battle_SkillWindow` (`ps4.asm:2933`) and
`Battle_ItemWindow` (`ps4.asm:3325`) show **four rows a page**.

- Window at plane row 12, columns 4, 16, 10, 10, 16 (`loc_19D0`, `loc_1D5E`,
  `loc_2154`); 12 (tech), 16 (skill) or 14 (item) cells wide, 9 high.
- Entry `k` is at row `13 + 2k`: cursor tile `$6E7`/`$6E8` at column 1, name at
  column 3, the number at columns 8-10 (tech cost) or 11-13 with window tile
  `$6FF` over the hundreds cell (skill uses). A disabled entry sets `$C000`
  (CRAM line 2) and ignores accept.
- `Battle_UpdateRedCursor2` (`ps4.asm:1572`): Up/Down wrap **inside the page**,
  whether or not the row holds an entry. Right flips to the next page only when
  the page's top-right corner holds the arrow word `$6EE6`, Left likewise with
  `$66E6` at the top-left (`ps4.asm:2603-2608`); a flip returns before the
  Cancel and accept tests.
- The red cursor blinks: `$FFFF41D2` counts down, `$FFFF41D4` is the phase;
  25 frames red, 15 blue, a held button or an accept press forces red
  (`ps4.asm:1590-1620`). The runtime keeps both words (`session/battle/blink.rs`).

Receipt: tape 32 (`oracle/tapes/32_battle_command_windows.tape`, frame 25134
the strip with the cursor on TECH, frame 25400 the technique window: SANER 6,
SHIFT 7, FOI 3).

### Not modelled, named

- Windows open over 6-8 frames (`Battle_CreateWindow`, `ps4.asm:1625`: two cells
  a frame); the port's windows appear whole. The red cursor's timers do not
  run during that growth in the cartridge.
- `Battle_Char_Comd_Index`, `Battle_Tech_Index` and `Battle_Skill_Index` are
  per-member words (`ps4.asm:2195`, `2598`, `2938`); the cartridge reopens
  those windows where that member left them, while the port starts at the
  first row. `Battle_Item_Index` is also per member during a round, but
  `loc_5BAE` (`ps4.asm:8422`) clears its five words at round end. The
  across-member/round cursor memory still belongs to [#51](https://github.com/TusanHomichi/PSIV/issues/51).
- The skill and item windows are decoded from their routines and drawn by the
  same code, but no oracle frame certifies them.
- Target selection is the cartridge's sprite cursor over the enemies or party
  (`Battle_PickTargetEnemy`, `ps4.asm:1200`); the port keeps its one-list page.

A sealed member **can** choose a technique, as in the cartridge: the window
refuses an entry on TP alone (`Battle_FillTechList` sets the sign bit,
`ps4.asm:1721`; `Battle_TechWindow` ignores it, `ps4.asm:2628`), and the cast
is paid and wasted (`CharTech_CheckTPCost` `ps4.asm:14211`, `CharTech_Cast`
`ps4.asm:14256`; the engine's `TechniqueRejected(Sealed)` after the paid
`TechniqueUsed`). `TechniqueEntry.available` stays false for a sealed member so
a policy avoids the wasted turn. Pad-only test:
`a_sealed_member_may_choose_a_technique_and_the_cast_is_wasted`.

## Status panes (#64)

`Battle_DrawCommandIcons` (`ps4.asm:11056`) runs every frame and decides each
pane's icon and ink from the member's status byte and command byte
(`Battle_Command_Data`, four bytes a member):

| status | icon | CRAM line of the pane's text |
| --- | --- | --- |
| `$44` dead | 8 | 2 |
| `$08`/`$20` asleep | 7 (paralysis `$02` outranks it with 6) | 1 |
| `$02` paralyzed | 6 | 1 |
| `$01` poisoned | the command icon | 0 |
| tech sealed `$10` | **nothing**: the routine never tests it | 3 |

The icon replaces the command byte, and a status icon whose status has gone
reads back as no command. Icons are always drawn on CRAM line 3; only the name,
HP/TP labels and numbers change ink: index 15 of line 0/1/2/3 is `$02CE`,
`$0CC4`, `$062E`, `$0EEE` (`loc_7654`, `ps4.asm:11205`). The command byte is 0
none, 1-5 attack, tech, skill, item, defend; 9 an empty seat. The runtime owns
the decision (`session/battle/panes.rs`); a round's end clears the table
(`loc_5BAE`, `ps4.asm:8419`).

Receipts, tape 32 at frame 25000 with `Character_Stats` status patched one
frame earlier: Chaz asleep `$08`, Alys paralyzed `$02`, Hahn sealed `$10`
(`battle-status`); Chaz poisoned `$01`, Alys dead `$04`, Hahn asleep and sealed
`$18` (`battle-status-2`).

## The fused MetaSlug (#64)

Fusion (`docs/battle/ENEMY_FUSION.md`) rebuilds the enemy side from
`loc_1A2F4`: one enemy, id 36, **position `$14`**. The runtime event now
carries the position and the battle view's `EnemyStatus` the enemy id and
position; the shell rebuilds slot 1 with the MetaSlug art, palette, overlay
clock and damage column exactly as a formation builds them. Receipt: tape 33
(`oracle/tapes/33_zol_fusion_metaslug.tape`, formation `$D2` forced with the
three patches in `tools/certify.py`'s `TAPE_33`), frame 25560, options back
over the MetaSlug. The party defends in the clone's round; the slugs' turn is
Fusion whatever it rolls. Not modelled: the two sprites sliding together and
flashing (`BattleObj_Fusion`, `ps4.asm:35675`).

## Mid-round attack (#52)

**Fixed.** The shell applied a frame's view before advancing the art clock, so
an attack animation a beat started advanced on its own start frame. The
cartridge creates the attack object in `Battle_UpdateFighters` and the object's
first run only draws frame 0 (`GameMode_Battle`, `ps4.asm:947-966`). Oracle
evidence, tape 34: the Zoran attack's sprites are on the sprite table for
exactly 16 frames (25368-25383), two frames per mapping (tiles `$276`, `$278`,
`$27B`, `$27B` flipped, ...). Godot now advances the clocks first and applies
the view second, so a cue is shown at its start, each frame lasts two ticks and
the layer is on screen 16 ticks. The runtime side is pinned by
`an_attack_animation_cue_rides_the_first_frame_of_its_beat`.

**Not certified.** `PSIV_DEBUG_BATTLE_WINDOW=top,timeline=attack` with
`PSIV_DEBUG_BATTLE_PHASE=418` at tick 146 against tape 34 frame 25377 reads
`rmse=20.254240` (63 cells). It is left out of `PAIRS`. Residual regions and
causes, found by this pair:

1. **Attack art is not the art on screen** (rows 15-20, columns 12-22). The
   Zoran attack sprites use tiles of the enemy's own art bank, which the idle
   overlay animation rewrites: `loc_1528A` (`ps4.asm:30119`) waits until the
   attacker's first overlay piece shows frame 2, then sets bit 3 of its
   `Enemy_Sprites` entry to freeze it, so the lightning is drawn from the
   frozen piece. `enemy_attacks.json` frames for enemy 10 were decoded from
   the static bank (a block of sparks; the oracle shows a starburst and a
   vertical streak), and the layer sits 24 px right of the oracle's sprites
   (oracle `(96,120)`, port `(120,104)` with y equal): `origin_pixels` is
   `[0,0]` where the canvas origin is the body origin minus 24.
2. **The attacker's first overlay piece freezes** (rows 13-14, columns 13-14).
   Oracle `Enemy_Sprites` entry 0 holds `(frame 2, timer 6)` from 25358 to 25395
   while its siblings run (RAM dumps in tape 34): windup 10 frames, the 16
   animation frames, then the target's damage wait (`loc_15360`,
   `ps4.asm:30180`).
3. **The target steps under the attacker**: `Enemy_Attack` writes the target's
   `fighter_x_pos` from the attacker's (`ps4.asm:19170-19180`); Alys is drawn
   about 40 px left of her seat during the strike.

Causes 1 and 2 need the extractor (`psiv_tools/battle_animation_*.py`, outside
this lane's write set) and a per-attack-routine choreography; cause 3 is
presentation of the target's `Character_Targeted` routine.

## The art comes from the pack

The icon and cursor tiles are the two Nemesis streams `loc_7382` loads
(`ps4.asm:10975`): 30 tiles at ROM `$27D860` to VRAM tile `$15E`, 8 at `$27DA70`
to `$17C`. `loc_7634` copies 15 words from ROM `$76A4` (`loc_76A4`,
`ps4.asm:11213-11250`) into CRAM line 3, indices 1-15. The extractor writes
both the CRAM-index rows and that palette into `battle/art/command_ui.json`,
with a manifest `command_ui` entry and `command_ui_tiles` census. `psiv-data`
requires all 38 VRAM tiles `$15E..$183` and the 15 valid palette words;
`rust/psiv-godot/src/battle/tiles.rs` draws from them. For a local native run,
set `PSIV_RUNTIME_PACK` to the absolute path of a real pack copy; `Field::ready`
uses that path for every loader. It checks the typed file while configuring
battle, then exits with a rebuild error if it is missing or corrupt. No
ROM-derived pixels or palette are
committed. `command_art_matches_the_oracle_vram` (Rust) and
`test_command_ui_art_matches_oracle_vram_and_cram` (Python) compare every tile
and palette word to `oracle/states/battle_command_idle_vdp_25000.json`;
`public_load_requires_every_vram_tile_and_the_palette` rejects incomplete
synthetic packs through the public loader.

## Pins

Clone tick `t` pairs with oracle frame `24800 + t` for the Academy fixtures
(`PSIV_DEBUG_BATTLE_WINDOW`): the battle starts at tick 30 with the overlay
clock seeded. The enemy's overlay clock is the cartridge's `Enemy_Sprites`
`(frame, timer)` words solved for the phase `E` (`E = 536` at 25377, `F -
24841` after the strip opens, one update skipped as it opens); a shot shows the
state after the tick's drive, the video frame the RAM of the frame before, so
`PSIV_DEBUG_BATTLE_PHASE = E(F) - 1 - (t - 29)`. The strip's cursor pins an
age (`age=7`), the technique window's blink pins `$FFFF41D2/4` (`blink=14/1`),
and the status pairs read `$FFFF41D2/4 = 17/0` at frame 25000 (`blink=3/1`).
