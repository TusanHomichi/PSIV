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
your save slots, and `logs` holds the game's log. Save in-game from **Camp →
STATE → SAVE**, and load from the title screen's **CONTINUE**, just like on the
cartridge. To update PSIV later, simply replace the game folder with the new
release's folder.

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

## When something is wrong

If the setup screen says your file is not the US retail ROM, check its SHA-256
against the hash [above](#the-rom).

Where PSIV knowingly differs from the cartridge, there is an open issue: see
[known differences](https://github.com/TusanHomichi/PSIV#known-differences).
For anything else, [open an issue](https://github.com/TusanHomichi/PSIV/issues/new/choose)
with your system, PSIV's version, what happened and where in the game, and the
game's log (`logs/godot.log` in the user data folder above).

## License

PSIV's own code is under the MIT License (`LICENSE.txt`).
`THIRD-PARTY-NOTICES.txt` lists the software it includes and their licenses,
and `nuked-opn2` holds the source of its sound chip emulation. Phantasy Star
IV is Sega's, and PSIV is not affiliated with Sega. What PSIV carries from the
game is listed in its
[license section](https://github.com/TusanHomichi/PSIV#license-and-third-party-code).
