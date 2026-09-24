//! Record-driven enemy damage skills.
//!
//! `Enemy_Attack` (`ps4.asm:19138`) loads one attack object per enemy turn,
//! stores the drawn `Current_Target_Index` in that object's `$38(a1)`
//! (lines 19170-19172) and hands the object to `EnemyAttackOffs[enemy_id]`
//! (`ps4.asm:19206`). A traced arm then ends in `move.w #$C, $2(target)` —
//! routine `$C`, `Fighter_TakeDamage` (`ps4.asm:3564`) — aimed at the stored
//! target, or in a five-slot loop that writes the same word to every party
//! slot. Either way the ability's `EnemySkillData` record decides the number:
//! the request reaches `Enemy_DamageCharacter`'s enemy-skill branch
//! (`ps4.asm:3775`), reads the caster's stat through record byte 1, the
//! target's stat through byte 4, the target's element factor through byte 5 and
//! the record's byte 3 as the bonus, and calls `Battle_CalculateDamage`
//! (`ps4.asm:17374`) once per target.
//!
//! [`DAMAGE_SKILL_ROUTES`] is therefore the whole gate: one `(enemy, ability)`
//! pair per arm whose request shape has been read out of the disassembly. The
//! pair proves the *route* and its [`DamageClass`] — one `$38` request, or the
//! five-slot all-party loop; the record still drives the arithmetic, and no
//! record-byte predicate is part of the proof beyond the effect handler: a
//! record whose effect is not `$01` (`AbilityEffect_None`, `ps4.asm:9092`)
//! would need that handler modelled as well, so it stays on the explicit
//! [`BattleEvent::UnsupportedAbility`] path. Byte 2's target nibble is *not* a
//! gate — it picks the `Ability_ProcessRange` (`ps4.asm:8975`) handler for the
//! effect, and the damage request the object makes comes from the object chain
//! alone.
//!
//! Anything outside the table keeps the explicit
//! [`BattleEvent::UnsupportedAbility`] path.

use super::{BattleData, BattleEvent, FighterId, Rolls, Roster, Side};

#[cfg(test)]
#[path = "enemy_damage_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "enemy_damage_acid_tests.rs"]
mod acid_tests;

#[cfg(test)]
#[path = "enemy_damage_flame_tests.rs"]
mod flame_tests;

#[cfg(test)]
#[path = "enemy_damage_gate_tests.rs"]
mod gate_tests;

#[cfg(test)]
#[path = "enemy_damage_motavia_tests.rs"]
mod motavia_tests;

#[cfg(test)]
#[path = "enemy_damage_all_party_tests.rs"]
mod all_party_tests;

/// An `AbilityEffectsOffs` (`ps4.asm:9036`) index whose handler does nothing
/// but return: `$01` is `AbilityEffect_None` (`ps4.asm:9092`), a bare `rts`.
///
/// The routes below model the damage request and nothing else, so a record
/// whose effect handler would also do something is not guessed at: it stays on
/// the unsupported path until that handler is implemented. Index `$00` is the
/// same handler, but no enemy skill record in the table carries it — every
/// damage record written by the game uses `$01`.
const EFFECT_NONE: u8 = 0x01;

/// What a proven route's object chain does with `move.w #$C`
/// (`Fighter_TakeDamage`, `ps4.asm:1033`): how many requests it makes and
/// against whom.
///
/// Record byte 2's low nibble is deliberately *not* the class. It selects the
/// `AbilityRangeOffs` (`ps4.asm:8903`) handler that `Ability_ProcessRange`
/// (`ps4.asm:8975`) uses to decide which fighters the *effect handler* runs
/// on, and effect `$01` runs on none of them anyway. The damage request is
/// made by the object chain, against the target `Enemy_Attack` stored in the
/// object's own `$38` (`ps4.asm:19172`). Nibble 8 (`AbilityRange_Single`,
/// `ps4.asm:8938`) and nibble 9 (`AbilityRange_MultiChars`, `ps4.asm:8962`)
/// routes below both make exactly one request, so the class comes from the
/// traced chain — never from the nibble.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DamageClass {
    /// Exactly one `move.w #$C, $2(aX)` in the whole chain, against the
    /// object's `$38`: the drawn `Current_Target_Index`, one party member.
    Single,
    /// Five requests, one per party slot, from a `moveq #4, dN` loop over
    /// `Obj_Fighters`: the shared tails `loc_24A9E` (`ps4.asm:48483`) and
    /// `loc_24BB6` (`ps4.asm:48562`), plus the two objects that inline the same
    /// loop — `BattleObj_LocustaSpiralBld` (`ps4.asm:29275-29280`) and
    /// `BattleObj_Earthquake` (`ps4.asm:47995-48000`).
    ///
    /// What the loop proves, and why the resolution below is its shape:
    ///
    /// - **Which slots.** The write reaches all five, empty and dead included.
    ///   `Battle_UpdateFighters` (`ps4.asm:987`) skips a slot whose object word
    ///   is zero, and `Fighter_TakeDamage` (`ps4.asm:3564`) skips a slot whose
    ///   `Fighters_Hit_Flags` byte (`ps4.constants.asm:2019`) is negative. That
    ///   byte is filled by `loc_B6A2` (`ps4.asm:17492`) *after* the arm has
    ///   run: these arms clear `Current_Target_Index` (see the route
    ///   comments), so `d7 = 4, d6 = 1` (lines 17510-17511) covers slots 1-5 and
    ///   `loc_B75A` (line 17559) writes 0 for a slot whose status carries
    ///   neither `StatusDead_Mask` nor `StatusAndroidDead_Mask` (`$04` and
    ///   `$40`, `ps4.constants.asm:77-81`) and `$FF` for one that does. So
    ///   every occupied, living party slot takes one hit, and an empty or out
    ///   slot takes none and draws nothing.
    /// - **Order.** `Battle_UpdateFighters` walks `Obj_Fighters` upward by
    ///   `obj_size` `$40` for 12 slots with one routine call each, i.e. slot
    ///   order 1-5. The object writes all five requests in one frame, so all
    ///   five `$C` routines run in the **same** frame in that order, and each
    ///   draws its own [`super::calculate_damage`] rolls through
    ///   `Enemy_DamageCharacter` (`ps4.asm:3775`, reached from
    ///   `Figher_DamageCheckActor`'s enemy branch at line 3744) — the same 16
    ///   draws a single-target request takes.
    /// - **Deaths.** `Fighter_TakeDamage` only *computes*: hit points come off
    ///   in `FighterShowDamage_DecreaseHP` (`ps4.asm:3640`), three window
    ///   phases later (`loc_24BE`, line 3622, drops routine `$E` to `$D`), and
    ///   every fighter advances one phase per frame. All five targets
    ///   therefore reach that phase in the same frame, after every roll has
    ///   been drawn, so a target that empties its HP there cannot stop a later
    ///   target's computation. The `$1C` gate in `loc_24A9E`/`loc_24BB6`
    ///   (lines 48489, 48568) postpones the whole five-slot write, never one
    ///   member of it, so it cannot skip a target either.
    AllParty,
}

