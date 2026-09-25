//! The enemy's own AI instruction block — `Enemy_Attack`'s second half.
//!
//! `Enemy_Attack` rolls a regular ability and writes it, then walks up to four
//! **condition** bytes the record carries at `$50(a3)`..`$53(a3)`, each paired
//! with the **conditional** ability at `$54(a3)`..`$57(a3)` that overwrites the
//! roll when its condition holds:
//!
//! ```text
//! loc_CFE6:
//!     jsr     (UpdateRNGSeed2).l
//!     andi.w  #7, d0
//!     cmp.w   ($FFFFEEA8).w, d0
//!     beq.s   loc_CFE6            ; REROLL if it matches the previous pick
//!     move.w  d0, ($FFFFEEA8).w
//!     movea.l stats_addr(a4), a3
//!     move.b  $58(a3,d0.w), ability+1(a4)   ; default = one of the 8 regular abilities
//!     moveq   #0, d0
//!     lea     $50(a3), a0
//!     moveq   #3, d7
//! loc_D00C:
//!     move.b  (a0)+, d0              ; condition id
//!     beq.s   loc_D024               ; 0 terminates
//!     add.b   d0, d0                 ; BYTE doubling -> table is 128 entries max
//!     lea     EnemyAIInstructionsOffs(pc), a1
//!     adda.w  (a1,d0.w), a1
//!     jsr     (a1)
//!     tst.b   d1
//!     bne.s   loc_D024               ; condition fired -> stop scanning
//!     dbf     d7, loc_D00C
//! ```
//!
//! (`ps4.asm:19146-19168`; the table is `EnemyAIInstructionsOffs`,
//! `ps4.asm:19364-19384`, twenty `dc.w` entries.) So the scan is **ordered**:
//! the first slot whose condition holds decides the ability, a `0` byte ends it
//! before the table is consulted, and each arm answers by writing `$3(a0)` —
//! the conditional byte of the slot it was called for, `a0` having been
//! post-incremented — into `ability+1(a4)` and returning a nonzero `d1`.
//!
//! Two things about the arms are easy to miss and both are modelled here:
//!
//! - **They read the actor's `reaction_flags`** (`$2A` of the fighter object):
//!   the AI's memory of what was done to it since its last action. Four arms
//!   clear that whole byte when they fire.
//! - **Their "occupied" test is the object's own word 0** (`_tst.w 0(a2)`) —
//!   the presence/kind word every object in this engine carries, not
//!   `fighter_id` (`$12`). The cartridge zeroes it both when a fighter dies
//!   (`loc_2D960`, `ps4.asm:59630`) and when `EnemyInit_Igglanova` empties a
//!   formation neighbour (`loc_BFE8`, `ps4.asm:18290`), so the port's
//!   [`Fighter::is_alive`] is that word being nonzero.
//!
//! Two arms do not write `d1` at all — `EnemyAI_Nothing`
//! (`ps4.asm:19389`) and `EnemyAI_CRayTubeNearSatMinion`, which writes `d0` on
//! both paths (`ps4.asm:22921` and `22925`) — so whether *their* scan stops depends on
//! the register's residue from unrelated frame code rather than on battle state;
//! see [`EnemyAiCondition::stops_the_scan`].

use super::fighters::{Fighter, FighterId, Roster, Side, reaction};
use super::records::{AI_CONDITIONS, BattleDataError, EnemyRecord};
use super::rng::Rolls;

#[cfg(test)]
#[path = "enemy_ai_tests.rs"]
mod tests;

/// The `EnemyID_*` constants the arms compare against (`ps4.constants.asm`).
pub mod enemy_id {
    /// `EnemyID_ArthroPod` — `$17`.
    pub const ARTHRO_POD: u16 = 23;
    /// `EnemyID_Wiredine` — `$19`.
    pub const WIREDINE: u16 = 25;
    /// `EnemyID_ZolSlug` — `$22`.
    pub const ZOL_SLUG: u16 = 34;
    /// `EnemyID_CRayTube` — `$28`.
    pub const CRAY_TUBE: u16 = 40;
    /// `EnemyID_SatMinion` — `$2A`.
    pub const SAT_MINION: u16 = 42;
    /// `EnemyID_BladeRight` — `$54`.
    pub const BLADE_RIGHT: u16 = 84;
    /// `EnemyID_HakenLeft` — `$56`.
    pub const HAKEN_LEFT: u16 = 86;
    /// `EnemyID_XeAThoul` — `$7B`.
    pub const XE_A_THOUL: u16 = 123;
}

