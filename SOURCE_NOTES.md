# Source / provenance notes

The extractor is grounded against Peter's verified US retail PSIV ROM and the public `ps4disasm` work. No ROM bytes are shipped in this repository beyond very short structural signatures used as tests.

## Public reverse-engineering references used

- `squidfeatures/ps4disasm` (`ps4.asm`, `ps4.constants.asm`), including the documented record layouts and stable symbolic IDs.
- `alechenninger/ps4disasm`, a later fork used as an additional orientation/reference source.

The public disassembly documents, among other things:

- initial character records as 66-byte entries
- inventory records as 22-byte entries
- enemy records as 48-byte entries
- enemy-skill records as 8-byte entries
- techniques, skills, and combo records as 8-byte entries
- vehicle records as 26-byte entries
- character level records as 22-byte entries, addressed through a per-character pointer table
- battle formations as variable-length `$FF`-terminated records inside Kosinski-compressed blobs

## Retail US offsets proven against this ROM

- Enemy data: `0x2816BC`
- Enemy skills: `0x28336C`
- Combo data: `0x285424`
- Vehicle data: `0x2855E2`
- Level pointer table: `0x004074`
- Primary Chaz level table / start of primary progression block: `0x2856B0`
- Mirrored progression block: `0x2A3A42`
- Initial character stats: `0x2A8ACA`
- Inventory data: `0x2A8E28`
- Technique data: `0x2A9BE8`
- Skill data: `0x2A9D28`

Kosinski-compressed blobs (end-exclusive):

- Battle formation indexes: `0x2836EC..0x283E67`
- Battle formation data 1: `0x283E6C..0x2842AE`
- Battle formation data 2: `0x2842BC..0x284713`
- Battle formation data 3: `0x28471C..0x284B7E`
- Battle formation data 4: `0x284B8C..0x284F7C`
- Boss formation data: `0x284F7C..0x285012`

- Shop inventories (`ShopInventories`): `0x0681A4..0x0682A3`, 49 `$FF`-terminated lists indexed by `GetOffsetByID_FF_Delim` (terminator counting, no pointer table, no bounds check — the bound lives in the data, corroborated by the 49-entry shopkeeper-portrait and greeting tables)
- Shop locations (`loc_68394`): `0x068394`, `$FFFF`-terminated 8-byte entries `(map id, x, y, group<<8|index)`; group 0 is the inn system with its own index space, groups 1–2 read `ShopInventories`

The offsets are accepted only after the full ROM SHA-256 matches the supported retail build, then short known signatures are checked again at those offsets before extraction.

## Important boundary

`ITEM_SYMBOLS` and `ENEMY_SYMBOLS` are disassembly identifiers, not decoded cartridge text. This is intentional. The eventual text-system decoder should own actual display names so this tooling does not silently blend researcher labels with source-ROM text.

## Kosinski decompression

`psiv_tools/kosinski.py` is a transcription of `KosDecomp` in `ps4.asm`, not an
implementation written from a prose description of the format. Each branch of
the Python decoder names the asm label it corresponds to. Two details were
taken from the routine rather than assumed:

- Description fields are 16-bit little-endian and their bits are consumed
  LSB-first, but a field is refilled only *after* its sixteenth bit has been
  consumed and *before* that element's operand bytes are read (the `dbf d4`
  sits between the `lsr.w` and the operand fetch). A decoder that refills at
  the start of the sixteenth element desynchronises on real data.
- In the extended-count branch, the extra byte `0` ends the stream and `1`
  falls back into the main loop, consuming three bytes and two description
  bits without emitting output.

The decoder returns the number of compressed bytes consumed, which turns every
documented blob range into a verification: decompression must terminate on the
end marker at exactly the annotated end address.

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

## Discrepancies found against the disassembly's annotations

- `Battle_FormationData4`: the inline range annotation in `ps4.asm` gives
  `0x284B8C-0x284C3D`, which is only the first of many annotated chunks for
  that blob. The Kosinski stream actually runs to `0x284F7C`, immediately
  before `Battle_BossFormationData`, and the decompressor consumes exactly
  0x3F0 bytes.
- Formation `0x177` (block 3 record `$77`) declares four enemies in byte 4 but
  lists three enemy/position pairs, and its group bitmasks cover three slots.
  The disassembly's uncompressed source has the same bytes, so this is the
  ROM's own inconsistency rather than a decode error. Both the declared count
  and a mismatch flag are emitted.