/// `EnemySkillData` `$33` ACIDBREATH, record `01 01 08 18 06 01 00 00` at
/// `$2834FC`.
const ACID_BREATH: u8 = 0x33;

/// `EnemySkillData` `$02` FLAME BOLT, record `01 01 08 50 07 03 00 00` at
/// `$283374`: effect `$01`, stat `$01` (strength), tgt 8, pow 80, res `$07`
/// (magic defense), el `3` (fire).
const FLAME_BOLT: u8 = 0x02;

/// `EnemySkillData` `$2E` GIWAT, record `01 82 08 58 07 05 00 00` at
/// `$2834D4`: effect `$01`, byte 1 `$82` — masked to selector 2 (mental) — tgt
/// 8, pow 88, res `$07` (magic defense), el `5` (water/ice).
const GIWAT: u8 = 0x2E;

/// `EnemySkillData` `$37` SAND STORM, record `01 05 09 60 06 01 00 00` at
/// `$28351C`: effect `$01`, stat `$05` (attack, a word), tgt 9, pow 96, res
/// `$06` (defense), el `1` (physical).
const SAND_STORM: u8 = 0x37;

/// `EnemySkillData` `$39` MAELSTROM, record `01 05 09 20 06 01 00 00` at
/// `$28352C`: effect `$01`, stat `$05` (attack), tgt 9, pow 32, res `$06`
/// (defense), el `1` (physical).
const MAELSTROM: u8 = 0x39;

/// `EnemySkillData` `$08` SPIRAL BLD, record `01 05 09 00 06 01 00 00` at
/// `$2833A4`: effect `$01`, stat `$05` (attack, a word), tgt 9, pow 0, res
/// `$06` (defense), el `1` (physical).
const SPIRAL_BLD: u8 = 0x08;

/// `EnemySkillData` `$38` EARTHQUAKE, record `01 05 09 00 06 01 00 00` at
/// `$283524`: effect `$01`, stat `$05` (attack), tgt 9, pow 0, res `$06`
/// (defense), el `1` (physical).
const EARTHQUAKE: u8 = 0x38;

/// `EnemySkillData` `$3F` FLODBREATH, record `01 05 08 14 06 01 00 00` at
/// `$28355C`: effect `$01`, stat `$05` (attack), tgt 8, pow 20, res `$06`
/// (defense), el `1` (physical).
const FLODBREATH: u8 = 0x3F;

/// `EnemySkillData` `$40` WAT, record `01 82 08 18 07 05 00 00` at `$283564`:
/// effect `$01`, byte 1 `$82` — selector 2 (mental) — tgt 8, pow 24, res `$07`
/// (magic defense), el `5` (water/ice).
const WAT: u8 = 0x40;

/// `EnemySkillData` `$44` FOI, record `01 82 08 14 07 03 00 00` at `$283584`:
/// effect `$01`, byte 1 `$82` — selector 2 (mental) — tgt 8, pow 20, res `$07`
/// (magic defense), el `3` (fire).
const FOI: u8 = 0x44;

/// `EnemySkillData` `$6D` ROUND EYES, record `01 01 08 00 06 01 00 00` at
/// `$2836CC`: effect `$01`, stat `$01` (strength), tgt 8, pow 0, res `$06`
/// (defense), el `1` (physical).
const ROUND_EYES: u8 = 0x6D;

/// `EnemySkillData` `$6E` LOVEL EYES, record `01 05 08 20 06 01 00 00` at
/// `$2836D4`: effect `$01`, stat `$05` (attack), tgt 8, pow 32, res `$06`
/// (defense), el `1` (physical).
const LOVEL_EYES: u8 = 0x6E;

/// One `(enemy, ability)` pair whose `EnemyAttack_*` arm has been traced to a
/// proven damage-request shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DamageRoute {
    /// `stats.enemy_id` of the carrier: the `fighter_id` `Enemy_Attack`
    /// (`ps4.asm:19173`) indexes `EnemyAttackOffs` with.
    enemy_id: u16,
    /// Raw ability byte the arm runs for, as `$24(a4)` holds it.
    ability: u8,
    /// What the traced chain does with `move.w #$C`.
    class: DamageClass,
}

