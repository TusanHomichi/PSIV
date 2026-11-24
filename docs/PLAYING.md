# Playing PSIV

[Project overview](https://github.com/TusanHomichi/PSIV) ·
[Documentation index](https://github.com/TusanHomichi/PSIV/blob/main/docs/README.md)

PSIV is Phantasy Star IV, the US Genesis/Mega Drive release, rebuilt as a
native game. It builds the game's graphics, sound, maps and text from your own
copy of the cartridge's ROM on first launch. This guide is also the
`README.txt` inside every release download.

**Quick start:** download and extract the archive for your OS, launch the game,
and pick (or drag and drop) your US Phantasy Star IV ROM on the setup screen.
PSIV builds its data pack once in a few seconds and boots straight to the title
screen.

## Setting up with an AI assistant

If a player asks you (an AI assistant or coding agent) to get PSIV set up for them:

1. **Use the prebuilt release — do not clone or compile from source** unless
   they explicitly want to develop or mod PSIV.
2. **Download the latest release archive for their OS** from
   `https://github.com/TusanHomichi/PSIV/releases/latest` (via `gh release download`
   or `curl`/`Invoke-WebRequest`), and verify it against `SHA256SUMS.txt` on the
   same release:
   - Linux (x86-64): `PSIV-*-linux-x86_64.tar.gz` → extract with `tar xzf`, run `./psiv.x86_64`.
   - Windows (64-bit): `PSIV-*-windows-x86_64.zip` → extract with `Expand-Archive`, run `PSIV.exe`.
   - macOS (Apple silicon & Intel): `PSIV-*-macos-universal.zip` → extract `PSIV.app` (on first run, the player may need **System Settings → Privacy & Security → Open Anyway**).
3. **Ask the player for the path to their own US Genesis/Mega Drive ROM.**
   Never search for or download a ROM from the internet. Check its SHA-256
   matches [the hash below](#the-rom); if it differs (headered `.smd`, another
   region, or a ROM hack), explain why before launching.
4. **Launch the game:** either open it normally so the player can pick or drop
   their ROM on the setup screen, or pass their verified ROM path as the first
   command-line argument (e.g. `./psiv.x86_64 "/path/to/rom.md"`) to build the
   pack automatically into their user data folder and boot straight to the title
   screen.
5. **Hand back a short summary:** where you extracted PSIV, where their saves
   live ([below](#where-your-pack-and-saves-live)), and the basic controls
   ([below](#controls)).

## The ROM

PSIV builds from one ROM image, the unheadered US retail release, and refuses
any other. The file's name and extension (`.md`, `.bin`, `.gen`) do not matter;
its SHA-256 must be exactly:

```text
511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a
```

To check yours:

- **Linux:** `sha256sum <file>`
- **macOS:** `shasum -a 256 <file>`
- **Windows (PowerShell):** `Get-FileHash <file>`

An interleaved (`.smd`) or headered copy, another region, or a ROM hack hashes
differently. You only need to supply the ROM once; after PSIV builds the pack
into your user data folder, you do not need to keep the ROM next to the game.

## Download and start

Download the archive for your system from the
[releases page](https://github.com/TusanHomichi/PSIV/releases), extract it
anywhere, and launch the game:

| System | Archive | Start |
| --- | --- | --- |
| Linux and the Steam Deck (x86-64) | `PSIV-<version>-linux-x86_64.tar.gz` | `psiv.x86_64` |
| Windows (64-bit) | `PSIV-<version>-windows-x86_64.zip` | `PSIV.exe` |
| macOS (Apple silicon and Intel) | `PSIV-<version>-macos-universal.zip` | `PSIV.app` |

Everything needed to run is inside the archive. You do not need to install
Godot, Rust, or Python, or download this project's source.

The first launch opens the setup screen:

- Click **Choose your ROM file** and select your ROM, **or** drag and drop the
  ROM file directly onto the window.
- You can also pass the ROM path as a command-line argument to skip the file
  picker.

PSIV checks the ROM hash, builds its pack in a few seconds, and starts the
game. The setup screen returns only if the pack is missing or a newer version
of PSIV requires a rebuilt pack.

**Linux.** Unpack with `tar xzf PSIV-<version>-linux-x86_64.tar.gz` and run
`./psiv.x86_64` in the extracted folder. You can also launch it from another
directory using its full path; the working directory does not affect where your
pack or saves live.

**Steam Deck.** In Desktop Mode, extract the archive and run `psiv.x86_64` once
to pick your ROM. Then open Steam, choose **Games → Add a Non-Steam Game to My
Library**, and select `psiv.x86_64`. After that, it plays straight from Game
Mode with the Deck's controls.

**Windows.** Extract the zip first (right-click → **Extract All**): the game
will not run from inside a compressed folder. PSIV is not signed with a paid
certificate, so Windows SmartScreen may show "Windows protected your PC":
choose **More info**, then **Run anyway**.

**macOS.** Unzip the archive and move `PSIV.app` where you like. PSIV is signed
but not notarized by Apple, so macOS blocks the first open. Open **System
Settings → Privacy & Security**, click **Open Anyway** beside the message about
PSIV, and confirm. It opens normally after that.

## Where your pack and saves live

PSIV stores its built pack and your save files in your user data folder,
separate from the game folder, so upgrading to a new version keeps your saves:

| System | Folder |
| --- | --- |
| Linux and the Steam Deck | `~/.local/share/PSIV` |
| Windows | `%APPDATA%\PSIV` |
| macOS | `~/Library/Application Support/PSIV` |

On Linux, setting `XDG_DATA_HOME` moves this folder to `$XDG_DATA_HOME/PSIV`.

Inside that folder, `runtime-pack` holds the extracted game pack, `saves` holds
your save slots, `mods` holds optional shareable mod folders, and `logs` holds
the game's log. Save in-game from **Camp → STATE → SAVE**, and load from the
title screen's **CONTINUE**, just like on the cartridge. To update PSIV later,
simply replace the game folder with the new release's folder.

## Controls

| Action | Keyboard | Controller |
| --- | --- | --- |
| Move, menu selection | Arrow keys or WASD | D-pad or left stick |
| Confirm, talk (Genesis C) | Enter, Space or Z | A (bottom face button) |
| Cancel (Genesis B) | Escape or X | B (right face button) |
| Camp menu (Genesis A) | C | X (left face button) |
| Start | Tab | Start |

In menus, the Camp button (`C` / controller `X`) also confirms selections, just
like Genesis button A. On the Motavia and Dezolis overworlds, open **Camp →
ITEM → MAP** to scroll the planet map.

## Mods

To install a mod, drop its folder (containing `mod.json`) into the `mods`
directory inside your [user data folder](#where-your-pack-and-saves-live) and
start the game. Modded runs automatically isolate saves under
`saves/mods/<profile>/` so your vanilla saves stay untouched; set
`PSIV_MODS=none` to launch without mods. Each release archive also includes
`psiv-pack` (`psiv-pack mod init`, `check`, and `diff`) for creating and
validating mods — see the
[Modding Guide](https://github.com/TusanHomichi/PSIV/blob/main/docs/MODDING.md)
for details.

## When something is wrong

If the setup screen says your file is not the US retail ROM, check its SHA-256
against the hash [above](#the-rom).

Where PSIV knowingly differs from the cartridge, there is an open issue: see
[known differences](https://github.com/TusanHomichi/PSIV#known-differences).
For anything else, follow the
[bug reporting guide](https://github.com/TusanHomichi/PSIV/blob/main/docs/REPORTING.md)
or [open an issue](https://github.com/TusanHomichi/PSIV/issues/new/choose)
with your system, PSIV's version, what happened and where in the game, and the
game's log (`logs/godot.log` in the user data folder above). Using an AI
assistant? Point it at the bug reporting guide and it can collect your log,
save slot info, and system details to file the report for you.

## License

PSIV's own code is under the MIT License (`LICENSE.txt`).
`THIRD-PARTY-NOTICES.txt` lists the software it includes and their licenses,
and `nuked-opn2` holds the source of its sound chip emulation. Phantasy Star
IV is Sega's, and PSIV is not affiliated with Sega. What PSIV carries from the
game is listed in its
[license section](https://github.com/TusanHomichi/PSIV#license-and-third-party-code).
