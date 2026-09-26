//! Which fighter a single-target command actually resolves against.
//!
//! A command's target is chosen at command time and copied into
//! `Current_Target_Index` by the round's turn pass
//! (`ps4.asm:8050-8053`); between that copy and the action, the fight moves.
//! `loc_5A98` (`ps4.asm:8323-8344`) is the one routine that decides what the
//! action then lands on, and every single-target command goes through it: a
//! plain swing, a technique, a skill, an item and the vehicle's own attack.
//!
//! ```text
//! loc_5A98:
//!     move.w  d0, d1                  ; d0 -> d1: the aimed slot
//!     bmi.w   loc_5B8E                ; 8325: a negative aim is the whole side
//!     tst.w   (Current_Target_Index).l
//!     bmi.w   loc_5B8E                ; 8327:   and is left alone
//!     ...  status & $C4 clear -> loc_5B8E      ; 8337: the aim is kept, no roll
//! loc_5ACE:
//!     cmpi.w  #5, d1
//!     bgt.s   loc_5AE6                ; 8340: an enemy slot -> the deficit scan
//!     ...  loc_56F0 / Enemy_TargetCharacter    ; 8341-8343: a party slot
//! loc_5AE6:
//!     moveq   #-1, d4                 ; 8346
//!     moveq   #6, d5 / moveq #6, d3   ; 8347-8348
//!     jsr     loc_5C8A(pc)            ; 8349: which loop this command takes
//!     cmpi.w  #1, d0 / beq.s loc_5B42 ; 8350-8351
//!     move.w  #$7000, d4              ; 8352
//! loc_5AFA:   ...                     ; 8353-8380: the **smallest** deficit
//! loc_5B42:   ...                     ; 8381-8407: the **largest** deficit
//! loc_5B88:
//!     move.w  d3, (Current_Target_Index).l     ; 8409
//! ```
//!
//! # The two loops are mirrors
//!
//! Both walk the four enemy slots 6..9 in order (`addq.w #1, d5` /
//! `cmpi.w #9, d5` / `ble.s`, `ps4.asm:8404-8407` and `8377-8379`), skip a slot
//! that holds no fighter (`tst.w (a0)`, `ps4.asm:8356-8357`, `8384-8385`) or one
//! whose `status & $44` is set (`andi.b #$44`, `ps4.asm:8359-8361`,
//! `8387-8389`, i.e. dead or android-dead), and compute each survivor's deficit
//! as `max_hp - curr_hp` (`move.w $10(a0), d0 / sub.w $E(a0), d0`,
//! `ps4.asm:8362-8363`, `8390-8391`). They differ in the comparison and in the
//! sentinel the running best starts at:
//!
//! * `loc_5B42` starts `d4` at `-1` (`ps4.asm:8346`) and takes a **strictly
//!   larger** deficit (`blt.s loc_5B7C`, `ps4.asm:8394`), leaving a smaller one
//!   (`bgt.s loc_5B80`, `ps4.asm:8393`);
//! * `loc_5AFA` starts `d4` at `$7000` (`ps4.asm:8352`) and takes a **strictly
//!   smaller** deficit (`bgt.s loc_5B34`, `ps4.asm:8366`), leaving a larger one
//!   (`blt.s loc_5B38`, `ps4.asm:8365`).
//!
//! An **equal** deficit draws `UpdateRNGSeed2` once in both loops
//! (`ps4.asm:8395-8400` and `8367-8372`): the later slot wins when the draw's
//! low bit is set (`btst #0, d1`), the earlier one when it is clear. No other
//! case draws. `loc_5B88` writes the winner into `Current_Target_Index`
//! (`ps4.asm:8409`); `d3` starts at slot 6 (`ps4.asm:8348`), so a scan that
//! takes nothing aims at the first enemy slot.
//!
//! # Which loop a command takes
//!
//! `loc_5C8A` (`ps4.asm:8504-8512`) reads `Current_Command`, takes its low byte
//! as the ability id, and dispatches on the command's own kind over `loc_5CA2`
//! (`ps4.asm:8514-8521`), returning 1 for "largest" and anything else for
//! "smallest" (`cmpi.w #1, d0 / beq.s loc_5B42`, `ps4.asm:8350-8351`):
//!
//! | kind (`constants:2005-2008`) | arm | loop |
//! |---|---|---|
//! | 1, attack | `loc_5CB0`'s `moveq #1, d0` (`ps4.asm:8523-8524`) | largest |
//! | 2, technique | `TechniqueData` byte 0's low nibble (`ps4.asm:8529-8537`) | largest iff nibble `1` |
//! | 3, skill | `SkillData` byte 0's low nibble (`ps4.asm:8533-8537`) | largest iff nibble `1` |
//! | 4, item | the item record's byte 0 low nibble (`ps4.asm:8540-8544`) | largest iff nibble `1` |
//! | 5, defense | `loc_5CB2`'s bare `rts` (`ps4.asm:8525-8526`) | smallest |
//! | 6, the vehicle's own attack | `loc_5CB0` (`ps4.asm:8523-8524`) | largest |
//! | 7, vehicle skill | `VehicleSkillData` byte 0's low nibble (`ps4.asm:8547-8548`) | largest iff nibble `1` |
//!
//! Byte 0 of an ability record is its effect id — [`Technique::effect`],
//! [`Skill::effect`] and [`BattleItem::effect`] all carry it — so
//! [`deficit_for_effect`] is that column: an **effect** whose low nibble is 1
//! (the damaging `AbilityEffectsOffs` entry, `$01` — and `$11`, which shares the
//! nibble) is re-aimed like a swing, and every other effect takes the mirrored
//! loop.
//!
//! [`Technique::effect`]: super::technique::Technique::effect
//! [`Skill::effect`]: super::skill::Skill::effect
//! [`BattleItem::effect`]: super::item::BattleItem::effect
//!
//! # Which commands reach the scan at all
//!
//! Entry is decided before the dispatch above, by the *target* the command's own
//! record declares: `loc_5BDC` (`ps4.asm:8434-8443`) returns `1` for a swing and
//! for the vehicle's attack (`ps4.asm:8466`), the record's target nibble for a
//! technique, skill or item (`ps4.asm:8479-8480`, `8492-8493`), and `2` for a
//! vehicle skill (`ps4.asm:8502`); the caller enters `loc_5A98` only when that
//! value is 1, 2 or 3 (`subq.w #1, d1 / cmpi.w #2, d1 / bls.w loc_5A98`,
//! `ps4.asm:8055-8057`). So:
//!
//! * a swing (kind 1) and the vehicle's attack (kind 6) always enter;
//! * a technique, skill, item or vehicle skill enters when its target nibble is
//!   1 (single enemy), 2 (all enemies) or 3 (`Battle_ComdTechTarget`'s group
//!   arms, `ps4.asm:2646-2654`);
//! * an ability aimed at the **party** — a heal or a revival, target nibble 4,
//!   6 or 8 — never enters at all, so a fallen ally stays the recipient and the
//!   effect's own eligibility decides what happens.
//!
//! The `d1 <= 5` arm at `loc_5ACE` is therefore the arm an **enemy** attacker's
//! command reaches: its cell is kind 1 (`move.w #$100, (a2)+`, `ps4.asm:7932`)
//! with a character's slot as the aim (`move.b (a0)+, (a2)+`, `ps4.asm:7935`),
//! and the arm makes the weighted draw (`loc_56F0` / `Enemy_TargetCharacter`)
//! the port's [`take_turn`](super::engine::Battle::take_turn) makes for one. A
//! command in an AUTO battle skips `loc_5A98` altogether and re-aims in the
//! macro table (`MacroSpecialActionTable` / `MacroSpecialActionOffs`,
//! `ps4.asm:8078-8104`), which is a rule of its own.

