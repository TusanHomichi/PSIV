# Modding PSIV

[Project overview](../README.md) · [Documentation index](README.md)

PSIV's game data is not compiled in (its rules and event scripts are). The game
reads the data at start-up from a **data pack**, a directory of JSON tables, PNG
images and sound data that `python3 -m psiv_tools pack <ROM> runtime-pack`
builds from your ROM. Edit those
files and the game plays your edits, as long as they still fit the pack's schema.
The pack is built from the ROM, so a mod is shared as a description or a script
of its edits, never as pack files.

## Where the game looks for the pack

In this order:

1. the directory `PSIV_RUNTIME_PACK` names;
2. `runtime-pack/` beside the game's executable (an exported build);
3. `runtime-pack/` at the repository root (a source checkout).

Work on a copy, so the original stays one command away:

```bash
cp -a runtime-pack my-mod-pack
python3 tools/godot.py prepare
PSIV_RUNTIME_PACK="$PWD/my-mod-pack" PSIV_SAVE_DIR="$PWD/saves" python3 tools/godot.py run
```

`run` inherits `PSIV_RUNTIME_PACK`, and the game prefers that override over the
run root's link to the checkout's pack, so the copy above is the one that loads.

**`pack` replaces.** Running `python3 -m psiv_tools pack` into a directory
builds a new pack beside it and swaps it in whole: the directory becomes a
link to the new version, and the previous versions, edits included, are kept
as `.<name>.v<stamp>-<pid>` beside it until three newer builds prune them.
Keep edits in a separate copy, as above.
A loader that layers a mod over the original pack instead is planned after 1.0
([#118](https://github.com/TusanHomichi/PSIV/issues/118)).

## What each file holds

The game reads these through `psiv-data` (`rust/psiv-data/src/`), and the Rust
types there are the schema: a field's name, type and range are what the loader
accepts.

| To change | Edit | Schema |
| --- | --- | --- |
| Dialogue and every in-game text window | `dialogue/trees.json` | `rust/psiv-data/src/dialogue/trees.rs` |
| Enemies: HP, stats, element resistances, abilities, rewards | `battle/enemies.json` | `rust/psiv-data/src/battle/enemies.rs` |
| Which enemies a battle seats, and where | `battle/formations.json` | `rust/psiv-data/src/battle/formations.rs` |
| Items and equipment | `battle/equipment.json` | `rust/psiv-data/src/battle/party.rs` |
| Party members, techniques and skills, level-up tables | `battle/characters.json`, `battle/abilities.json`, `battle/levels.json` | `rust/psiv-data/src/battle/party.rs`, `abilities.rs`, `levels.rs` |
| Shops and inns: stock, prices, inn rates | `shops.json` | `rust/psiv-data/src/shops.rs` |
| Maps: collision, warps, objects, chests, music | `maps/<id>_<symbol>.json`, with the rendered planes in `maps/<id>_<symbol>.png` and `_over.png` | `rust/psiv-data/src/map.rs` |
| The new-game state | `game_start.json` | `rust/psiv-data/src/new_game.rs` |
| Sprites | `sprites/` (`party.json`, `npcs.json`, `vehicles.json` and the sheets) | `rust/psiv-data/src/sprites.rs` |
| Music and sound effects | `sound/` | `rust/psiv-data/src/sound.rs` |

`manifest.json` lists every map file and the ROM the pack came from; the
loader checks it before anything else. The extractor's other output,
`generated/`, is a research table set; the game does not read it.

## When an edit does not fit

The loader refuses a table that does not fit its schema, names the file and
the place, and the game reports it in its log: for example, an enemy HP of
70000 in `battle/enemies.json` (HP is a 16-bit value, at most 65535) gives

```text
battle pack failed to load from <pack>: could not parse <pack>/battle/enemies.json:
invalid value: integer `70000`, expected u16 at line 1 column 12565
```

and no battle starts. Dialogue is checked for lines that fit the window (at
most 32 characters, two lines a window) and characters the font has
(`dialogue/font.json`).

## Example: change a line of dialogue

Hahn's first line when he finds the professor in the BioPlant is
"Professor! / Thank goodness you're safe!". It is entry 0 of tree 34 in
`dialogue/trees.json`:

```json
{"text": "Professor!"},
{"code": "0xFC", "ctrl": "newline", "operands": []},
{"text": "Thank goodness you're safe!"},
```

An entry's `segments` are what the text window draws: text runs, and control
codes (`newline`, `wait`, `portrait` and the rest) between them. To change the
line, change its `text` segment, and change the same line in the entry's
`pages` (what the cartridge's text loop would show, which the tests compare
with) and its `text` (a plain copy for searching) to match. This script does
all three:

```python
import json
path = "my-mod-pack/dialogue/trees.json"
data = json.load(open(path))
entry = next(t for t in data["trees"] if t["tree"] == 34)["entries"][0]
old, new = "Thank goodness you're safe!", "Glad to see you in one piece!"
for segment in entry["segments"]:
    if segment.get("text") == old:
        segment["text"] = new
for page in entry["pages"]:
    page["lines"] = [new if line == old else line for line in page["lines"]]
entry["text"] = entry["text"].replace(old, new)
json.dump(data, open(path, "w"))
```

The scene's own debug selector opens it directly. With the edited pack, Hahn's
window reads "Professor! / Glad to see you in one piece!":

```bash
PSIV_RUNTIME_PACK="$PWD/my-mod-pack" PSIV_SAVE_DIR="$PWD/saves" \
  PSIV_DEBUG_EVENT=0x8007 python3 tools/godot.py run
```

A longer line takes longer to type out: the window draws one character every
three frames.

## Example: change an enemy's stats

The first random battle of the game, in the Academy basement, is two Zoran
Bults (enemy 10, formation `$8A`) with 25 HP each. In `battle/enemies.json`,
the record with `"id": 10` holds `"hp": 25`; set it to 99:

```python
import json
path = "my-mod-pack/battle/enemies.json"
data = json.load(open(path))
next(e for e in data["enemies"] if e["id"] == 10)["hp"] = 99
json.dump(data, open(path, "w"))
```

The battle's debug selector starts that formation, and `PSIV_DEBUG_STATE`
writes the runtime's state to a file at the frame `PSIV_DEBUG_SHOT_FRAME`
names. With the edited pack both enemies start at 99 HP:

```bash
PSIV_RUNTIME_PACK="$PWD/my-mod-pack" PSIV_SAVE_DIR="$PWD/saves" \
  PSIV_DEBUG_BATTLE=0x8a PSIV_DEBUG_STATE="$PWD/state.json" PSIV_DEBUG_SHOT_FRAME=200 \
  python3 tools/godot.py run -- --quit-after 400
```

```text
"battle": [ {"id": 1, "name": "Chaz", "hp": 25, ...},
            {"id": 6, "name": "ZORAN BULT", "hp": 99, ...},
            {"id": 7, "name": "ZORAN BULT", "hp": 99, ...} ]
```

Every Zoran Bult in the game takes the new HP: a record is per enemy, not per
battle.

## Sharing a mod

Share the script or the list of edits, not pack files: the pack is built from
the ROM, and its files are not yours to distribute. Mods are welcome in
[Discussions, "Show and tell"](https://github.com/TusanHomichi/PSIV/discussions/categories/show-and-tell).
