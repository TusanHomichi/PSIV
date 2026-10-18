# RIMIT, the party's sleep technique

Every technique's dispatch and capture status is
[Player techniques and skills](PLAYER_ABILITIES.md), which also owns the FEEVE,
AROWS and SEALS rules. This page is RIMIT's record.

RIMIT (technique 23) is the record at `0x2A9C98`: effect 7, 10 TP, all
enemies, MEN against MEN, psychic resistance, miss threshold 48.
`AbilityEffect_SleepParalyze` returns before drawing a chance for a target
already asleep or paralyzed. Immune targets still draw and miss, and equality
at the threshold misses. `BattleObj_RimitMain` (`ps4.asm:74245`) sets sleep bit
3 on each target it lands on, without reducing agility. A sleeping fighter's
turn is skipped by the queue and it wakes through the ordinary round-end
recovery. The object starts sound `$CB`; a RIMIT that a late seal cancels has
paid its TP and plays no cue ([cancelled spell cues](../sound/SOUND_INTEGRATION.md#cancelled-spell-cues)).
Its spell artwork and exact object timing are not modelled.

Tests: two core tests cover the chance boundary, the order, immunity, the
skipped draw, unchanged HP and agility, and one TP payment; the runtime
regression casts Raja's level-25 RIMIT from the pack's own records against
formation `$8A`, wins, and round-trips a save.
