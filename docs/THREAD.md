# Crawler THREAD dispatch

The connected Alshline run exposed a combat discrepancy in formation 9B:
`build/native-alshline-thread-fallback` reached the final basement floor,
attempted RUN, and lost Chaz and Hahn after the engine substituted physical
attacks for unimplemented THREAD. That run is failed evidence, not a quest
completion. Its original/repaired source saves were untouched.

The original crawler family (enemy IDs 30..32) selects object 0x134 for
ability 16 (`EnemyAttack_Crawler`, ps4.asm:23081). `BattleObj_Thread` calls
`GetEnemySkillEffectAndRange` once at its animation handoff and never requests
a physical damage reaction. The record at 0x2833E4 is effect 6, STR versus
live AGI, physical resistance, miss threshold 64, selected-party target 8.
A success sets battle AGI to max(1, modified AGI minus actor STR). It does not
subtract repeatedly from the previously debuffed value, change HP, apply
paralysis, or change dexterity. Defending's physical resistance affects the
chance. The original wind-up writes EnemyAttack5, SFX DA.

The resolver now dispatches this exact record for the crawler family and
returns before the physical fallback. Two core tests cover threshold equality,
no damage, one chance draw, the floor and repeated non-stacking behavior.
The real-pack test uses original Raja against the four Carrion Crawlers in
9B: learned ANTI keeps his turn non-damaging without DEFEND's resistance,
THREAD lowers AGI, its actors never also attack, SFX DA is emitted, and
ordinary RUN/save/load preserves spent TP and pays no reward.

Native campaign verification passes in `build/native-alshline/route`: the
original saved party survives the corrected crawler encounter, heals through
normal RES, collects Alshline and returns to town. The THREAD capture was
inspected, and the final save also passes fresh title Continue. THREAD's
original web animation and exact timing remain unimplemented. Other unknown
enemy skills still emit explicit unsupported diagnostics and need individual
dispatch work; this fix does not justify a general enemy-ability parity claim.