/// One dispatchable arm of `EnemyAIInstructionsOffs` (`ps4.asm:19364`), named
/// after the label the disassembly gives the routine behind it.
///
/// Nineteen variants for twenty table entries: `$00` and `$13` both point at
/// `EnemyAI_Nothing` (`ps4.asm:19389`), a bare `rts`. `$00` can never be
/// dispatched — a zero condition byte ends the scan at `ps4.asm:19159` — so
/// only `$13` reaches that routine through [`CONDITIONS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyAiCondition {
    /// Entries `$00` and `$13` — `EnemyAI_Nothing`, `ps4.asm:19389`.
    Nothing,
    /// Entry `$01` — `EnemyAI_EmptySpace`, `ps4.asm:23524`.
    EmptySpace,
    /// Entry `$02` — `EnemyAI_HalfHPOrLower`, `ps4.asm:23436`.
    HalfHpOrLower,
    /// Entry `$03` — `EnemyAI_WiredineExists`, `ps4.asm:23259`.
    WiredineExists,
    /// Entry `$04` — `EnemyAI_ArthroPodExists`, `ps4.asm:23288`.
    ArthroPodExists,
    /// Entry `$05` — `EnemyAI_CRayTubeNearSatMinion`, `ps4.asm:22905`.
    CRayTubeNearSatMinion,
    /// Entry `$06` — `EnemyAI_ZolSlugs`, `ps4.asm:23017`.
    ZolSlugs,
    /// Entry `$07` — `EnemyAI_PhysicalAtkReceived`, `ps4.asm:22816`.
    PhysicalAtkReceived,
    /// Entry `$08` — `EnemyAI_MagicDamageReceived`, `ps4.asm:22500`.
    MagicDamageReceived,
    /// Entry `$09` — `EnemyAI_Alone`, `ps4.asm:22328`.
    Alone,
    /// Entry `$0A` — `EnemyAI_TechDamageReceived`, `ps4.asm:22232`.
    TechDamageReceived,
    /// Entry `$0B` — `EnemyAI_Ambush`, `ps4.asm:22217`.
    Ambush,
    /// Entry `$0C` — `EnemyAI_TechSealed`, `ps4.asm:22203`.
    TechSealed,
    /// Entry `$0D` — `EnemyAI_HakenLeftExists`, `ps4.asm:21628`.
    HakenLeftExists,
    /// Entry `$0E` — `EnemyAI_BladeRightExists`, `ps4.asm:21557`.
    BladeRightExists,
    /// Entry `$0F` — `EnemyAI_HalfHPOrLower_AllEnemies`, `ps4.asm:21320`.
    HalfHpOrLowerAllEnemies,
    /// Entry `$10` — `EnemyAI_Unknown`, `ps4.asm:20803`.
    Unknown,
    /// Entry `$11` — `EnemyAI_HP25PercentOrLower`, `ps4.asm:20491`.
    Hp25PercentOrLower,
    /// Entry `$12` — `EnemyAI_ThreeXeAThouls`, `ps4.asm:20365`.
    ThreeXeAThouls,
}

/// `EnemyAIInstructionsOffs` — `ps4.asm:19364-19384`, in table order.
///
/// The index into this array is the condition id a record's byte holds, which
/// is why it has twenty entries for nineteen routines.
pub const CONDITIONS: [EnemyAiCondition; 20] = [
    EnemyAiCondition::Nothing,
    EnemyAiCondition::EmptySpace,
    EnemyAiCondition::HalfHpOrLower,
    EnemyAiCondition::WiredineExists,
    EnemyAiCondition::ArthroPodExists,
    EnemyAiCondition::CRayTubeNearSatMinion,
    EnemyAiCondition::ZolSlugs,
    EnemyAiCondition::PhysicalAtkReceived,
    EnemyAiCondition::MagicDamageReceived,
    EnemyAiCondition::Alone,
    EnemyAiCondition::TechDamageReceived,
    EnemyAiCondition::Ambush,
    EnemyAiCondition::TechSealed,
    EnemyAiCondition::HakenLeftExists,
    EnemyAiCondition::BladeRightExists,
    EnemyAiCondition::HalfHpOrLowerAllEnemies,
    EnemyAiCondition::Unknown,
    EnemyAiCondition::Hp25PercentOrLower,
    EnemyAiCondition::ThreeXeAThouls,
    EnemyAiCondition::Nothing,
];