use super::fighters::{Fighter, FighterId, Roster, Side};
use super::rng::Rolls;

#[cfg(test)]
#[path = "retarget_tests.rs"]
mod tests;

/// The running best `loc_5AFA` starts at: `move.w #$7000, d4`
/// (`ps4.asm:8352`), so the smallest-deficit loop rejects a candidate only at
/// `$7000` or above. No fighter in the pack can present `max_hp - curr_hp` that
/// large, so in this port the sentinel never rejects a living slot — the
/// comparison is still written the way the loop writes it.
const SMALLEST_START: i32 = 0x7000;

/// Which of retail's two mirrored loops re-aims a command whose aim is down.
///
/// The command's own kind and record pick it ([`deficit_for_effect`]), and the
/// loop is the only thing that differs between them: same slots, same skip
/// mask, same one-draw tiebreak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deficit {
    /// `loc_5B42` (`ps4.asm:8381-8407`): the largest `max_hp - curr_hp`, from
    /// `d4 = -1` (`ps4.asm:8346`). A swing and the vehicle's own attack take
    /// it, and so does an ability whose effect id is `$01`.
    Largest,
    /// `loc_5AFA` (`ps4.asm:8353-8380`): the smallest `max_hp - curr_hp`, from
    /// `d4 = $7000` (`ps4.asm:8352`). Every other ability takes it.
    Smallest,
}

/// `loc_5C8A` -> `loc_5CA2`'s ability arms: the loop a record's own effect id
/// selects.
///
/// The dispatch reads byte 0 of the record — the effect id — and takes
/// `andi.w #$F` of it before the `cmpi.w #1, d0` (`ps4.asm:8529-8536`), so the
/// test is on the **low nibble**: `$01` and `$11` both ask for the
/// largest-deficit loop, everything else for the mirrored one.
#[must_use]
pub const fn deficit_for_effect(effect: u8) -> Deficit {
    if effect & 0x0F == 1 {
        Deficit::Largest
    } else {
        Deficit::Smallest
    }
}

