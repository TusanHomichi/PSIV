# Playing PSIV

[Project overview](../README.md) · [Documentation index](README.md)

PSIV is Phantasy Star IV, the US Genesis/Mega Drive release, rebuilt as a
native game. It builds the game's graphics, sound, maps and text from your own
copy of the cartridge's ROM, which it asks for on its first run. This guide is
also the `README.txt` of every release download.

## The ROM

PSIV builds from one ROM image, the US retail release, and refuses any other.
The file's name does not matter; its SHA-256 must be exactly this one:

```text
511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a
```

To check yours: `sha256sum <file>` on Linux, `shasum -a 256 <file>` on macOS,
`Get-FileHash <file>` in Windows PowerShell. An interleaved (`.smd`) or
headered copy, another region or a revision hashes differently.

## Download and start

Download the archive for your system from the
[releases page](https://github.com/TusanHomichi/PSIV/releases), unpack it
anywhere, and start the game:

| System | Archive | Start |
| --- | --- | --- |
| Linux and the Steam Deck (x86-64) | `PSIV-<version>-linux-x86_64.tar.gz` | `psiv.x86_64` |
| Windows (64-bit) | `PSIV-<version>-windows-x86_64.zip` | `PSIV.exe` |
| macOS (Apple silicon and Intel) | `PSIV-<version>-macos-universal.zip` | `PSIV.app` |

The first run opens the setup screen. Choose **Choose your ROM file**, pick
the ROM, and PSIV builds its pack and starts the game. Starting the game with
the ROM's path as its argument does the same without the file browser. The
setup screen comes back only when the pack is missing or a new version of
PSIV needs a new one.

**Linux.** Unpack with `tar xzf PSIV-<version>-linux-x86_64.tar.gz` and run
`./psiv.x86_64` in the folder it makes.

**Steam Deck.** In Desktop Mode, unpack the archive and run `psiv.x86_64`
once, to choose the ROM there. Then, in Steam, choose Games, then Add a
Non-Steam Game to My Library, and browse to `psiv.x86_64`; it plays from
Game Mode with the controller.

**Windows.** Extract the zip first (right-click it, then Extract All): the
game does not run from inside it. PSIV is not signed with a paid certificate,
so the first start may show "Windows protected your PC": choose More info,
then Run anyway.

**macOS.** Unzip it and move `PSIV.app` where you like. PSIV is signed but not
notarized by Apple, so macOS blocks the first open and says it could not
verify the app. Open System Settings, then Privacy & Security, choose Open
Anyway beside the message about PSIV, and confirm. It opens normally after
that.

## Where your pack and saves live

PSIV keeps the pack it built and your saves in your user data folder, not
beside the game, so a new version finds them:

| System | Folder |
| --- | --- |
| Linux and the Steam Deck | `~/.local/share/PSIV` |
| Windows | `%APPDATA%\PSIV` |
| macOS | `~/Library/Application Support/PSIV` |

`runtime-pack` there is the pack, `saves` your saves and `logs` the game's
log. Save from the camp menu's STATE, then SAVE, and load with the title
screen's CONTINUE, as on the cartridge. To update, replace the game's folder
with the new version's.

## Controls

| Action | Keyboard | Controller |
| --- | --- | --- |
| Move, menu selection | Arrow keys or WASD | D-pad or left stick |
| Confirm, talk | Enter, Space or Z | A (bottom face button) |
| Cancel | Escape or X | B (right face button) |
| Camp menu | C | X (left face button) |
| Start | Tab | Start |

## When something is wrong

If the setup screen says your file is not the US retail ROM, its SHA-256 is
not the one [above](#the-rom).

Where PSIV knowingly differs from the cartridge, there is an open issue: the
[known differences](https://github.com/TusanHomichi/PSIV#known-differences).
For anything else, [open an issue](https://github.com/TusanHomichi/PSIV/issues/new/choose)
with your system, PSIV's version, what happened and where in the game, and
the game's log (`logs/godot.log` in the folder above).

## License

PSIV's own code is under the MIT License (`LICENSE.txt`).
`THIRD-PARTY-NOTICES.txt` lists the software it includes and their licenses,
and `nuked-opn2` holds the source of its sound chip emulation. Phantasy Star
IV is Sega's, and PSIV is not affiliated with Sega. What PSIV carries from the
game is listed in its
[license section](https://github.com/TusanHomichi/PSIV#license-and-third-party-code).