/// How many condition bytes a record carries (`$50`..`$53`).
pub const CONDITION_SLOTS: usize = AI_CONDITIONS;

impl EnemyAiCondition {
    /// The arm a condition id selects, or `None` for an id whose table read
    /// runs off the end.
    ///
    /// The id is doubled **as a byte** (`add.b d0, d0`, `ps4.asm:19160`), so the
    /// top bit is lost before it indexes the table: `$80` reads entry `$00`, and
    /// everything from `$14` to `$7F` reads past the twenty entries into
    /// whatever follows them. Retail has no bound check; this does, and the
    /// census test is what keeps shipped ids inside the range.
    #[must_use]
    pub const fn from_id(id: u8) -> Option<EnemyAiCondition> {
        let index = (id & 0x7F) as usize;
        if index < CONDITIONS.len() {
            Some(CONDITIONS[index])
        } else {
            None
        }
    }

    /// The table entry an arm sits at, and `$13` for the `Nothing` the last
    /// entry holds: a round trip through [`from_id`](Self::from_id), used by the
    /// census test.
    pub fn ids(&self) -> impl Iterator<Item = u8> + '_ {
        (0..=0x13u8).filter(move |id| EnemyAiCondition::from_id(*id) == Some(*self))
    }

    /// Whether the arm writes `d1`, which is what makes the scan stop.
    ///
    /// All but two do. `EnemyAI_Nothing` (`ps4.asm:19389`) is a bare `rts`, and
    /// `EnemyAI_CRayTubeNearSatMinion` (`ps4.asm:22921` and `22925`) writes `d0`
    /// on both of its paths; neither touches `d1`, so `tst.b d1` at
    /// `ps4.asm:19164`
    /// reads whatever the frame's other code left there. [`instruction_block`]
    /// treats that residue as zero and keeps scanning, and the census test
    /// proves no shipped record can tell the difference.
    #[must_use]
    pub const fn stops_the_scan(&self) -> bool {
        !matches!(
            self,
            EnemyAiCondition::Nothing | EnemyAiCondition::CRayTubeNearSatMinion
        )
    }

    /// The ability this arm writes when it fires: `$54 + k` for the slot `k`
    /// the scan stopped at (`ps4.asm:19158-19160`).
    ///
    /// Only used to name the ability a turn could not resolve; a record that
    /// does not list this arm at all yields the zero every byte array starts
    /// with.
    #[must_use]
    pub fn written_ability(&self, record: &EnemyRecord) -> u8 {
        record
            .condition_ids
            .iter()
            .position(|id| EnemyAiCondition::from_id(*id) == Some(*self))
            .and_then(|slot| record.conditional_abilities.get(slot).copied())
            .unwrap_or(0)
    }
}

/// What the instruction block decided, for the caller that owns the roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AiOutcome {
    /// No arm fired: the rolled regular ability stands.
    Rolled,
    /// Slot `slot`'s condition held, so the ability is that slot's conditional
    /// one. `neighbour` is the empty enemy slot arm `$01` named, which the
    /// fission object refills (`loc_14CBE`, `ps4.asm:29699`); `None` when that
    /// arm did not fire or named a side with no enemy slot behind it.
    Replaced {
        /// Which of the four condition slots fired, `0..=3`.
        slot: usize,
        /// The neighbouring slot the fission object would refill.
        neighbour: Option<FighterId>,
    },
    /// The scan stopped on an arm this port cannot evaluate, so the ability the
    /// turn really ran is unknown.
    Unsupported {
        /// The arm that stopped the scan.
        condition: EnemyAiCondition,
    },
}

impl AiOutcome {
    /// The ability the timeline reports for an outcome that stopped the scan on
    /// an arm the port cannot evaluate.
    ///
    /// A `Rolled` outcome reports nothing: the roll already names its ability.
    #[must_use]
    pub(crate) fn unreported_ability(&self, record: &EnemyRecord) -> Option<u8> {
        match self {
            AiOutcome::Unsupported { condition } => Some(condition.written_ability(record)),
            AiOutcome::Rolled | AiOutcome::Replaced { .. } => None,
        }
    }
}

