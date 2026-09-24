# Oracle methodology

Scope: how the graphics and battle-formation decoders are pinned
against the retail ROM — the exact-length oracles, the visual
identifications, and the uncompressed disassembly sources used as
acceptance criteria.

Index: [source and provenance notes](../../SOURCE_NOTES.md).

## Oracle methodology for graphics

No decompressed Nemesis source exists in the disassembly, so formations-style
round trips are unavailable. Substitutes, in decreasing strength:

- Battle backgrounds carry a ROM-internal exact-length oracle: each art blob's
  Enigma plane mapping is stored immediately after it, and all 20 blobs decode
  to exactly `mapping_ptr - art_ptr` consumed bytes.
- The disassembly stores Nemesis blobs already compressed; each `binclude`
  payload occurs exactly once in the retail image, fixing the offsets. The
  oracle tests match portraits and battle art to payloads by content, not by
  filename.
- Visual identification pins the decoders end to end: the font decodes to the
  Latin alphabet (letter-A bitmap pinned in a test), the Sega logo proves the
  XOR accumulator, Chaz's portrait proves row-major 48×48 composition, and the
  Mota desert background proves the pointer-table → art → palette path.
- sha256 pins: font, Sega logo, Chaz individually, plus one digest over the
  sorted digests of all 68 blobs.

## Oracle methodology for battle formations

`reference/ps4disasm/battles/{battle_formations_1..4,boss_formations}.asm`
contain the same formation data uncompressed, as plain `dc.b` listings. The
test suite parses those listings (rejecting any non-`dc.b` directive rather
than assembling anything) and asserts byte-for-byte equality against the
decompressed retail blobs. All five match exactly: 1,708 / 1,716 / 1,678 /
1,540 / 300 bytes. That equality, not the plausibility of the output, is the
acceptance criterion for the decoder.

The enemy/item id conventions used by formation records were proven the same
way. `ps4.constants.asm` defines `EnemyID_Helex = 0` and `ItemID_Dagger = 1`,
and the formation sources annotate every enemy byte with its symbol; the tests
check the extractor's resolution against those annotations for all 100+
distinct enemy ids that appear in formations.
