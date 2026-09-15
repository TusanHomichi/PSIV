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

`build/native-battle-recovery/core-tests.log` passes 227 battle tests.
New tests cover original costs, no RNG, human/android boundaries, dead
recipients, paid no-effect casts, partial/full revival, seal preservation,
the AGI/DEX reset quirk and preservation of unrelated resources.
`runtime-tests.log` passes the new real-record recovery battle/save test
and the existing instant-death, item and technique integration tests.

The isolated recovery fixture uses original level records for level-31
Raja and level-eight Chaz/Hahn, then deliberately injures/statuses the
recipients. Raja's REGEN is earned from the real level-31 row. This is not
connected campaign progress. `build/native-battle-recovery/route/receipt.json`
completes all four casts through normal Godot menus, victory and SAVE 2.
Raja spends exactly 55 TP (204 to 149); Chaz is cured and Hahn revives,
reaches full 55 HP through REGEN, and retains tech seal. A fresh process
uses title CONTINUE 2 and re-saves after one normal step; only standing Y
changes in the payload, and both source slots remain unchanged.

Inspection of those captures exposed separate three-digit number and
party-status layout defects. Their fix/proof is tracked in `PARTY_STATUS.md`.
The original spell objects, sound delays and detailed reaction animation
remain presentation work.
