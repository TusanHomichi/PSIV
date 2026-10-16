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
Rune supply three-digit values without invented stats.
`tools/native/native_status.gd` plays an ordinary battle, victory, summary,
STATUS and SAVE with it. Status and command icons use a placeholder.


STATUS loads each character's own 80x80 portrait map, including its
border, at (24,16). `status_portraits.py` reads the eleven Nemesis-art pointers
at 0x5D548 and raw 10x10 map pointers at 0x5D574; every map uses CRAM line 2.
The original age words at 0x2A8E12 supply all eleven ages; zero remains
undisclosed for Alys and Rune. These assets and ages are carried by the JSON
character records, keeping the native renderer independent of the ROM.

The new extraction regression covers all eleven maps and ages. A real-pack
runtime regression checks all eleven professions, ages and distinct paths.
The native fixture switches STATUS through Raja, Chaz, Hahn, Gryz and Rune and
saves; it is isolated rendering evidence, not late-game campaign proof.
