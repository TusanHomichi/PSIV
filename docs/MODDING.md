# Modding PSIV

[Project overview](../README.md) · [Documentation index](README.md) ·
[Player's guide](PLAYING.md)

PSIV's game data is not compiled in (its rules and event scripts are). The game
reads the data at start-up from a **data pack**, a directory of JSON tables, PNG
images and sound data that `psiv-pack` builds from your US retail ROM.

You can mod PSIV without touching the base pack by dropping **shareable mod
folders** into your `mods/` directory — or by editing a copy of `runtime-pack/`
and running `psiv-pack mod diff` to turn your edits into a compact delta mod
folder.

## Quick start: shareable mod folders

A mod is a folder carrying **only your edits and original assets**, never base
pack files:

```text
mods/
  my-mod/
    mod.json                      # name, id, version, order, enabled
    patches/
      battle/enemies.json         # sparse ID-keyed JSON patches
      dialogue/trees.json         # dialogue edits (or shorthand replace/lines)
    assets/
      maps/010_Piata.png          # optional replacement PNG/audio assets
```

### Where the game looks for mods

1. **`PSIV_MODS`** (if set):
   - Set `PSIV_MODS=none` (or `0`) to run the unmodified base pack with all mods
     disabled.
   - Set `PSIV_MODS=/path/to/mod-a:/path/to/mod-b` (use `;` on Windows) to load
     explicit mod folders in that exact order.
2. Otherwise, the install's `mods/` directory (independent of `PSIV_RUNTIME_PACK`,
   so pointing `PSIV_RUNTIME_PACK` at an alternate or read-only base pack still
   uses your writable `mods/` folder):
   - **Installed release build:** `<user data folder>/mods/` (see
     [where your pack and saves live](PLAYING.md#where-your-pack-and-saves-live)).
   - **Source checkout:** `mods/` at the repository root.

Every enabled mod in `mods/` is sorted by `(order, id)` (lower `order` loads
first; later mods override earlier ones) and layered over the base pack into a
hidden staging directory inside `mods/` without ever modifying `runtime-pack/`
or `saves/`. Rebuilding the base pack from your ROM never touches `mods/`.

> **Automatic save isolation:** Modded runs never touch your vanilla
> `saves/slot_1..3.sram`. Whenever one or more mods are active (and
> `PSIV_SAVE_DIR` is not set), the game automatically saves into
> `saves/mods/<profile>/`, where `<profile>` is derived from the active mod
> IDs in load order (not their version numbers, so updating a mod keeps your
> progress). A small `mod_profile.json` sidecar is written beside the modded
> slots without altering `.sram` bytes — so you can still copy a vanilla
> `slot_1.sram` into a mod's save folder to test a mod mid-campaign, or set
> `PSIV_SAVE_DIR` to point anywhere you want. When active mods differ from the
> save folder's `mod_profile.json` (added, removed, reordered, or version
> changed), the title screen surfaces an in-game notice.
>
> **In-game Mods indicator and manager:** Whenever one or more mods are active
> (or discovered under `mods/`), the title screen displays active mods and a
> **Mods** button (`M`) to inspect load order, view status, and toggle
> `"enabled"` in `mod.json` (read-only when `PSIV_MODS` specifies explicit
> paths). Vanilla runs with no mods installed keep the retail title screen
> unchanged.

## Modding with an AI assistant

If you point an AI assistant or coding agent at this repo (or at an installed
release folder) to create a mod:

1. **Never distribute or commit base pack files.** Create a mod folder with
   `psiv-pack mod init` (ships inside every release archive beside the game
   binary, or run via `cargo run --release --manifest-path rust/Cargo.toml -p psiv-pack -- mod ...`).
2. **Patch tables by stable ID under `patches/`** (see the table key reference
   below) and place original replacement art/audio under `assets/`.
3. **Validate before launching** with `psiv-pack mod check <base-pack> <mod-dir>`.
   It runs the exact same fail-closed schema checks the game runs on startup and
   names the mod, file, and field if anything does not fit.
4. **Drop the validated mod folder into `mods/`** (or test directly with
   `PSIV_MODS=/path/to/mod`).

## `mod.json` manifest

```json
{
  "format_version": 1,
  "id": "hard-mode",
  "name": "Hard Mode",
  "version": "1.0.0",
  "author": "Your Name",
  "description": "Raises early enemy HP and tweaks Hahn's BioPlant greeting",
  "pack_format_version": 2,
  "order": 100,
  "enabled": true
}
```

- `id`: 1–64 lowercase ASCII letters, digits, `-` or `_`.
- `order`: optional integer (default `100`); lower loads first, ties break by `id`.
- `enabled`: optional boolean (default `true`); set `false` to disable without
  deleting the folder.

## CLI tools (`psiv-pack mod`)

Every release archive includes the `psiv-pack` executable alongside the game
(`psiv-pack`, `psiv-pack.exe`, or `PSIV.app/Contents/MacOS/psiv-pack`). In a
source checkout, replace `psiv-pack` with
`cargo run --release --manifest-path rust/Cargo.toml -p psiv-pack --`.

| Command | What it does |
| --- | --- |
| `psiv-pack mod init <out-dir> --id <id> [--name <name>] [--version <ver>]` | Scaffolds a mod folder (`mod.json`, `patches/`, `assets/`). |
| `psiv-pack mod check <base-pack> <mod-dir>...` | Layers one or more mods over `<base-pack>`, runs full schema validation, and reports shareability notes/warnings (without failing original hand-drawn assets or private local mods). |
| `psiv-pack mod diff <base-pack> <edited-pack> <out-dir> [--id <id>] [--name <name>] [--version <ver>] [--allow-derived]` | Diffs an edited pack copy against `<base-pack>` into sparse JSON patches and brand-new assets; refuses whole unkeyed base arrays and modified base-path binary assets unless `--allow-derived` is passed. |

## What each file holds and its patch key

The game reads pack data through `psiv-data` (`rust/psiv-data/src/`), and the
Rust types there are the schema: a field's name, type and range are what the
loader accepts. Under `patches/<file>`:

- **ID-keyed record arrays** accept either an ID-keyed object
  (`{"10": {"hp": 99}}`) or an array of partial objects carrying that key
  (`[{"id": 10, "hp": 99}]`).
- **2D tile/cell grids** (`collision.rows`, `vehicle_battle.rows`,
  `layout_variants.*.collision.rows`, `layout_variants.*.vehicle_battle.rows`,
  and `battle/formations.json` `position_grids.*.cells`) accept a sparse cell
  patch `{"cells": [[row, col, value], ...]}` (or explicit objects
  `{"row": r, "col": c, "value": v}`), validating bounds, types, value ranges,
  duplicate targets, and unknown fields fail-closed.

| To change | Pack file | Keyed arrays / sparse grids | Schema |
| --- | --- | --- | --- |
| Dialogue and text windows | `dialogue/trees.json`, `dialogue/portraits.json`, `dialogue/window.json` | `trees` → `tree`, `entries` → `id`, `talk_options.entries` → `entry`, `system_messages.messages` → `id`, `portraits` → `id`, `windows` / `menu_windows` → `index` | `rust/psiv-data/src/dialogue/trees.rs`, `dialogue/chrome.rs` |
| Enemies: HP, stats, resistances, AI, rewards | `battle/enemies.json`, `battle/enemy_animations.json` | `enemies` → `id`, `animations` → `enemy_id` | `rust/psiv-data/src/battle/enemies.rs`, `battle/animations.rs` |
| Battle formations and map encounters | `battle/formations.json` | `formations` → `id`, `boss_formations` → `event_battle_index`, `inline_formations` → `label`, `*.enemies` → `slot`, `encounter_groups.groups` → `group`, `map_bindings` → `map_id`, `position_grids` → `name` (`cells` sparse grid) | `rust/psiv-data/src/battle/formations.rs` |
| Items and equipment | `battle/equipment.json` | `items` → `id`, `types` → `name` | `rust/psiv-data/src/battle/party.rs` |
| Party members | `battle/characters.json` | `characters` → `character_id` | `rust/psiv-data/src/battle/party.rs` |
| Techniques, skills, enemy skills, combos | `battle/abilities.json` | `techniques`, `skills`, `enemy_skills`, `item_effects`, `combos` → `id` | `rust/psiv-data/src/battle/abilities.rs` |
| Level-up tables | `battle/levels.json` | `characters` → `character_id`, `levels` → `level` | `rust/psiv-data/src/battle/levels.rs` |
| Shops and inns | `shops.json` | `inventories` → `index`, `inns` → `index`, `counters` → `id`, `portraits` → `png` | `rust/psiv-data/src/shops.rs` |
| Maps: collision, warps, NPCs, treasures, updates | `maps/<id>_<symbol>.json` (art: `.png`, `_over.png`) | `npcs`, `warps`, `treasure_chests`, `interaction_areas`, `map_updates`, `patch_tiles.tiles` → `index`; `layout_variants` → `png`; `overworld_patches` → `event_flag`; `collision.rows` & `vehicle_battle.rows` sparse cell grids | `rust/psiv-data/src/map.rs` |
| Travel, title, ship menu, panels | `travel.json`, `title/sound_test.json`, `ship_menu/ship_menu.json`, `presentation/panels.json` | `towns` → `town_flag`, `dungeons` → `index`, `world_maps` → `name`, `tracks` → `index`, `windows` / `place_windows` → `id`, `panels` → `id` | `rust/psiv-data/src/travel.rs`, `sound_test.rs`, `ship_menu.rs`, `presentation.rs` |
| New-game starting state | `game_start.json` | recursive JSON merge | `rust/psiv-data/src/new_game.rs` |
| Sprites, battle art & sound | `sprites/`, `battle/art/`, `sound/` | `sheets` → `id`, `backgrounds` / `frames` → `index`, `tiles` → `vram_tile`, music/sfx `records` → `id` (`tracks` → `track_id`, `voices` → `index`), plus original assets in `assets/` | `rust/psiv-data/src/sprites.rs`, `sound/schema.rs` |

## Example 1: change an enemy's stats

The first random battle of the game, in the Academy basement, is two Zoran
Bults (enemy `10`, formation `$8A`) with 25 HP each. Scaffold a mod folder with
`psiv-pack mod init` and create `patches/battle/enemies.json` inside it:

```json
{
  "enemies": {
    "10": {
      "hp": 99
    }
  }
}
```

Validate the mod against your base pack and launch the game (example paths below
use Linux's `~/.local/share/PSIV`; substitute your platform's
[user data folder](PLAYING.md#where-your-pack-and-saves-live) on Windows or
macOS):

```bash
./psiv-pack mod init ~/.local/share/PSIV/mods/tough-zoran --id tough-zoran --name "Tough Zoran"
# ... write ~/.local/share/PSIV/mods/tough-zoran/patches/battle/enemies.json ...
./psiv-pack mod check ~/.local/share/PSIV/runtime-pack ~/.local/share/PSIV/mods/tough-zoran
./psiv.x86_64
```

Any enabled mod inside your `mods/` folder loads automatically on launch, or you
can test a single folder anywhere on disk with
`PSIV_MODS=/path/to/tough-zoran ./psiv.x86_64`. Leave `PSIV_SAVE_DIR` unset so
automatic per-mod save isolation keeps your vanilla saves untouched.

## Example 2: change a line of dialogue

Hahn's first line when he finds the professor in the BioPlant is tree `34`,
entry `0` in `dialogue/trees.json`. Dialogue patches support two shorthands
that update `segments`, `pages`, and `text` in lockstep automatically:

- **`"replace": [["old", "new"], ...]`** — replaces matching text while keeping
  every portrait, newline, and control code intact:

```json
{
  "trees": {
    "34": {
      "entries": {
        "0": {
          "replace": [
            ["Thank goodness you're safe!", "Glad to see you in one piece!"]
          ]
        }
      }
    }
  }
}
```

- **`"lines": ["Line 1", "Line 2"]`** (or nested pages `[["Page 1 L1", "L2"], ["Page 2"]]`) —
  replaces the entry's text cleanly (keeping any leading portrait unless
  `"keep_portrait": false`).

Dialogue is checked for lines that fit the window (at most 32 characters, two
lines a window) and characters present in `dialogue/font.json`.

## Example 3: edit a map's collision grid

To change individual 16×16 collision cells in `maps/010_Piata.json` without
embedding the retail 64×64 grid, write a sparse cell patch in
`patches/maps/010_Piata.json`:

```json
{
  "collision": {
    "rows": {
      "cells": [
        [1, 1, 8]
      ]
    }
  }
}
```

Each entry is `[row, col, value]` (or `{"row": 1, "col": 1, "value": 8}`),
where `row` is `y` (`0..height_cells`), `col` is `x` (`0..width_cells`), and
`value` is a 4-bit retail collision code (`0` normal walkable, `1` map change,
`2` recovery, `8` solid wall, `9` water, `10` sand, `11` ice, `12` shop).
Out-of-bounds coordinates, invalid codes (`> 15`), duplicate `(row, col)`
targets, and unknown fields are rejected fail-closed.

## Replacing map and sprite art

Place replacement PNGs under `assets/` at the same pack-relative path (for
example `assets/maps/010_Piata.png`, `assets/maps/010_Piata_over.png`, or
`assets/sprites/<sheet>.png`):

- **Baked RGB/RGBA PNGs (easiest):** Ordinary RGB or RGBA PNGs work everywhere,
  including on live-palette maps such as `010_Piata.png`. When a mod replaces a
  map `.png` without also supplying its `_indices.png`, the loader drops the
  base pack's `_indices.png` redirect so the renderer draws your PNG directly.
  Baked PNGs keep their exact colors and do not shift during cartridge palette
  cycles, flashes, or fades.
- **Live-palette indexed PNGs (full CRAM effects):** To keep cartridge palette
  animations, flashes, and fades active on a modded map or sprite, save an 8-bit
  indexed PNG (PNG color type 3) whose palette indices match the cartridge's
  CRAM slots (0–31 for map lines 0–1; 0–15 for a sprite sheet's CRAM line). On
  maps that ship a `<name>_indices.png` file, you can either supply an indexed
  `assets/maps/<name>.png` (which is rewritten to indices at load) or supply
  `assets/maps/<name>_indices.png` explicitly (where each pixel's red channel
  holds the CRAM slot 0–31 and alpha holds transparency).

## When an edit does not fit

The loader refuses any mod that violates the schema and names **the mod, the
file, and the place** — in `psiv-pack mod check`, in `logs/godot.log`, and on
an in-game startup error screen with **Disable & Retry**, **Retry**, and
**Launch Unmodified** actions.
For example, setting `"hp": 70000` (HP is a 16-bit `u16`, at most 65535) fails
with:

```text
mod `tough-zoran` (battle/enemies.json): could not parse .../battle/enemies.json:
invalid value: integer `70000`, expected u16 at line 1 column ...
```

## Edit-a-copy workflow (`psiv-pack mod diff`) and shareability

If you prefer editing a full copy of `runtime-pack/` directly, keep your base
pack clean and export your changes with `psiv-pack mod diff`:

```bash
cp -rL runtime-pack my-edited-pack
# ... edit files inside my-edited-pack ...
psiv-pack mod diff runtime-pack my-edited-pack mods/my-mod --id my-mod --name "My Mod"
psiv-pack mod check runtime-pack mods/my-mod
```

### Shareable delta mods (default, strict)

By default, `psiv-pack mod diff` fails closed so exported mods never leak
cartridge-derived base data:

- **Allowed by default:** sparse ID-keyed table edits, sparse cell-level grid
  edits (`collision.rows`, `vehicle_battle.rows`, `layout_variants.*.collision.rows`,
  `position_grids.*.cells`), dialogue text edits (exported as `replace` /
  `lines` shorthands), scalar/object field edits, and **brand-new** asset files
  whose path does not exist in `runtime-pack/`.
- **Refused by default:**
  1. **Whole unkeyed base JSON arrays** (such as editing `events` or
     `map_effects`, or resizing a 2D grid) — copying the array embeds retail
     table rows into `patches/`.
  2. **Modified binary assets at existing base-pack paths** (such as editing
     `maps/010_Piata.png` or a base sound file inside `my-edited-pack`) —
     `mod diff` cannot tell whether a modified base file is original hand-drawn
     art or recolored cartridge pixels.

When an edit is refused, `psiv-pack mod diff` exits with an error naming the
offending file and field without touching the output folder.

### Private local edits and original replacement art (`--allow-derived`)

Pass `--allow-derived` to `psiv-pack mod diff` only when:

- you are exporting a **private-only local mod** (for example, edits to unkeyed
  base arrays or touched-up base sprites) that you will **not** redistribute; or
- you are explicitly attesting that every replaced binary asset at a base path
  is **your own original work** containing no cartridge pixels or audio.

If you draw original replacement PNGs or audio directly (as in
[Replacing map and sprite art](#replacing-map-and-sprite-art)), place them
straight into `mods/<your-mod>/assets/`. `psiv-pack mod check` accepts those
mods and prints an advisory note reminding you to share only original work (and
warns if any file in `assets/` is byte-for-byte identical to the base pack).

Only share mods containing **sparse deltas and original assets** in
[Discussions, "Show and tell"](https://github.com/TusanHomichi/PSIV/discussions/categories/show-and-tell).
Never distribute base pack files or edits derived from cartridge art, audio, or
whole retail tables.

## Testing in a source checkout

In a repository checkout (see [build, run and verify](DEVELOPMENT.md)), you can
run `psiv-pack` through Cargo and jump directly to a specific battle or event
with the debug environment variables:

```bash
cargo run --release --manifest-path rust/Cargo.toml -p psiv-pack -- mod check runtime-pack mods/tough-zoran
python3 tools/godot.py prepare
PSIV_MODS="$PWD/mods/tough-zoran" \
  PSIV_DEBUG_BATTLE=0x8a PSIV_DEBUG_STATE="$PWD/state.json" PSIV_DEBUG_SHOT_FRAME=200 \
  python3 tools/godot.py run -- --quit-after 400
```

Leaving `PSIV_SAVE_DIR` unset keeps automatic per-mod save isolation active;
set `PSIV_SAVE_DIR` to a scratch directory if you want a disposable save folder
for automated runs.

## Where the game looks for the pack

In this order:

1. the directory `PSIV_RUNTIME_PACK` names;
2. for an installed game (an exported build), `runtime-pack/` in its
   [user data folder](PLAYING.md#where-your-pack-and-saves-live);
3. for a source checkout, `runtime-pack/` at the repository root.

When that directory holds no pack the game can load, it shows a setup screen
that asks for the ROM and builds the pack there
([`pack_setup.rs`](../rust/psiv-godot/src/pack_setup.rs)).
