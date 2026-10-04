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
//! [`DAMAGE_SKILL_ROUTES`] — one slice per carrier family, in [`routes`] — is
//! therefore the whole gate: one `(enemy, ability)` pair per arm whose request
//! shape has been read out of the disassembly. The pair proves the *route*: its
//! [`DamageClass`] — one `$38` request, or the five-slot all-party loop — and
//! the [`ObjectDraws`] its chain takes off the shared stream while it animates;
//! the record still drives the arithmetic, and no record-byte predicate is part
//! of the proof beyond the effect handler: the proven no-op indices `$01`,
//! `$20`, `$23` and `$24` all reach `AbilityEffect_None` (`ps4.asm:9092`). A real
//! effect needs its own implementation and stays on the explicit
//! [`BattleEvent::UnsupportedAbility`] path.
//! Byte 2's target nibble is *not* a gate — it picks the
//! `Ability_ProcessRange` (`ps4.asm:8975`) handler for the effect, and the
//! damage request the object makes comes from the object chain alone.
//!
//! Anything outside the table keeps the explicit
//! [`BattleEvent::UnsupportedAbility`] path. That is a statement about the
//! *whole* turn, not just the arithmetic: an ability whose chain makes no
//! `move.w #$C` request at all — `$4C` EVIL EYE, `$2A` RIMIT and `$2F` VOL on
//! every carrier read so far, whose effect handlers `$07` and `$02` do the
//! work instead — has no row here, because a row would claim a damage request
//! the cartridge never makes. See `docs/battle/ENEMY_DAMAGE_ROUTES.md` §2 for
//! those rows and which lane owns their handlers.

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

#[cfg(test)]
#[path = "enemy_damage_firebreath_tests.rs"]
mod firebreath_tests;

#[cfg(test)]
#[path = "enemy_damage_zio_tests.rs"]
mod zio_tests;

#[cfg(test)]
#[path = "enemy_damage_flaeli_tests.rs"]
mod flaeli_tests;

#[cfg(test)]
#[path = "enemy_damage_route_tests.rs"]
mod route_tests;

/// The proven damage records' no-op `AbilityEffectsOffs` indices
/// (`ps4.asm:9036`): each reaches `AbilityEffect_None`, a bare `rts`.
///
/// The routes below model the damage request and nothing else, so a record
/// whose effect handler would also do something is not guessed at: it stays on
/// the unsupported path until that handler is implemented. Other no-op
/// indices are not admitted without a proven damage record/chain pair.
fn effect_is_none(effect: u8) -> bool {
    // `AbilityEffectsOffs` (ps4.asm:9036-9088): $20 (CHARGCNNON) and
    // $23 (EXPLOSION) and $24 (DETONATION) also point at the bare rts
    // at AbilityEffect_None (table lines 9076/9079/9080).
    matches!(effect, 0x01 | 0x20 | 0x23 | 0x24)
}

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
    /// FloatMine's `loc_18574` (ps4.asm:33694-33724): one stored-target
    /// request, then clears the actor's object and reaction byte. Cached HP
    /// is not changed and the reward routine is not called.
    SingleRemoveActor,
    /// Five requests, one per party slot, from a `moveq #4, dN` loop over
    /// `Obj_Fighters`: the shared tails `loc_24A9E` (`ps4.asm:48483`) and
    /// `loc_24BB6` (`ps4.asm:48562`), plus the objects that inline the same
    /// loop — `BattleObj_LocustaSpiralBld` (`ps4.asm:29275-29280`),
    /// `BattleObj_Earthquake` (`ps4.asm:47995-48000`), the Zio family's
    /// `loc_2AFB6` (`ps4.asm:56413-56431`, which both `$47` ZAN and `$56`
    /// FORCEFLASH jump into), TechMaster's `loc_21674`
    /// (`ps4.asm:44880-44898`) and CORRSION's `loc_2C03A`
    /// (`ps4.asm:57574-57593`).
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
    /// CommndBall's `loc_17C9E` (ps4.asm:33114-33127) requests the party
    /// once. `loc_17D4A` (33154-33177) then clears the next and previous
    /// enemy objects, retaining the actor, cached stats and reward pools.
    AllPartyRemoveNeighbours,
}

