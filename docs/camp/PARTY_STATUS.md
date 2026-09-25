# Party status seats and numeric field widths

The three-character opening references concealed two hardcoded assumptions:
`append_status_quads` only drew members in the middle three panes, and the
UI sorted those members by character ID. Four/five-character parties lost
status displays; a reordered party could display a different character's
status under each combat sprite. The icon renderer had the same three-pane
assumption.

Both renderers now look up fighter seats in left-to-right order 4, 2, 1, 3,
5. This is the center-out order already used by `PARTY_COLUMNS`. The opening
party keeps its original placement, while one-, four- and five-character
parties occupy the appropriate seats independently of character IDs or
input array order. The five-size regression deliberately reverses that
array to prevent another ordering shortcut.

Battle HP/TP now right-align within their three-cell fields. Previously
three-digit values started one cell too far right and overwrote the next
pane. Camp's current/max values also use three-cell fields, and levels use
two cells. Previously the slash overwrote a third current-value digit,
three-digit maxima crossed the window border, and a two-digit level crossed
the summary border. Existing two-digit HP/TP and single-digit level glyphs
retain their positions.

The five-person visual fixture deliberately seats original Raja, Chaz,
Hahn, Gryz and Rune records in that order. It changes only the party and
location; character stats remain their original initial values. Raja and
Rune supply three-digit values without invented stats. The first native run completed under `build/native-status/route`: an ordinary
battle, victory, summary, STATUS and SAVE. Fresh title Continue from slot 2,
one downward step and SAVE to slot 3 passed, changing only standing Y at
logical save offset 0x309. Source slots remain unchanged. All 875 workspace
tests passed at that checkpoint; the Godot library accounted for 72.
Status/command icons still use the existing question-mark placeholder.


The same capture exposed two further shortcuts: STATUS tiled Chaz's 48x48
dialogue portrait into a 64x64 box for every character, and the schema read
`name` while the pack supplied `display_name` for professions. STATUS now
loads each character's separate original 80x80 portrait map, including its
border, at (24,16). `status_portraits.py` reads the eleven Nemesis-art pointers
at 0x5D548 and raw 10x10 map pointers at 0x5D574; every map uses CRAM line 2.
The original age words at 0x2A8E12 supply all eleven ages; zero remains
undisclosed for Alys and Rune. These assets and ages are carried by the JSON
character records, keeping the native renderer independent of the ROM.

The new extraction regression covers all eleven maps and ages. A real-pack
runtime regression checks all eleven professions, ages and distinct paths.
Native verification passed in `build/native-status/portraits/route/receipt.json`.
The fixture wins its battle, switches STATUS through Raja, Chaz, Hahn, Gryz
and Rune, and saves. The clean five-person battle capture, Raja's portrait/
PRIEST/age 85 and Chaz's portrait/HUNTER/age 16 were visually inspected.
The pack determinism/manifest tests also pass (15 tests including portrait
extraction). This is isolated rendering evidence, not late-game campaign proof.
