# RIMIT and remaining technique work

RIMIT (23) is now supported by the command resolver. The original eight-byte
record at 0x2A9C98 specifies effect 7, 10 TP, all enemies, MEN versus MEN,
psychic resistance and miss threshold 48. `AbilityEffect_SleepParalyze`
returns before drawing a chance for an already asleep or paralyzed target.
Immune targets still draw and miss; equality at the threshold misses.
`BattleObj_RimitMain` (ps4.asm:74245) sets sleep bit 3 on successful targets
without reducing agility. The existing queue skips sleeping fighters and
uses the ordinary round-end recovery path. The original object starts SFX CB;
its spell artwork and exact object timing are still pending.

Two core regressions cover the chance boundary, order, immunity, skip/no-draw,
unchanged HP/AGI and one TP payment. The real-pack runtime regression uses
Raja's original level-25 RIMIT, original five-character records and formation
8A, then ordinary victory and a complete save round trip. The native fixture passed in `build/native-rimit/route/receipt.json`: Raja
casts RIMIT through the normal menu, the enemy-sleep message appears, the
party wins, Raja retains 160 of his original 170 TP, and SAVE writes slot 2.
Both cast and sleep captures were inspected. Fresh title Continue from slot 2, one Down and SAVE to slot 3 also passed.
The only payload change is standing Y at 0x309 (208 to 224), and both source
slots remain unchanged. Validation is under `build/native-rimit/continued`. This raises supported battle techniques to 35 of 38; SEALS, FEEVE
and AROWS remain.

## Late-seal sound correction (2026-09-15)

A paid but sealed RIMIT no longer emits its spell sound. Successful RIMIT
still emits `$CB`, including when a target resists. The runtime regression
and verified original seal branch are recorded in
[SOUND_INTEGRATION.md](../sound/SOUND_INTEGRATION.md). Gameplay coverage remains 35 of
38 battle techniques; this closes a sound-dispatch bug.

## FEEVE source trap — not implemented

Do not translate power 11 into psychic resistance slot 10. The US ROM's
actual effect-13 routine at 0x6370 reads byte 3 of the technique, adds 0x2E
and writes one byte, without doubling the selector. The actual FEEVE record
at 0x2A9CE8 is `0d 05 19 0b 00 00 00 00`; this addresses stats byte 0x39,
the saved low byte of the water-resistance word at 0x38, rather than the
live psychic word at 0x44. The later restoration/persistence consequences
still need tracing. The source's generic comment about increased elemental
resistance is insufficient to implement this record correctly.

AROWS also needs its original all-eleven-character stored-roster sleep clear,
and SEALS needs the actual enemy attack-family handling after a seal. Neither
should be marked supported merely because a target flag can be changed.
