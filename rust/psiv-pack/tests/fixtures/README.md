# Primitive fixtures

`primitives.json` contains synthetic inputs only. Its 99 decoder calls were
recorded from the passing `TestEnigmaHeader`, `TestEnigmaDecoder` and
`TestNemesisDecoder` methods in `tests/test_enigma.py` and `tests/test_gfx.py`
at `382f8770`. Each call retains its originating test name, arguments, decoded
output and consumed length, or an explicit refusal. Header-only calls retain
the five Rust header fields. The 40 PNGs are `tests/test_png.py`'s indexed and
RGB examples, re-encoded with filters 0–4 and compression levels 0, 1, 6 and 9,
split IDAT chunks and an ancillary annotation. Their expected image digests
come from the original unfiltered images. Compressed file bytes are inputs,
never the expected identity of an image.

`primitive_retail.json` contains only labels, offsets, lengths, geometry and
digests from the same revision's passing graphics/plane extractors: 68 distinct
Nemesis art blobs, 35 Enigma mappings and five named palettes. It contains no
ROM bytes, decoded words, palettes or pixels. The retail tests independently
decode the local verified US ROM and compare words and composed pixel digests;
the reference tests read the local disassembly's binary assets. Neither local
input is distributed. Missing inputs print explicit skips; corrupt inputs fail.

These are frozen acceptance vectors, not regenerated expected values during a
test run. Change an expectation only with a documented retail correction.
The decoder census remains the separate full-pack Python/Rust parity check.
