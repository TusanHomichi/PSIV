# Battle animation oracle art fixtures

These indexed PNGs are receipt renders, not hand-authored replacement art.
Each frame comes from a named `psiv_oracle --dump-state` capture in
`../battle_animation_remainder.json`: the live Genesis VDP sprite table selects
the linked sprites, and the corresponding `vdp_vram` tile bytes are decoded
into the indexed 320x224 image. The receipt's `origin_pixels` anchors that
screen-space image at the patched first enemy's retail formation position when
Godot attaches it to the local enemy node. Palette RGB values are intentionally
line-relative; the structural contract is the sprite geometry, tile data, frame
order, duration receipt, and runtime anchor.

The reproducible renderer is
`PYTHONPATH=. python3 -m psiv_tools.battle_animation_oracle`; pass the ROM,
the output directory, and one or more `--capture-root` directories containing
the oracle state dumps. It verifies the dense capture hashes before writing.

The pack emitter copies these source frames into
`battle/art/enemy_attacks/` and retains the routine/frame receipt in the JSON
art index. A missing source frame is an emission error, never a transparent
fallback.