/// The calls a route's object chain makes on the shared stream *before* its
/// damage request.
///
/// Damage rolls are not the whole of what an ability takes from
/// `UpdateRNGSeed2` (`ps4.asm:86097`). An enemy skill's arm loads a battle
/// object and hands it the turn; that object animates for as long as it likes
/// before its last phase writes the `move.w #$C` the class above describes, and
/// an object that shakes the screen reads the generator to do it — the raster
/// H/V counter is its entropy, so a shake is exactly the kind of thing that
/// draws. The calls land on the *same* 32-bit seed as every other battle and
/// field roll, so they move every later draw whether or not the port models
/// them: a missing one shifts every subsequent roll by one, and the port reads
/// another action's numbers as its own (issue
/// [#33](https://github.com/TusanHomichi/PSIV/issues/33)).
///
/// So the count is part of the route, read out of the chain the same way the
/// class is, and the resolver takes the calls where the object does: after the
/// ability roll and before the first damage run. It depends on nothing the
/// battle state holds — the object runs its fixed frame count whatever the
/// target count, the party's HP or the fight's outcome — which is why it is a
/// per-route constant and not a function.
///
/// The file's every `jsr (UpdateRNGSeed2).l` call site was enumerated for this
/// (43 of them, `docs/source-notes/battle-enemy-abilities.md`, 2026-09-25):
/// `BattleObj_Earthquake` is the only object a proven route's chain reaches
/// that owns one, and no proven arm routine owns one either. Every route below
/// that lists [`ObjectDraws::None`] was checked against that list: its arm, its
/// own region and the objects it loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectDraws {
    /// Nothing in the chain calls the generator: the route's whole draw is the
    /// ability roll and the damage runs.
    None,
    /// `BattleObj_Earthquake`'s shake (`ps4.asm:47884`), the one chain read to
    /// call `UpdateRNGSeed2` — [`EARTHQUAKE_SHAKE_DRAWS`] calls, all of them
    /// before the five-slot request at `ps4.asm:47998`.
    EarthquakeShake,
}

