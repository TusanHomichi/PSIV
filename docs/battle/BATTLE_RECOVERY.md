# ANTI, RIMPA, REVER and REGEN in battle

Technique IDs 34 through 37 now work in the native battle command path.
The field versions already existed. Their original costs are 2, 5, 12 and
36 TP. All four target humans; androids are excluded before payment.

`Ability_ProcessRange` allows only dead targets for REVER, allows either
state for REGEN, and excludes dead targets for ANTI/RIMPA. A selected
ineligible human still consumes the cast's TP. There are no RNG draws for
these four effects. The ordinary late-seal check still consumes TP and
prevents the effect.

The US `bugfixes=0` effect dispatcher sends effects 19 through 22 through
`AbilityEffect_RestoreAgiAndDex`: eligible recipients reset battle AGI and
DEX to their modified values, even when no relevant ailment is present.
Other stat buffs and spent resources remain untouched.

`BattleObj_StatusHealMain` clears poison for ANTI ($22) and paralysis for
RIMPA. `BattleObj_ReverMain` clears all persistent status flags except tech
seal, sets REVER HP to `max_hp >> 2` and REGEN HP to maximum. REGEN applies
the same status clearing to a living recipient. Revived fighters use the
existing same-round suppression and retargeting rules; no temporary sprite
bit is written into their save record.

Battle tests cover the original costs, no RNG, human and android boundaries,
dead recipients, paid no-effect casts, partial and full revival, seal
preservation, the AGI/DEX reset quirk and preservation of unrelated resources; a
runtime test runs a real-record recovery battle and save.
`tools/native/native_battle_recovery.gd` plays the isolated recovery fixture
(level-31 Raja, level-eight Chaz and Hahn, injured and statused on purpose)
through normal menus: Raja spends exactly 55 TP, Chaz is cured, Hahn revives and
reaches full HP through REGEN and keeps the tech seal for the rest of the fight
(`Battle_LastMessage` clears bits 3 and 4 of every party slot on the way out,
`andi.b #$E7, $16(a0)`, `ps4.asm:6366`). A fresh process loads its save through
CONTINUE. It is not connected campaign progress.

Three-digit numbers and the party-status layout are
[PARTY_STATUS.md](../camp/PARTY_STATUS.md)'s.
The original spell objects, sound delays and detailed reaction animation
remain presentation work.