/// `Enemy_Attack`'s instruction block: the scan over `$50(a3)`..`$53(a3)`.
///
/// The ability the roll wrote is left alone when nothing fires, which is why
/// this returns an outcome rather than an ability: the caller keeps the roll
/// when the answer is [`AiOutcome::Rolled`].
///
/// # Draws
///
/// Only arm `$01` draws, and only when **both** neighbouring slots are empty:
/// the routine's own `jsr (UpdateRNGSeed2).l` (`ps4.asm:23541`) sits inside the
/// `cmpi.b #3` branch. Every other arm is a pure read of battle state.
///
/// # Errors
/// [`BattleDataError::UnknownAiCondition`] for a condition byte past the
/// table's twenty entries, which retail would dispatch into whatever word
/// follows `EnemyAIInstructionsOffs`.
pub(crate) fn instruction_block(
    roster: &mut Roster,
    actor: FighterId,
    record: &EnemyRecord,
    rolls: &mut impl Rolls,
) -> Result<AiOutcome, BattleDataError> {
    // The last arm that wrote the ability slot. An arm that does not stop the
    // scan leaves its write in place for the next one to overwrite.
    let mut chosen: Option<(usize, Option<FighterId>)> = None;
    for slot in 0..CONDITION_SLOTS {
        let id = record.condition_ids.get(slot).copied().unwrap_or(0);
        if id == 0 {
            // `beq.s loc_D024` (`ps4.asm:19159`): a zero byte ends the scan.
            break;
        }
        let Some(condition) = EnemyAiCondition::from_id(id) else {
            return Err(BattleDataError::UnknownAiCondition(id));
        };
        match arm(roster, actor, condition, rolls) {
            Arm::NoFire => {}
            Arm::Fire => {
                chosen = Some((slot, None));
                if condition.stops_the_scan() {
                    break;
                }
            }
            Arm::FireEmptySpace(neighbour) => {
                chosen = Some((slot, Some(neighbour)));
                if condition.stops_the_scan() {
                    break;
                }
            }
            Arm::Clears => {
                // `clr.b reaction_flags(a4)`: the four "what was done to me"
                // arms wipe the whole byte, not just the bit they tested.
                if let Some(fighter) = roster.get_mut(actor) {
                    fighter.reaction_flags = 0;
                }
                chosen = Some((slot, None));
                if condition.stops_the_scan() {
                    break;
                }
            }
            Arm::ClearsXeAThoulFlags => {
                // Three `bclr #4, reaction_flags` on Fighter_Enemy_1..3
                // (`ps4.asm:20369-20371`): slots 6, 7 and 8.
                for id in 6..=8u8 {
                    if let Some(fighter) = FighterId::new(id).and_then(|id| roster.get_mut(id)) {
                        fighter.reaction_flags &= !super::fighters::reaction::MULTI_TARGET;
                    }
                }
                chosen = Some((slot, None));
                if condition.stops_the_scan() {
                    break;
                }
            }
            Arm::Unsupported => return Ok(AiOutcome::Unsupported { condition }),
        }
    }
    Ok(match chosen {
        Some((slot, neighbour)) => AiOutcome::Replaced { slot, neighbour },
        None => AiOutcome::Rolled,
    })
}

/// What one arm answered, in the terms the scan needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    /// The condition did not hold: `d1 = 0`, and the scan moves on.
    NoFire,
    /// It held: the caller writes the slot's conditional ability.
    Fire,
    /// Arm `$01` held, and the object refills this slot.
    FireEmptySpace(FighterId),
    /// It held and cleared the actor's whole `reaction_flags` byte.
    Clears,
    /// It held and cleared bit 4 on enemy slots 1..3.
    ClearsXeAThoulFlags,
    /// The condition cannot be evaluated from port state.
    Unsupported,
}

