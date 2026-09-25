# CRASH, BROSE, VOL and SAVOL

These four ordinary player commands now execute their original gameplay.
CRASH is skill 34; BROSE/VOL/SAVOL are techniques 17/18/19. All use effect 2
from the ROM's `AbilityEffectsOffs`, with the normal learned-slot, target,
resource and weapon checks. They do not substitute a physical attack.

## Source and behavior

`AbilityEffect_Death` ($6218) checks the existing death bit and calls
`Battle_ProcessEffect`. `Effect_DoTechnique` ($654A) supplies caster MEN;
`Effect_DoSkill` and `Effect_SetupSkillParams` use the skill's selected stat.
The ordinary chance calculation consumes one RNG2 draw per living eligible
target, in slot order. Zero elemental resistance still consumes that draw
and misses. The inclusive miss threshold is preserved.

- CRASH: actor STR against target STR, destroy resistance, threshold 48,
  and one use from its learned skill slot. It requires a weapon. The original
  consumes the use before rejecting an unarmed actor. Technique seal does
  not block skills. `SkillObj_Crash` loads the axe object and dispatches its
  effect once. `BattleObj_Crash` / `loc_3B82C` clears the successful target's
  HP after its timer.
- BROSE: caster MEN against each enemy's STR and brose resistance, threshold
  48, 16 TP. `TechObj_Brose` computes the target results; `BattleObj_Brose`
  finishes at `loc_39068`, which clears successful enemies' HP.
- VOL and SAVOL: the same success/kill handoff, using MEN resistance and the
  biological element. VOL chooses one enemy; SAVOL visits all living enemies.
  The current US records cost 8 and 18 TP respectively.

Success sets HP to zero and uses the ordinary defeated-enemy path. There is
no physical damage/critical roll or invented damage number. A target killed
earlier in the round is excluded; a single enemy command retargets the first
living enemy. Kills supply the formation's normal experience and meseta once.

BROSE dispatches its original sound $CB. The original palette distortion,
VOL/SAVOL objects, CRASH axe animation, and their exact delays remain work.
The native text/hide sequence is not a claim of original presentation parity.

## Verification

`build/native-death-abilities/core-tests.log`: 220 core battle tests pass.
New cases cover inclusive hit/miss boundaries, normal/weak/immune elements,
MEN versus STR selection, spent TP/skill uses, dead-target retargeting,
no damage draws, suppressed defeated-enemy turns, and once-only rewards.

`build/native-death-abilities/real-record-tests.log` exercises original Gryz
records through JSON, CRASH, BROSE, further paid CRASH attempts after misses,
victory and a save/load cycle. No character stats are boosted. The isolated
Godot fixture uses that original Gryz as a one-person party in Tonoe basement;
it is not campaign progression. The existing opening technique/skill runtime
tests and strict workspace Clippy also pass.

`build/native-death-abilities/route/receipt.json` completes the isolated Godot
run: seven submitted rounds, six paid CRASH commands and one BROSE, then
ordinary victory and SAVE 2. Gryz ends at 57/66 HP, 4/20 TP, one CRASH use,
and 506 meseta (the original 500 plus six). Captures were inspected.

`build/native-death-abilities/continued/receipt.json` starts a fresh Godot
process at the actual title, selects CONTINUE 2, verifies resources, moves
one cell down and writes SAVE 3. Both source files remain byte-identical;
the saved payload differs only at standing Y. This proves native resource
persistence, not physical controller or audible sound output. The battle
command icon still uses a question-mark placeholder for this skill.