/// Every `(enemy, ability)` pair whose arm has been traced, with the class the
/// trace established. The `Single` entries make one `$38` request, the three
/// `AllParty` entries at the end of the table make the five-slot one; the class
/// decides how the resolver above walks its targets, and both classes share the
/// per-target arithmetic.
///
/// **Acid Breath `$33`.** `EnemyAttackOffs` (`ps4.asm:19206`) gives
/// `EnemyAttack_FlattrPlnt` to enemy ids `$4B`, `$4C` and `$4D` — 75 FlattrPlnt,
/// 76 FlyScreamr and 77 TechPlant — and `EnemyAttack_Piercer`
/// (`ps4.asm:21518`) to `$55` and `$56`, 85 Piercer and 86 HakenLeft. The gate
/// is the proven *carrier* set rather than the routine set: a routine can only
/// run for an enemy that rolled `$33`, and 77 TechPlant's US ability list is
/// `$2A`/`$2E` only (`generated/enemies.json`), so it never reaches the `$33`
/// arm even though it shares the routine.
///
/// `EnemyAttack_FlattrPlnt` (`ps4.asm:21778`) keeps `Current_Target_Index` for
/// `$33`: the `loc_F5BE` arm has no write to it, while `$34` (`loc_F60A`),
/// `$2A` (`loc_F65E`) and the fallback (`loc_F722`) all clear it. The arm
/// converts the enemy's own attack object into `BattleObj_AcidBreath`
/// (`ps4.asm:38566`) and loads `BattleObj_AcidBreathChild` (`ps4.asm:38622`).
/// The child only animates and asks for the hit reaction
/// (`move.w #5, $2(a3)` / `$1C = $E`); the main object's `loc_24AEC` exit
/// (`ps4.asm:48507`) is the single damage request, `move.w #$C, $2(a3)` gated
/// on the target's hit timer and waited on through `($FFFF416C)`.
///
/// `EnemyAttack_Piercer`'s `$33` arm (`loc_F2A0`, `ps4.asm:21532`) also leaves
/// `Current_Target_Index` alone, but loads object `$35C` = `loc_23998`
/// (`ps4.asm:47264`) with the chosen party target in `$38(a1)` and then spawns
/// child `$360` = `loc_24FD2` (`ps4.asm:48883`). The child makes the same
/// hit-reaction write, and `loc_23AB6` (`ps4.asm:47344`) makes the same single
/// `move.w #$C, $2(a3)` damage request once the child releases
/// `($FFFFEE80)` — no attack, no status, one reaction and one hit for both
/// arms. Both write MoleAttack `$D5` and then EnemyAttack4 `$D8`.
///
/// `loc_B75A` supplies a normal hit without a chance roll and
/// `Enemy_DamageCharacter` (`ps4.asm:3775`) reads strength, defense and
/// physical resistance through the shared `EnemySkillData` record, so the
/// number depends on the caster's strength and not on which arm ran.
///
/// **FLAME BOLT `$02`.** `EnemyAttack_ForcedFly` (`ps4.asm:23567`) branches to
/// `EnemyAttack_MonsterFly` (`ps4.asm:23591`) only for ability 0 and otherwise
/// falls through into `EnemyAttack_Helex` (`ps4.asm:23574`), which writes
/// object `$48` (line 23575) — `BattleObj_HelexFlameBolt` (`ps4.asm:30279`) —
/// and, like both Acid Breath arms, never touches `Current_Target_Index`. That
/// object only animates; on animation frame 2 (`cmpi.b #2, $10(a4)`, line
/// 30294) it loads `BattleObj_HelexFlameBolt2` (`ps4.asm:30315`, the `$4C`
/// write at line 30300) and copies `$38`/`$3C` into it (lines 30301-30302),
/// keeping the target the loading path stored in the parent. The child waits
/// out `$2E` = `$110`, writes the hit reaction (`move.w #5, $2(a3)`,
/// `$1C = $C`, lines 30334-30335) and then makes the single damage request
/// `move.w #$C, $2(a3)` (`ps4.asm:30342`) behind the `btst #1, $4(a4)` /
/// `bset #1, $4(a4)` once-guard, waited on through `($FFFF416C)`. That is one
/// request against the chosen party target, and the arm writes EnemyAttack3
/// `$D7` at the parent's load (line 30285) and FireBreath `$C2` at the child's
/// (line 30321).
///
/// FLAME BOLT's carriers are 0 Helex, whose eight regular slots are all `$02`,
/// and 5 ForcedFly, whose slots 5-8 are `$02` and whose lower slots are zero
/// (`generated/enemies.json`): ForcedFly's zero roll takes the
/// `EnemyAttack_MonsterFly` branch, so `$02` is the only nonzero ability either
/// carrier can dispatch.
///
/// **GIWAT `$2E`.** One arm per routine, all loading a target-carrying object:
///
/// - 71 FrostSaber (`EnemyAttack_ShadowSabr`, `ps4.asm:21933`): `loc_F8F2`
///   (`ps4.asm:21987`) writes object `$2B4` (line 22003) = `loc_1D5AA`
///   (`ps4.asm:39951`; the `$2B4` entry of `BattleObjsGroup4Ptrs` is line 37748)
///   with `$38`; its
///   phase `loc_1D6A0` makes the one request at line 40042.
/// - 77 TechPlant (`EnemyAttack_FlattrPlnt`): `loc_F6C4` (`ps4.asm:21846`)
///   writes `$2FC` (line 21861) = `BattleObj_EnemyGiwat` (`ps4.asm:38195`) plus
///   the visual child `$2F0` (line 21867) = `BattleObj_AcidBreathChild`
///   (`ps4.asm:38622`). The phase table `loc_1BE1C` (`ps4.asm:38212`) jumps to
///   `loc_24AEC` (line 38215): the one request at line 48513.
/// - 91 HewGilla (`EnemyAttack_HewGilla`, `ps4.asm:21395`): the routine tests
///   only `$3F` (line 21400) and `$40` (line 21418), so `$2E` takes the else
///   arm `loc_F108` (`ps4.asm:21426`), writing `$390` (line 21430) =
///   `loc_22670` (`ps4.asm:45925`) with `$38`. Its phase table `loc_226A2`
///   (`ps4.asm:45940`) branches to `loc_24B20` (line 45943): the one request at
///   line 48529.
/// - 101 DarkWitch (`EnemyAttack_TechUser`, `ps4.asm:21156`): `loc_EE0E`
///   (`ps4.asm:21229`) writes `$3C4` (line 21244) = `loc_21852`
///   (`ps4.asm:45013`) with `$38`. Its phase table `loc_21886`
///   (`ps4.asm:45026`) sends phase 8 to the shared `loc_21D08`
///   (`ps4.asm:45302`): one request at line 45310, target `movea.l $38(a4), a3`
///   (line 45307).
/// - 122 DElmLars and 123 XeAThoul (`EnemyAttack_DElmLars`, `ps4.asm:20222`):
///   `loc_DFBE` (`ps4.asm:20300`) writes `$7B8` (line 20315) = `loc_28F76`
///   (`ps4.asm:54226`) with `$38`. Its phase table `loc_28FAC`
///   (`ps4.asm:54239`) jumps to `loc_24A6C` (line 54242): the one request at
///   line 48474.
///
/// **SAND STORM `$37` and MAELSTROM `$39`.** Record target byte 9 — the
/// all-party *nibble* — but both chains make exactly one request. 81 DesrtLeach
/// (`EnemyAttack_SandWorm`, `ps4.asm:21658`) takes `loc_F48A`
/// (`ps4.asm:21692`), writing `$328` (line 21707) = `BattleObj_SandStorm`
/// (`ps4.asm:48204`); 82 Leviathan takes the else arm `loc_F4FA`
/// (`ps4.asm:21720`), writing `$338` (line 21733) = `BattleObj_Maelstrom`
/// (`ps4.asm:47766`). Each phase table (`loc_246A6` `ps4.asm:48215`,
/// `loc_240C8` `ps4.asm:47778`) branches to `loc_24B64` (lines 48217 and
/// 47780): one request at line 48547 against `$38(a4)`, the target both arms
/// stored. `loc_F4D4` — the `$38` EARTHQUAKE arm that clears
/// `Current_Target_Index` and loads the five-slot `$330` object — is not part
/// of either route.
///
/// **FLODBREATH `$3F`.** 90 Depcen (`EnemyAttack_Ismounos`, `ps4.asm:21434`)
/// has no per-id arm: `tst.w $24(a4)` (line 21439) sends every nonzero ability
/// to `loc_F13C` (`ps4.asm:21443`), which writes `$380` (line 21451) =
/// `BattleObj_FlodBreath` (`ps4.asm:46319`). 91 HewGilla and 92 Elmelew share
/// `EnemyAttack_HewGilla`'s `$3F` arm (test at line 21400), writing `$384`
/// (line 21409) = `loc_22A90` (`ps4.asm:46199`). Both phase tables
/// (`loc_22C8E` `ps4.asm:46333`, `loc_22ABA` `ps4.asm:46211`) branch to
/// `loc_24B20`: one request at line 48529. Their chains write MoleAttack `$D5`
/// as the wind-up starts and EnemyAttack4 `$D8` before that request.
///
/// **WAT `$40`.** 91 HewGilla and 92 Elmelew take `loc_F0CC`
/// (`ps4.asm:21412`, the `$40` test at line 21418), writing `$388`
/// (line 21423) = `BattleObj_EnemyWat` (`ps4.asm:45978`), whose phase table
/// `loc_22766` (`ps4.asm:45992`) branches to `loc_24B20` at line 45995 — one
/// request at line 48529. 99 TechUser and 100 TechMaster take `loc_EDC4`
/// (`ps4.asm:21211`), writing `$3C0` (line 21226) = `loc_218D6`
/// (`ps4.asm:45047`), whose phase table `loc_2190A` (`ps4.asm:45060`) sends
/// phase 8 to `loc_21D08` — the one request at line 45310. 114 Juza takes
/// `loc_E3DE` (`ps4.asm:20593`), writing `$744` (line 20608) = `loc_2B006`
/// (`ps4.asm:56437`), whose phase table `loc_2B036` (`ps4.asm:56449`) sends
/// phase 8 to `loc_2B192` (line 56452) — the one request at line 56553.
///
/// **FOI `$44`.** 99 TechUser and 100 TechMaster take `EnemyAttack_TechUser`'s
/// first arm (test at line 21157), writing `$3B0` (line 21171) = `loc_21BF0`
/// (`ps4.asm:45229`); its phase table `loc_21C24` (`ps4.asm:45242`) sends
/// phase 8 to the same shared `loc_21D08`. 114 Juza takes
/// `EnemyAttack_Juza`'s first arm (`ps4.asm:20575`, test at line 20576),
/// writing `$740` (line 20590) = `loc_2B08E` (`ps4.asm:56477`), whose phase
/// table `loc_2B0BE` (`ps4.asm:56489`) sends phase 8 to the same `loc_2B192`.
/// Both therefore make their one request at the shared tails' lines (45310 and
/// 56553), and both chains write MoleAttack `$D5` at the wind-up and TechCast
/// `$BB` as the request phase starts.
///
/// **ROUND EYES `$6D` and LOVEL EYES `$6E`.** `EnemyAttack_Rappy`
/// (`ps4.asm:19578`) turns the attack object itself into the ability object:
/// nonzero, `$6D` writes `$8F8` (line 19590) = `BattleObj_RoundEyes`
/// (`ps4.asm:67760`), everything else writes `$8FC` (line 19593) =
/// `BattleObj_LovelEyes` (`ps4.asm:67729`), and both arms then reach
/// `loc_D200` (`ps4.asm:19395`) to set the object's `parent`. Their phase
/// tables (`loc_347B0` `ps4.asm:67772`, `loc_3474E` `ps4.asm:67741`) jump to
/// `loc_24B20` at phase 8 (lines 67775 and 67744): one request at line 48529
/// against `$38`, the target `Enemy_Attack` stored in the object. 148 BlueRappy
/// has no `$6D` in its list and 147 Rappy no `$6E`, so each pair owns its arm.
const DAMAGE_SKILL_ROUTES: &[DamageRoute] = &[
    // `EnemyAttackOffs` `$4B` (`ps4.asm:19282`) → `EnemyAttack_FlattrPlnt`
    // (`ps4.asm:21778`), `$33` arm `loc_F5BE` → BattleObj_AcidBreath
    // (`ps4.asm:38566`); one request at `loc_24AEC` (`ps4.asm:48507`).
    DamageRoute {
        enemy_id: 75,
        ability: ACID_BREATH,
        class: DamageClass::Single,
    },
    // `EnemyAttackOffs` `$4C` (`ps4.asm:19283`) → the same
    // `EnemyAttack_FlattrPlnt` arm and object.
    DamageRoute {
        enemy_id: 76,
        ability: ACID_BREATH,
        class: DamageClass::Single,
    },
    // `EnemyAttackOffs` `$55` (`ps4.asm:19292`) → `EnemyAttack_Piercer`
    // (`ps4.asm:21518`), `$33` arm `loc_F2A0` → object `$35C` = `loc_23998`
    // (`ps4.asm:47264`); one request at `loc_23AB6` (`ps4.asm:47344`).
    DamageRoute {
        enemy_id: 85,
        ability: ACID_BREATH,
        class: DamageClass::Single,
    },
    // `EnemyAttackOffs` `$56` (`ps4.asm:19293`) → the same
    // `EnemyAttack_Piercer` arm and object.
    DamageRoute {
        enemy_id: 86,
        ability: ACID_BREATH,
        class: DamageClass::Single,
    },
    // `EnemyAttackOffs` `$00` (`ps4.asm:19207`) → `EnemyAttack_Helex`
    // (`ps4.asm:23574`) → object `$48` = BattleObj_HelexFlameBolt
    // (`ps4.asm:30279`), child `$4C` = BattleObj_HelexFlameBolt2
    // (`ps4.asm:30315`); one request at `ps4.asm:30342`.
    DamageRoute {
        enemy_id: 0,
        ability: FLAME_BOLT,
        class: DamageClass::Single,
    },
    // `EnemyAttackOffs` `$05` (`ps4.asm:19212`) → `EnemyAttack_ForcedFly`
    // (`ps4.asm:23567`) falls through into `EnemyAttack_Helex` for a nonzero
    // ability: the same object chain and the same single request.
    DamageRoute {
        enemy_id: 5,
        ability: FLAME_BOLT,
        class: DamageClass::Single,
    },
    // 71 FrostSaber, `$2E` GIWAT: `EnemyAttackOffs` `$47` (`ps4.asm:19278`) →
    // `EnemyAttack_ShadowSabr` (`ps4.asm:21933`), arm `loc_F8F2`
    // (`ps4.asm:21987`) writing object `$2B4` (line 22003) = `loc_1D5AA`
    // (`ps4.asm:39951`) with the drawn target; one request at line 40042
    // behind `btst #3, $4(a4)`, target `movea.l $38(a4), a3` (line 40039).
    DamageRoute {
        enemy_id: 71,
        ability: GIWAT,
        class: DamageClass::Single,
    },
    // 77 TechPlant, `$2E` GIWAT: `EnemyAttackOffs` `$4D` (`ps4.asm:19284`) →
    // `EnemyAttack_FlattrPlnt` arm `loc_F6C4` (`ps4.asm:21846`), writing `$2FC`
    // (line 21861) = `BattleObj_EnemyGiwat` (`ps4.asm:38195`) plus the visual
    // child `$2F0` (line 21867); `jmp loc_24AEC` at line 38215, one request at
    // line 48513.
    DamageRoute {
        enemy_id: 77,
        ability: GIWAT,
        class: DamageClass::Single,
    },
    // 91 HewGilla, `$2E` GIWAT: `EnemyAttackOffs` `$5B` (`ps4.asm:19298`) →
    // `EnemyAttack_HewGilla` (`ps4.asm:21395`); `$2E` is the else arm
    // `loc_F108` (`ps4.asm:21426`), writing `$390` (line 21430) =
    // `loc_22670` (`ps4.asm:45925`); `bra.w loc_24B20` at line 45943, one
    // request at line 48529.
    DamageRoute {
        enemy_id: 91,
        ability: GIWAT,
        class: DamageClass::Single,
    },
    // 101 DarkWitch, `$2E` GIWAT: `EnemyAttackOffs` `$65` (`ps4.asm:19308`) →
    // `EnemyAttack_TechUser` (`ps4.asm:21156`), arm `loc_EE0E`
    // (`ps4.asm:21229`) writing `$3C4` (line 21244) = `loc_21852`
    // (`ps4.asm:45013`); phase 8 reaches `loc_21D08` (`ps4.asm:45302`), one
    // request at line 45310.
    DamageRoute {
        enemy_id: 101,
        ability: GIWAT,
        class: DamageClass::Single,
    },
    // 122 DElmLars, `$2E` GIWAT: `EnemyAttackOffs` `$7A` (`ps4.asm:19329`) →
    // `EnemyAttack_DElmLars` (`ps4.asm:20222`), arm `loc_DFBE`
    // (`ps4.asm:20300`) writing `$7B8` (line 20315) = `loc_28F76`
    // (`ps4.asm:54226`); `jmp loc_24A6C` at line 54242, one request at
    // line 48474.
    DamageRoute {
        enemy_id: 122,
        ability: GIWAT,
        class: DamageClass::Single,
    },
    // 123 XeAThoul, `$2E` GIWAT: `EnemyAttackOffs` `$7B` (`ps4.asm:19330`) →
    // the same `EnemyAttack_DElmLars` arm, object and request.
    DamageRoute {
        enemy_id: 123,
        ability: GIWAT,
        class: DamageClass::Single,
    },
    // 81 DesrtLeach, `$37` SAND STORM: `EnemyAttackOffs` `$51`
    // (`ps4.asm:19288`) → `EnemyAttack_SandWorm` (`ps4.asm:21658`), arm
    // `loc_F48A` (`ps4.asm:21692`) writing `$328` (line 21707) =
    // `BattleObj_SandStorm` (`ps4.asm:48204`); `bra.w loc_24B64` at line 48217,
    // one request at line 48547.
    DamageRoute {
        enemy_id: 81,
        ability: SAND_STORM,
        class: DamageClass::Single,
    },
    // 82 Leviathan, `$39` MAELSTROM: `EnemyAttackOffs` `$52`
    // (`ps4.asm:19289`) → `EnemyAttack_SandWorm`'s else arm `loc_F4FA`
    // (`ps4.asm:21720`), writing `$338` (line 21733) =
    // `BattleObj_Maelstrom` (`ps4.asm:47766`); `bra.w loc_24B64` at line 47780,
    // one request at line 48547.
    DamageRoute {
        enemy_id: 82,
        ability: MAELSTROM,
        class: DamageClass::Single,
    },
    // 90 Depcen, `$3F` FLODBREATH: `EnemyAttackOffs` `$5A`
    // (`ps4.asm:19297`) → `EnemyAttack_Ismounos` (`ps4.asm:21434`), whose
    // nonzero arm `loc_F13C` (`ps4.asm:21443`) writes `$380` (line 21451) =
    // `BattleObj_FlodBreath` (`ps4.asm:46319`); `bra.w loc_24B20` at line
    // 46335, one request at line 48529.
    DamageRoute {
        enemy_id: 90,
        ability: FLODBREATH,
        class: DamageClass::Single,
    },
    // 91 HewGilla, `$3F` FLODBREATH: `EnemyAttackOffs` `$5B`
    // (`ps4.asm:19298`) → `EnemyAttack_HewGilla`'s `$3F` arm (test at line
    // 21400), writing `$384` (line 21409) = `loc_22A90` (`ps4.asm:46199`);
    // `bra.w loc_24B20` at line 46213, one request at line 48529.
    DamageRoute {
        enemy_id: 91,
        ability: FLODBREATH,
        class: DamageClass::Single,
    },
    // 92 Elmelew, `$3F` FLODBREATH: `EnemyAttackOffs` `$5C`
    // (`ps4.asm:19299`) → the same `EnemyAttack_HewGilla` arm, object and
    // request.
    DamageRoute {
        enemy_id: 92,
        ability: FLODBREATH,
        class: DamageClass::Single,
    },
    // 91 HewGilla, `$40` WAT: `EnemyAttack_HewGilla`'s `$40` arm `loc_F0CC`
    // (`ps4.asm:21412`, test at line 21418) writes `$388` (line 21423) =
    // `BattleObj_EnemyWat` (`ps4.asm:45978`); `bra.w loc_24B20` at line 45995,
    // one request at line 48529.
    DamageRoute {
        enemy_id: 91,
        ability: WAT,
        class: DamageClass::Single,
    },
    // 92 Elmelew, `$40` WAT: `EnemyAttackOffs` `$5C` (`ps4.asm:19299`) → the
    // same arm and object.
    DamageRoute {
        enemy_id: 92,
        ability: WAT,
        class: DamageClass::Single,
    },
    // 99 TechUser, `$40` WAT: `EnemyAttackOffs` `$63` (`ps4.asm:19306`) →
    // `EnemyAttack_TechUser`'s `$40` arm `loc_EDC4` (`ps4.asm:21211`), writing
    // `$3C0` (line 21226) = `loc_218D6` (`ps4.asm:45047`); phase 8 reaches
    // `loc_21D08`, one request at line 45310.
    DamageRoute {
        enemy_id: 99,
        ability: WAT,
        class: DamageClass::Single,
    },
    // 100 TechMaster, `$40` WAT: `EnemyAttackOffs` `$64` (`ps4.asm:19307`) →
    // the same `EnemyAttack_TechUser` arm and object.
    DamageRoute {
        enemy_id: 100,
        ability: WAT,
        class: DamageClass::Single,
    },
    // 114 Juza, `$40` WAT: `EnemyAttackOffs` `$72` (`ps4.asm:19321`) →
    // `EnemyAttack_Juza` (`ps4.asm:20575`), arm `loc_E3DE` (`ps4.asm:20593`)
    // writing `$744` (line 20608) = `loc_2B006` (`ps4.asm:56437`); phase 8
    // reaches `loc_2B192` (line 56452), one request at line 56553.
    DamageRoute {
        enemy_id: 114,
        ability: WAT,
        class: DamageClass::Single,
    },
    // 99 TechUser, `$44` FOI: `EnemyAttack_TechUser`'s first arm (test at
    // line 21157) writes `$3B0` (line 21171) = `loc_21BF0`
    // (`ps4.asm:45229`); phase 8 reaches `loc_21D08`, one request at
    // line 45310.
    DamageRoute {
        enemy_id: 99,
        ability: FOI,
        class: DamageClass::Single,
    },
    // 100 TechMaster, `$44` FOI: `EnemyAttackOffs` `$64` (`ps4.asm:19307`) →
    // the same `EnemyAttack_TechUser` arm and object.
    DamageRoute {
        enemy_id: 100,
        ability: FOI,
        class: DamageClass::Single,
    },
    // 114 Juza, `$44` FOI: `EnemyAttack_Juza`'s first arm (`ps4.asm:20575`,
    // test at line 20576) writes `$740` (line 20590) = `loc_2B08E`
    // (`ps4.asm:56477`); phase 8 reaches `loc_2B192`, one request at
    // line 56553.
    DamageRoute {
        enemy_id: 114,
        ability: FOI,
        class: DamageClass::Single,
    },
    // 147 Rappy, `$6D` ROUND EYES: `EnemyAttackOffs` `$93`
    // (`ps4.asm:19354`) → `EnemyAttack_Rappy` (`ps4.asm:19578`), whose
    // nonzero arm `loc_D4BE` (`ps4.asm:19583`) writes object `$8F8`
    // (line 19590) = `BattleObj_RoundEyes` (`ps4.asm:67760`) into the attack
    // object itself and then reaches `loc_D200` (`ps4.asm:19395`) for the
    // parent pointer; phase 8 jumps to `loc_24B20` (line 67775), one request
    // at line 48529.
    DamageRoute {
        enemy_id: 147,
        ability: ROUND_EYES,
        class: DamageClass::Single,
    },
    // 148 BlueRappy, `$6E` LOVEL EYES: `EnemyAttackOffs` `$94`
    // (`ps4.asm:19355`) → the same routine's else arm `loc_D4DE`
    // (`ps4.asm:19592`), writing object `$8FC` (line 19593) =
    // `BattleObj_LovelEyes` (`ps4.asm:67729`); phase 8 jumps to `loc_24B20`
    // (line 67744), one request at line 48529.
    DamageRoute {
        enemy_id: 148,
        ability: LOVEL_EYES,
        class: DamageClass::Single,
    },
    // 15 Fanbite, `$08` SPIRAL BLD — the first all-party route.
    // `EnemyAttackOffs` `$0F` (`ps4.asm:19222`) → `EnemyAttack_Locusta`
    // (`ps4.asm:23472`). The arm has no ability-id test: it loads the three
    // approach objects (`$98`/`$9C`/`$A4`), then `tst.w ability(a4)` at line
    // 23486 picks the nonzero-ability body, which clears
    // `Current_Target_Index` (line 23488), loads object `$A0` into the second
    // object bank (`ps4.asm:26770`) = `BattleObj_LocustaSpiralBld`
    // (`ps4.asm:29175`) and copies the stored target pointer into its `$38`
    // (line 23491). Fanbite's regular list is `$08` in slots 7-8 and zeros
    // elsewhere, so every nonzero roll runs this body.
    //
    // The object's state 4 (`loc_1468A`, `ps4.asm:29259`) walks in until
    // `$2C(a4) >= $1BF` and then ORs the five party slots' `$1C` timers
    // (lines 29266-29274); only when all of them read zero does it write
    // `#$C` to the five slots from `$FF4400` (lines 29275-29280) and set
    // `($FFFF416C)`. While it walks, `loc_146F2` also flinches each live
    // member it passes (`$1C = $C`, routine 5, line 14756) in the `loc_147A0`
    // order, which is what the `$1C` gate is waiting for; the flinch is not a
    // damage request.
    DamageRoute {
        enemy_id: 15,
        ability: SPIRAL_BLD,
        class: DamageClass::AllParty,
    },
    // 80 SandWorm, `$38` EARTHQUAKE. `EnemyAttackOffs` `$50`
    // (`ps4.asm:19287`) → `EnemyAttack_SandWorm` (`ps4.asm:21658`); `loc_F4D4`
    // (`ps4.asm:21710`) tests `$38` (line 21711), clears
    // `Current_Target_Index` (line 21713) and writes object `$330` into the
    // attack object itself (line 21718) — `BattleObjsGroup5Ptrs` line 43704 =
    // `BattleObj_Earthquake` (`ps4.asm:47884`). SandWorm lists `$38` in slots
    // 6-8 only, and `loc_F48A` owns `$37` and `loc_F4FA` the else arm, so this
    // body is `$38`'s alone.
    //
    // The object's state table (`loc_2427A`, `ps4.asm:47901`) reaches
    // `loc_243C8` (line 47992) for the request: it writes `#$C` to the five
    // slots from `Obj_Fighters` (lines 47995-48000) and sets `($FFFF416C)`,
    // then waits for the flag to clear (line 48003) before its follow-through
    // phase. Unlike the two tails and the Fanbite object it does not
    // test the `$1C` timers, and `Enemy_Attack` cleared them for all five
    // slots at line 19143 anyway.
    DamageRoute {
        enemy_id: 80,
        ability: EARTHQUAKE,
        class: DamageClass::AllParty,
    },
    // 149 KingRappy, `$38` EARTHQUAKE. `EnemyAttackOffs` `$95`
    // (`ps4.asm:19356`) → `EnemyAttack_KingRappy` (`ps4.asm:19596`), whose
    // nonzero arm `loc_D516` (line 19608) clears `Current_Target_Index`
    // (line 19609) and writes object `$904` (line 19610) —
    // `BattleObjsGroup10Ptrs` line 66419 = `BattleObj_KingRappyEarthquake`
    // (`ps4.asm:67513`). KingRappy's regular list is `$38` in slots 3-4 and
    // zeros elsewhere, so every nonzero roll takes that arm.
    //
    // The object's state table (`loc_34428`, `ps4.asm:67523`) sends states 0,
    // 4 and 8 through its own wind-up — which loads sound object `$8EC`
    // (line 67541, with `SFXID_Slasher` at line 67544), writes
    // `SFXID_GraveOpening` (line 67567) and clears the party's `$1C` timers
    // (lines 67603-67607) — and state `$C` straight to `jmp (loc_24BB6).l`
    // (line 67527), the shared all-party tail at `ps4.asm:48562`: its `$1C`
    // gate at lines 48565-48571, the five `#$C` writes at lines 48572-48577 and
    // `($FFFF416C)` at line 48578.
    DamageRoute {
        enemy_id: 149,
        ability: EARTHQUAKE,
        class: DamageClass::AllParty,
    },
];