/// One arm's answer, reading exactly what its routine reads.
fn arm(
    roster: &Roster,
    actor: FighterId,
    condition: EnemyAiCondition,
    rolls: &mut impl Rolls,
) -> Arm {
    match condition {
        // `rts`: no ability, no `d1`, and unobservable either way — see
        // [`EnemyAiCondition::stops_the_scan`].
        EnemyAiCondition::Nothing => Arm::NoFire,

        // `EnemyAI_EmptySpace`, `ps4.asm:23524-23552`: `$FFFFEE81` bit 0 is the
        // word at `prev_obj` being zero, bit 1 the word at `next_obj`. Both
        // sides empty costs one draw — even keeps the left, odd the right — and
        // one side empty costs none, which is why this arm fires for either.
        EnemyAiCondition::EmptySpace => {
            let mask = empty_sides(roster, actor);
            let side = match (mask & 1 != 0, mask & 2 != 0) {
                (false, false) => return Arm::NoFire,
                (true, true) => {
                    if rolls.next_roll() & 1 == 0 {
                        Neighbour::Left
                    } else {
                        Neighbour::Right
                    }
                }
                (true, false) => Neighbour::Left,
                (false, true) => Neighbour::Right,
            };
            match neighbour(actor, side) {
                Some(slot) => Arm::FireEmptySpace(slot),
                // `$FFFFEE81` named a side with no enemy object behind it: the
                // first enemy slot's left neighbour is a party slot, and the
                // last enemy slot's right neighbour is past `Obj_Fighters`.
                // Retail writes the ability and the object looks for a slot
                // there; the port's refill declines (see `resolve_fission`).
                None => Arm::Fire,
            }
        }

        // `EnemyAI_HalfHPOrLower`, `ps4.asm:23436-23444`: `lsr.w #1` of the
        // actor's own maximum, then `cmp.w curr_hp, d1 / bcs`, so the condition
        // holds at exactly half.
        EnemyAiCondition::HalfHpOrLower => {
            if at_or_below_fraction(roster, actor, 1) {
                Arm::Fire
            } else {
                Arm::NoFire
            }
        }

        // `EnemyAI_WiredineExists`, `ps4.asm:23259-23273`: slot 2 holds
        // Wiredine, and the slot **two** before the actor — or, for
        // Fighter_Enemy_1, two after it (`$80(a4)`) — reads zero.
        EnemyAiCondition::WiredineExists => {
            if occupant(roster, 7).map(|f| f.stats.enemy_id) != Some(enemy_id::WIREDINE) {
                return Arm::NoFire;
            }
            let other = if actor.get() == 6 {
                slot_after(actor, 2)
            } else {
                slot_after(actor, -2)
            };
            if occupied(roster, other) {
                Arm::NoFire
            } else {
                Arm::Fire
            }
        }

        // `EnemyAI_ArthroPodExists`, `ps4.asm:23288-23308`: ArthroPod in slot 1
        // with slot 3 empty, or ArthroPod in slot 3 with slot 1 empty.
        EnemyAiCondition::ArthroPodExists => {
            let first = occupant(roster, 6).map(|f| f.stats.enemy_id);
            let third = occupant(roster, 8).map(|f| f.stats.enemy_id);
            let fires = match first {
                Some(id) => id == enemy_id::ARTHRO_POD && !occupied(roster, 8),
                None => third == Some(enemy_id::ARTHRO_POD),
            };
            if fires { Arm::Fire } else { Arm::NoFire }
        }

        // `EnemyAI_CRayTubeNearSatMinion`, `ps4.asm:22905-22953`: slots 1 and 3
        // hold SatMinion with CRayTube in slot 2.
        EnemyAiCondition::CRayTubeNearSatMinion => {
            let ids = [
                occupant(roster, 6).map(|f| f.stats.enemy_id),
                occupant(roster, 7).map(|f| f.stats.enemy_id),
                occupant(roster, 8).map(|f| f.stats.enemy_id),
            ];
            if ids
                == [
                    Some(enemy_id::SAT_MINION),
                    Some(enemy_id::CRAY_TUBE),
                    Some(enemy_id::SAT_MINION),
                ]
            {
                Arm::Fire
            } else {
                Arm::NoFire
            }
        }

        // `EnemyAI_ZolSlugs`, `ps4.asm:23017-23036`: every occupied enemy slot
        // holds a ZolSlug, and there are exactly two of them. A slot holding
        // anything else leaves the loop at once.
        EnemyAiCondition::ZolSlugs => {
            let mut slugs = 0u8;
            for id in 6..=9u8 {
                match occupant(roster, id).map(|f| f.stats.enemy_id) {
                    None => {}
                    Some(enemy) if enemy == enemy_id::ZOL_SLUG => slugs += 1,
                    Some(_) => return Arm::NoFire,
                }
            }
            if slugs == 2 { Arm::Fire } else { Arm::NoFire }
        }

        // `EnemyAI_PhysicalAtkReceived`, `ps4.asm:22816-22826`.
        EnemyAiCondition::PhysicalAtkReceived => received(roster, actor, reaction::PHYSICAL),

        // `EnemyAI_MagicDamageReceived`, `ps4.asm:22500-22510`.
        EnemyAiCondition::MagicDamageReceived => received(roster, actor, reaction::MAGIC),

        // `EnemyAI_Alone`, `ps4.asm:22328-22344`: exactly one occupied enemy
        // slot. The count *is* `d1`, so the answer is 1 and not merely "fired".
        EnemyAiCondition::Alone => {
            let occupied = (6..=9u8).filter(|id| occupied(roster, *id)).count();
            if occupied == 1 {
                Arm::Fire
            } else {
                Arm::NoFire
            }
        }

        // `EnemyAI_TechDamageReceived`, `ps4.asm:22232-22242`.
        EnemyAiCondition::TechDamageReceived => received(roster, actor, reaction::TECHNIQUE),

        // `EnemyAI_Ambush`, `ps4.asm:22217-22228`: bit 3, which the opening
        // priority roll sets on every enemy slot (`loc_B62A`'s tail,
        // `ps4.asm:17456-17463`).
        EnemyAiCondition::Ambush => received(roster, actor, reaction::AMBUSH),

        // `EnemyAI_TechSealed`, `ps4.asm:22203-22210`: the actor's own status
        // byte, bit 4 (`StatusTechSealed`).
        EnemyAiCondition::TechSealed => {
            let sealed = roster.get(actor).is_some_and(|f| {
                f.is_alive() && f.stats.status & super::stats::status::TECH_SEALED != 0
            });
            if sealed { Arm::Fire } else { Arm::NoFire }
        }

        // `EnemyAI_HakenLeftExists` (`ps4.asm:21628`) is named by 84
        // BladeRight and `EnemyAI_BladeRightExists` (`ps4.asm:21557`) by 86
        // HakenLeft, despite the labels reading the other way round: each fires
        // for the **last** of its pair, once no other enemy object holds either
        // id.
        EnemyAiCondition::HakenLeftExists => {
            partner_gone(roster, actor, enemy_id::BLADE_RIGHT, enemy_id::HAKEN_LEFT)
        }
        EnemyAiCondition::BladeRightExists => {
            partner_gone(roster, actor, enemy_id::HAKEN_LEFT, enemy_id::BLADE_RIGHT)
        }

        // `EnemyAI_HalfHPOrLower_AllEnemies`, `ps4.asm:21320-21338`: any
        // occupied enemy slot at or below half of **its own** maximum, the
        // actor included, tested slot 1 upwards.
        EnemyAiCondition::HalfHpOrLowerAllEnemies => {
            let any = (6..=9u8)
                .any(|id| FighterId::new(id).is_some_and(|id| at_or_below_fraction(roster, id, 1)));
            if any { Arm::Fire } else { Arm::NoFire }
        }

        // `EnemyAI_Unknown`, `ps4.asm:20803-20813`: bit 0 of `$FFFFEEA4`. No
        // routine in the disassembly ever sets that word — it is tested and
        // cleared here and written nowhere — so the port has no writer to read
        // the bit from.
        EnemyAiCondition::Unknown => Arm::Unsupported,

        // `EnemyAI_HP25PercentOrLower`, `ps4.asm:20491-20502`: the actor at or
        // below a quarter of its maximum, **and** `$FFFFEE86` clear. That byte
        // is Lashiec's own battle-object flag: `EnemyInit_Lashiec` clears it
        // (`ps4.asm:18114`), and object `$7EC` (`loc_27ED0`, `ps4.asm:53099`)
        // sets it — the object Lashiec's own `$62` arm loads (`ps4.asm:20172`).
        // The port models no battle objects, so it cannot say whether that flag
        // is set; 128 Lashiec is the only record naming this entry.
        EnemyAiCondition::Hp25PercentOrLower => Arm::Unsupported,

        // `EnemyAI_ThreeXeAThouls`, `ps4.asm:20365-20397`: bit 4 on the actor,
        // then enemy slots 1, 2 and 3 all holding XeAThoul.
        EnemyAiCondition::ThreeXeAThouls => {
            let multi = roster
                .get(actor)
                .is_some_and(|f| f.reaction_flags & reaction::MULTI_TARGET != 0);
            let all = (6..=8u8).all(|id| {
                occupant(roster, id).map(|f| f.stats.enemy_id) == Some(enemy_id::XE_A_THOUL)
            });
            if multi && all {
                Arm::ClearsXeAThoulFlags
            } else {
                Arm::NoFire
            }
        }
    }
}