impl ObjectDraws {
    /// How many calls the chain makes before its damage request.
    const fn count(self) -> u16 {
        match self {
            ObjectDraws::None => 0,
            ObjectDraws::EarthquakeShake => EARTHQUAKE_SHAKE_DRAWS,
        }
    }
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

/// The length in frames of `BattleObj_Earthquake`'s shake:
/// `move.b #$3C, $11(a4)` (`ps4.asm:47944`, in the handover at `loc_24308`,
/// line 47935), counted down once a frame by `subq.b #1, $11(a4)`
/// (`ps4.asm:47963`, in `loc_2436A`, line 47962).
const EARTHQUAKE_SHAKE_FRAMES: u16 = 0x3C;

/// How often the shake draws: `loc_2438E` (`ps4.asm:47972`) bumps `$10(a4)` and
/// masks it to two bits, and only the frame that masks to zero reaches the two
/// `jsr (UpdateRNGSeed2).l` calls (lines 47976 and 47983) — one for
/// `Camera_X_Pos_FG` and one for `Camera_X_Pos_BG`.
const EARTHQUAKE_SHAKE_PERIOD: u16 = 4;

/// The calls one `BattleObj_Earthquake` run makes: two, every fourth frame of
/// the shake.
///
/// The frame that empties `$11(a4)` takes the `bne` at `ps4.asm:47964` out of
/// the shake instead, so 59 of the 60 frames run the shake body and the last
/// drawing frame is the 56th: `4, 8, … 56` is fourteen frames, two calls each.
/// The captured Motavia battle that settled this measured exactly that — 28
/// calls, two a frame over f26541–f26593 of
/// `build/sweep-3B/capture/forced_3B_attack_rolls.csv`
/// (`docs/source-notes/battle-enemy-abilities.md`, 2026-09-25).
const EARTHQUAKE_SHAKE_DRAWS: u16 = 2 * ((EARTHQUAKE_SHAKE_FRAMES - 1) / EARTHQUAKE_SHAKE_PERIOD);

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

/// `EnemySkillData` `$21` FIREBREATH, record `01 01 08 20 07 03 00 00` at
/// `$28346C`: effect `$01`, stat `$01` (strength), tgt 8, pow 32, res `$07`
/// (magic defense), el `3` (fire).
const FIREBREATH: u8 = 0x21;

/// `EnemySkillData` `$47` ZAN, record `01 82 09 10 07 01 00 00` at `$28359C`:
/// effect `$01`, byte 1 `$82` — masked to selector 2 (mental) — tgt 9, pow 16,
/// res `$07` (magic defense), el `1` (physical).
const ZAN: u8 = 0x47;

/// `EnemySkillData` `$56` FORCEFLASH, record `01 02 09 24 07 02 00 00` at
/// `$283614`: effect `$01`, stat `$02` (mental), tgt 9, pow 36, res `$07`
/// (magic defense), el `2` (energy).
const FORCEFLASH: u8 = 0x56;

/// `EnemySkillData` `$4D` CORRSION, record `01 02 09 40 07 01 00 00` at
/// `$2835CC`: effect `$01`, stat `$02` (mental), tgt 9, pow 64, res `$07`
/// (magic defense), el `1` (physical).
const CORRSION: u8 = 0x4D;

/// `EnemySkillData` `$4F` HEWN, record `01 02 09 38 07 01 00 00` at
/// `$2835DC`: effect `$01`, stat `$02` (mental), tgt 9, pow 56, res `$07`
/// (magic defense), el `1` (physical).
const HEWN: u8 = 0x4F;

/// `EnemySkillData` `$6C` BLACK WAVE2, record `01 02 08 70 07 01 00 00` at
/// `$2836C4`: effect `$01`, stat `$02` (mental), tgt 8, pow 112, res `$07`
/// (magic defense), el `1` (physical). The weak Black Wave the Psycho Wand's
/// Zio2 casts (`ps4.asm:66908`).
const BLACK_WAVE2: u8 = 0x6C;

mod routes;

use routes::proven;
#[cfg(test)]
pub(super) use routes::{DAMAGE_SKILL_ROUTES, all};

/// The traced damage requests of one route, for every pair in
/// [`DAMAGE_SKILL_ROUTES`]: one request against the chosen party target
/// ([`DamageClass::Single`]) or one per party slot ([`DamageClass::AllParty`]).
///
/// Returns `false`, leaving the ability to
/// [`BattleEvent::UnsupportedAbility`], when the `(enemy, ability)` pair is not
/// in the table, when the ability has no record, when the record's effect byte
/// is not a proven no-op (`AbilityEffect_None`), or when the actor is not a
/// living enemy. Nothing is
/// drawn and no event is emitted on any of those paths, so the caller's
/// fallback starts from the state the ability roll left behind.
///
/// A resolved skill emits [`BattleEvent::EnemySkillUsed`] first, then draws the
/// route's [`ObjectDraws`] — the calls its object makes while it animates —
/// and then one [`BattleEvent::Resolved`] per target that is on the party side
/// and alive, each carrying its own clamped damage and the fighter's remaining
/// hit points — plus [`BattleEvent::Died`] when that reaches zero. The `Single`
/// class resolves the `intended` target alone; the `AllParty` class ignores
/// `intended` (its arm cleared `Current_Target_Index` before loading the
/// object) and walks every occupied, living party slot in slot order, which is
/// the order `Battle_UpdateFighters` (`ps4.asm:987`) runs their `$C` routines
/// in.
///
/// The object's calls are drawn whatever the battle state: the arm loads the
/// object on the ability roll alone, so the shake runs its sixty frames and
/// takes its 28 calls even in a round where no party slot is left to damage.
/// They sit before every damage run, never between them — the request is the
/// object's last phase, and all of its draws are behind it.
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
        .filter(|s| effect_is_none(s.effect))
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
    let Some(route) = proven(caster.stats.enemy_id, ability) else {
        return false;
    };
    let class = route.class;
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
    // The chain's own calls come here, between the ability roll the caller made
    // and the first damage run below: they are the frames the object spends
    // animating before its request, and they move the shared seed like any
    // other draw, so a port that skips them reads another action's numbers as
    // its own. See [`ObjectDraws`] for what makes them a per-route constant.
    for _ in 0..route.draws.count() {
        rolls.next_roll();
    }
    match class {
        // One request against the object's `$38`, the drawn
        // `Current_Target_Index`.
        DamageClass::Single | DamageClass::SingleRemoveActor => {
            if let Some(target) = intended.filter(|id| id.side() == Side::Party) {
                damage_one_target(roster, actor, skill, power, target, rolls, events);
            }
        }
        // Five requests in one frame, one per occupied party slot that is
        // still standing, in slot order. `intended` plays no part: the arm
        // cleared `Current_Target_Index` before loading its object and the
        // loop runs over `Obj_Fighters` itself.
        DamageClass::AllParty | DamageClass::AllPartyRemoveNeighbours => {
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
    let removed = match class {
        DamageClass::SingleRemoveActor => vec![actor],
        DamageClass::AllPartyRemoveNeighbours => {
            [actor.get().checked_add(1), actor.get().checked_sub(1)]
                .into_iter()
                .flatten()
                .filter_map(FighterId::new)
                .filter(|id| id.side() == Side::Enemy)
                .collect()
        }
        DamageClass::Single | DamageClass::AllParty => Vec::new(),
    };
    for id in removed {
        if let Some(fighter) = roster.get_mut(id) {
            let occupied = fighter.active;
            fighter.active = false;
            if class == DamageClass::SingleRemoveActor {
                fighter.reaction_flags = 0;
            }
            // Object removal is visible on the same existing presentation
            // path as an enemy's death; it does not mark cached stats dead.
            if occupied {
                events.push(BattleEvent::Died { fighter: id });
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