/// The class the table has proven for this exact pair, if any.
fn proven(enemy_id: u16, ability: u8) -> Option<DamageClass> {
    DAMAGE_SKILL_ROUTES
        .iter()
        .find(|route| route.enemy_id == enemy_id && route.ability == ability)
        .map(|route| route.class)
}

/// The traced damage requests of one route, for every pair in
/// [`DAMAGE_SKILL_ROUTES`]: one request against the chosen party target
/// ([`DamageClass::Single`]) or one per party slot ([`DamageClass::AllParty`]).
///
/// Returns `false`, leaving the ability to
/// [`BattleEvent::UnsupportedAbility`], when the `(enemy, ability)` pair is not
/// in the table, when the ability has no record, when the record's effect byte
/// is not `$01` (`AbilityEffect_None`, so its handler would do more than the
/// request this models), or when the actor is not a living enemy. Nothing is
/// drawn and no event is emitted on any of those paths, so the caller's
/// fallback starts from the state the ability roll left behind.
///
/// A resolved skill emits [`BattleEvent::EnemySkillUsed`] first and then one
/// [`BattleEvent::Resolved`] per target that is on the party side and alive,
/// each carrying its own clamped damage and the fighter's remaining hit points
/// — plus [`BattleEvent::Died`] when that reaches zero. The `Single` class
/// resolves the `intended` target alone; the `AllParty` class ignores
/// `intended` (its arm cleared `Current_Target_Index` before loading the
/// object) and walks every occupied, living party slot in slot order, which is
/// the order `Battle_UpdateFighters` (`ps4.asm:987`) runs their `$C` routines
/// in.
pub(super) fn resolve_damage_skill(
    roster: &mut Roster,
    actor: FighterId,
    ability: u8,
    intended: Option<FighterId>,
    data: &BattleData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> bool {
    let Some(skill) = data
        .enemy_skill(ability)
        .filter(|s| s.effect == EFFECT_NONE)
    else {
        return false;
    };
    let Some(caster) = roster
        .get(actor)
        .filter(|f| f.is_alive() && f.id.side() == Side::Enemy)
    else {
        return false;
    };
    // The table's class selects the shape resolved below. The match is
    // exhaustive, so a new class fails to compile until it has a branch here.
    let Some(class) = proven(caster.stats.enemy_id, ability) else {
        return false;
    };
    // `Effect_SetupSkillParams` (`ps4.asm:9576`) masks the record's stat byte
    // with `$7F` (line 9580) before indexing `AbilityStatsOffs`, which is how a
    // record written as `$82` selects mental. Byte 4 is read raw there (line
    // 9604) and in `Enemy_DamageCharacter`'s enemy-skill branch, so only the
    // power stat is masked. Both readers take selectors `$05`..`$07` as words
    // (`move.w (a1,d1.w), d1`, table offsets `atk_pow_battle` and above) and
    // `$01`..`$04` as bytes, which is what [`super::technique::stat`] returns.
    let power = super::technique::stat(&caster.stats, skill.power_stat & super::STAT_INDEX_MASK);
    events.push(BattleEvent::EnemySkillUsed {
        actor,
        skill: ability,
        name: skill.name.clone(),
    });
    match class {
        // One request against the object's `$38`, the drawn
        // `Current_Target_Index`.
        DamageClass::Single => {
            if let Some(target) = intended.filter(|id| id.side() == Side::Party) {
                damage_one_target(roster, actor, skill, power, target, rolls, events);
            }
        }
        // Five requests in one frame, one per occupied party slot that is
        // still standing, in slot order. `intended` plays no part: the arm
        // cleared `Current_Target_Index` before loading its object and the
        // loop runs over `Obj_Fighters` itself.
        DamageClass::AllParty => {
            let targets: Vec<FighterId> = roster
                .side(Side::Party)
                .filter(|f| f.is_alive())
                .map(|f| f.id)
                .collect();
            for target in targets {
                damage_one_target(roster, actor, skill, power, target, rolls, events);
            }
        }
    }
    true
}

/// One target's `Enemy_DamageCharacter` call: its own defense, its own element
/// factor and its own sixteen draws, then the hit points and the events.
///
/// The single-target branch reaches this through the `$38` request and the
/// all-party branch through one of the five writes; both compute the same
/// number for the same target, because neither the request nor the loop carries
/// any state the formula reads.
fn damage_one_target(
    roster: &mut Roster,
    actor: FighterId,
    skill: &super::EnemySkill,
    power: u16,
    target: FighterId,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) {
    let Some(fighter) = roster.get_mut(target).filter(|f| f.is_alive()) else {
        return;
    };
    let damage = super::clamp_damage(super::calculate_damage(
        power,
        super::technique::stat(&fighter.stats, skill.resistance),
        u16::from(fighter.stats.element_factor(skill.element).unwrap_or(0)),
        u16::from(skill.power),
        rolls,
    ));
    fighter.stats.curr_hp = fighter.stats.curr_hp.saturating_sub(damage);
    events.push(BattleEvent::Resolved {
        actor,
        target,
        verdict: super::Verdict::Normal,
        damage: Some(damage),
        remaining_hp: fighter.stats.curr_hp,
    });
    if fighter.stats.curr_hp == 0 {
        fighter.mark_defeated();
        events.push(BattleEvent::Died { fighter: target });
    }
}