/// The "what was done to me" arms: bit set, fire — and wipe the whole byte.
fn received(roster: &Roster, actor: FighterId, bit: u8) -> Arm {
    let set = roster
        .get(actor)
        .is_some_and(|f| f.reaction_flags & bit != 0);
    if set { Arm::Clears } else { Arm::NoFire }
}

/// The partner-existence pair: fire when no other enemy object holds `partner`
/// **and** none holds `other`.
///
/// Both loops read `fighter_id(a2)` of every occupied slot, skip `a4` itself
/// (`cmpa.l a4, a2`) and count the other id in `d5`, which starts at `-1` so a
/// count of zero fires.
fn partner_gone(roster: &Roster, actor: FighterId, partner: u16, other: u16) -> Arm {
    let mut others = 0u8;
    for id in 6..=9u8 {
        if id == actor.get() {
            continue;
        }
        let Some(fighter) = occupant(roster, id) else {
            continue;
        };
        if fighter.stats.enemy_id == partner {
            return Arm::NoFire;
        }
        if fighter.stats.enemy_id == other {
            others += 1;
        }
    }
    if others == 0 { Arm::Fire } else { Arm::NoFire }
}

/// Whether the fighter in `id` is at or below `max_hp >> shift`.
///
/// The routines halve (or quarter) `max_hp` into `d1` and then
/// `cmp.w curr_hp(a3), d1 / bcs`: an unsigned borrow, so the branch is taken
/// when the reduced maximum is **below** the current HP. The condition
/// therefore holds when `curr_hp <= max_hp >> shift` — at exactly half, too.
fn at_or_below_fraction(roster: &Roster, id: FighterId, shift: u8) -> bool {
    roster
        .get(id)
        .is_some_and(|f| f.stats.curr_hp <= f.stats.max_hp >> shift)
}