/// `loc_5AE6`-`loc_5B88`: which enemy slot a re-aimed command lands on.
///
/// The four enemy slots are walked in order, only living fighters take part,
/// and the deficit the winner holds is the running best ([`Deficit`]). An
/// **equal** deficit costs one `UpdateRNGSeed2` draw and the later slot wins
/// exactly when that draw's low bit is set; a strictly better one costs
/// nothing.
///
/// `None` is a scan that took no slot: retail then aims at `d3`'s initial slot
/// 6 (`ps4.asm:8348`, `8409`), which the walk has just shown to hold no fighter
/// or an out one — this port reports "nothing to re-aim at" and lets the
/// caller's own turn decide, which is the same battle outcome without an aim at
/// an empty slot.
fn scan(roster: &Roster, deficit: Deficit, rolls: &mut impl Rolls) -> Option<FighterId> {
    let mut best = match deficit {
        // `moveq #-1, d4` (`ps4.asm:8346`). HP is floored at zero and every
        // heal is capped at the maximum, so no live slot presents a negative
        // deficit and the first candidate always takes.
        Deficit::Largest => -1,
        Deficit::Smallest => SMALLEST_START,
    };
    let mut winner = None;
    for fighter in roster.side(Side::Enemy) {
        // `tst.w (a0)` (`ps4.asm:8384-8385`) and `status & $44`
        // (`ps4.asm:8387-8389`): an empty slot and an out fighter are skipped.
        if !fighter.is_alive() {
            continue;
        }
        let candidate = i32::from(fighter.stats.max_hp) - i32::from(fighter.stats.curr_hp);
        let take = match deficit {
            // `cmp.w d0, d4 / bgt.s loc_5B80 / blt.s loc_5B7C`
            // (`ps4.asm:8392-8394`): strictly larger takes the slot.
            Deficit::Largest => best < candidate,
            // `cmp.w d0, d4 / blt.s loc_5B38 / bgt.s loc_5B34`
            // (`ps4.asm:8364-8366`): strictly smaller takes it.
            Deficit::Smallest => best > candidate,
        };
        // A tie draws once in both loops (`ps4.asm:8395-8400`, `8367-8372`),
        // and the later slot wins on an odd draw.
        let take = if best == candidate {
            rolls.next_roll() & 1 == 1
        } else {
            take
        };
        if take {
            best = candidate;
            winner = Some(fighter.id);
        }
    }
    winner
}

/// `loc_5A98`-`loc_5B8A`: the fighter a single-target command resolves against.
///
/// `commanded` is the fighter the command named — the cell the round's turn
/// pass copied into `Current_Target_Index`, which a capture's `bcmd` group
/// holds. `deficit` is the loop this command's kind and record select
/// ([`deficit_for_effect`] for an ability, [`Deficit::Largest`] for a swing and
/// the vehicle's own attack, `loc_5CB0`/`ps4.asm:8523-8524`).
///
/// An aim whose slot holds a fighter that is not out keeps the aim and draws
/// **nothing** (`tst.w (a0)` / `status & $C4` clear, `ps4.asm:8330-8337`).
/// Anything else — the slot empty, or its fighter dead or android-dead —
/// dispatches on the aim's own side, the cartridge's `cmpi.w #5, d1`
/// (`ps4.asm:8339-8340`):
///
/// * an **enemy** slot (id above 5) is re-aimed by [`scan`], so the answer is a
///   living enemy or `None` when there is none;
/// * a **party** slot keeps the commanded fighter. Retail's arm there is the
///   weighted character draw, which only an enemy attacker's command reaches
///   (see the module note: a party member's heal never enters `loc_5A98` at
///   all), and that draw lives in
///   [`take_turn`](super::engine::Battle::take_turn).
///
/// A whole-side command never comes here: its aim is `$FFFF` and the routine
/// returns early (`ps4.asm:8325-8327`). This port's own default cursor — a
/// command with no target cell, which is [`crate::battle::Command::Attack`] —
/// names the first living enemy, which is alive by construction and therefore
/// kept exactly as a commanded one would be.
#[must_use]
pub fn single_target(
    roster: &Roster,
    commanded: FighterId,
    deficit: Deficit,
    rolls: &mut impl Rolls,
) -> Option<FighterId> {
    if roster.get(commanded).is_some_and(Fighter::is_alive) {
        return Some(commanded);
    }
    match commanded.side() {
        Side::Enemy => scan(roster, deficit, rolls),
        Side::Party => Some(commanded),
    }
}
