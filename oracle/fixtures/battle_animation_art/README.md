# Battle animation oracle art fixtures

These indexed PNGs are receipt renders, not hand-authored replacement art.
Each frame comes from a named `psiv_oracle --dump-state` capture in
`../battle_animation_remainder.json`: the live Genesis VDP sprite table selects
the linked sprites, and the corresponding `vdp_vram` tile bytes are decoded
into the indexed 320x224 image. The receipt's `origin_pixels` records the
patched first enemy's retail formation position for that screen-space image.
Palette RGB values are intentionally
line-relative; the structural contract is the sprite geometry, tile data, frame
order, duration receipt, and runtime anchor.

The reproducible renderer is
`PYTHONPATH=. python3 -m psiv_tools.battle_animation_oracle`; pass the ROM,
the output directory, and one or more `--capture-root` directories containing
the oracle state dumps. It verifies the dense capture hashes before writing.

These PNGs are oracle evidence only; the pack never reads them (#214). By
pixel index the captures are shared across routine bodies (eight bodies are
one sequence, three another), so they are not each routine's attack art. The
receipt bodies ship `deferred` with no frames in
`battle/art/enemy_attacks.json`; the retail basis is in
`docs/source-notes/README.md`, "Other recorded deviations".