/// The cartridge's `_tst.w 0(a2)`: the fighter object occupying the slot `id`
/// names, if any.
///
/// An id outside 1..=9 is not a fighter slot at all — see [`neighbour`] — and
/// yields `None`, the way the wiped, never-written word there reads.
fn occupant(roster: &Roster, id: u8) -> Option<&Fighter> {
    FighterId::new(id)
        .and_then(|id| roster.get(id))
        .filter(|f| f.is_alive())
}

/// Whether an object occupies the fighter slot `id` names.
fn occupied(roster: &Roster, id: u8) -> bool {
    occupant(roster, id).is_some()
}

/// The slot one `$40`-byte object is away from the actor, as a raw slot id.
///
/// `prev_obj`/`next_obj` are plain address arithmetic, not formation lookups:
/// the first enemy slot's left neighbour is the **fifth party slot**
/// (`Fighter_Enemy_1` is `$FFFF4540`), and the fourth enemy slot's right
/// neighbour is `$FFFF4640`, one slot past the nine `Obj_Fighters` holds. The
/// battle start wipes `$FFFF4000`-`$FFFF47FF` (`ps4.asm:9989-9993`) and no
/// routine writes that word, so it always reads zero.
fn slot_after(actor: FighterId, delta: i8) -> u8 {
    actor.get().wrapping_add_signed(delta)
}

/// `$FFFFEE81`: bit 0 for an empty left neighbour, bit 1 for an empty right.
fn empty_sides(roster: &Roster, actor: FighterId) -> u8 {
    let mut mask = 0u8;
    if !occupied(roster, slot_after(actor, -1)) {
        mask |= 1;
    }
    if !occupied(roster, slot_after(actor, 1)) {
        mask |= 2;
    }
    mask
}

/// Which of the two neighbouring objects the empty-space mask names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Neighbour {
    /// `$FFFFEE81` bit 0.
    Left,
    /// `$FFFFEE81` bit 1.
    Right,
}

/// The enemy slot a side names, or `None` when no slot there can hold an enemy.
///
/// Only word 0 decides whether the arm fires; this decides whether the
/// object's refill has an enemy slot to put a fighter back into.
fn neighbour(actor: FighterId, side: Neighbour) -> Option<FighterId> {
    let delta = match side {
        Neighbour::Left => -1,
        Neighbour::Right => 1,
    };
    FighterId::new(slot_after(actor, delta)).filter(|id| id.side() == Side::Enemy)
}
