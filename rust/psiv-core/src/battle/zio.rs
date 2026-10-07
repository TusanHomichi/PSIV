//! The Zio family's phase counter, `$FFFFEE98`.
//!
//! Four enemy routines read the byte and write it back: `EnemyAttack_Zio3`
//! (`ps4.asm:19410`, enemy 152), `EnemyAttack_Zio` (`ps4.asm:19483`, enemy 139)
//! and `EnemyAttack_Zio2` (`ps4.asm:19519`, enemy 140). Nothing else in the
//! image touches it:
//!
//! | access | where | what |
//! |---|---|---|
//! | clear | `EnemyInit_Zio` (`ps4.asm:17895-17896`), the init routine of `$8B`, `$8C` and `$98` | `clr.b ($FFFFEE98).w` |
//! | read, write | `EnemyAttack_Zio3` (`ps4.asm:19411-19461`) | tests 0 to 4 in turn, `addq.b #1` on each arm that runs |
//! | read, write | `EnemyAttack_Zio` (`ps4.asm:19484-19499`) | tests 0 and 1, `addq.b #1` on both |
//! | read, write | `EnemyAttack_Zio2` (`ps4.asm:19520-19523`) | tests 0, `addq.b #1` on that arm only |
//!
//! (`grep -n EE98 ps4.asm` finds those seventeen code lines; its other hits are
//! data bytes and the unrelated label `loc_27EE98`.) The counter is a
//! single byte shared by whichever Zio routine runs, so a form change does not
//! reset it: the Psycho Wand's `loc_3CF60` (`ps4.asm:79203-79215`) swaps enemy 139
//! for 140 in the formation and calls `loc_7F22`, which refills the stats and
//! names and runs no init routine (`ps4.asm:11908-11934`, `11939`).
//!
//! The routines ignore the ability the roll wrote (`$24(a4)`) on every arm they
//! script: the roll, the instruction block and their draws all happen first
//! (`Enemy_Attack`, `ps4.asm:19138-19173`), then the arm overwrites the
//! ability. [`step`] is the table of those arms.

/// `EnemyID_Zio`, the invulnerable first form (`ps4.asm:79209-79213`).
pub(super) const ZIO: u16 = 139;
/// `EnemyID_Zio2`, the form the Psycho Wand reveals.
pub(super) const ZIO2: u16 = 140;
/// `Zio3`, the Demi-rescue encounter that returns to the field.
pub(super) const ZIO3: u16 = 152;

/// What a scripted Zio turn does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ZioTurn {
    /// Ability `$6B` and `BattleObj_MagBarrir` (`ps4.asm:67319`): an animation
    /// that never writes to a party fighter.
    Barrier,
    /// Zio3 phase 1: ability cleared, `BattleObj_NightmarePart1`
    /// (`ps4.asm:67253`); no damage and no effect.
    Invocation,
    /// Zio3 phases 2 and 5 onward: the arm ends the turn with no object
    /// (`loc_D264`, `loc_D31C`).
    Pause,
    /// Ability `$53`: `BattleObj_NightmarePart2` (`ps4.asm:67107`) for Zio3,
    /// `BattleObj_NightmareFull` (`ps4.asm:66960`) for Zio. Neither writes to a
    /// party fighter.
    Nightmare,
    /// Zio3 phase 4: ability `$70`, `BattleObj_BlackWave1` (`ps4.asm:67019`),
    /// which returns to the field without a defeat epilogue.
    BlackWaveExit,
    /// Zio's third and later turns: ability `$54`, `BattleObj_BlackWave3`
    /// (`ps4.asm:66897`, "the instant-kill Black Wave"): its state `$C`
    /// (`loc_33B72`, `ps4.asm:66915-66919`) jumps to `loc_25048` (`ps4.asm:48916`),
    /// which kills the drawn target with no chance roll.
    BlackWaveKill,
}

/// One scripted arm: what runs, the ability byte it writes, and whether the arm
/// advances the counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Step {
    /// The turn the arm runs.
    pub turn: ZioTurn,
    /// What the arm writes to `$24(a4)` (`0` where it clears it).
    pub ability: u8,
    /// Whether the arm executes `addq.b #1, ($FFFFEE98).w`.
    pub advances: bool,
}

const fn step_of(turn: ZioTurn, ability: u8, advances: bool) -> Step {
    Step {
        turn,
        ability,
        advances,
    }
}

/// Whether `enemy_id`'s routine scripts every turn and never dispatches the
/// rolled ability: `EnemyAttack_Zio3` and `EnemyAttack_Zio` (Zio2 dispatches
/// it from its second action on).
pub(super) const fn ignores_roll(enemy_id: u16) -> bool {
    matches!(enemy_id, ZIO | ZIO3)
}

/// The scripted arm `enemy_id`'s routine runs with the counter at `phase`, or
/// `None` when it dispatches the rolled ability instead (every other enemy, and
/// Zio2 from its second action on: `ps4.asm:19531-19576`).
pub(super) const fn step(enemy_id: u16, phase: u8) -> Option<Step> {
    match (enemy_id, phase) {
        // `EnemyAttack_Zio3` (`ps4.asm:19410`): `$908`, `$90C`, nothing,
        // `$910`, `$914`, then `loc_D31C` for every later turn.
        (ZIO3, 0) => Some(step_of(ZioTurn::Barrier, 0x6B, true)),
        (ZIO3, 1) => Some(step_of(ZioTurn::Invocation, 0, true)),
        (ZIO3, 2) => Some(step_of(ZioTurn::Pause, 0, true)),
        (ZIO3, 3) => Some(step_of(ZioTurn::Nightmare, 0x53, true)),
        (ZIO3, 4) => Some(step_of(ZioTurn::BlackWaveExit, 0x70, true)),
        (ZIO3, _) => Some(step_of(ZioTurn::Pause, 0, false)),
        // `EnemyAttack_Zio` (`ps4.asm:19483`): `$908`, `$918`, then `loc_D392`
        // for every later turn, which does not advance the counter.
        (ZIO, 0) => Some(step_of(ZioTurn::Barrier, 0x6B, true)),
        (ZIO, 1) => Some(step_of(ZioTurn::Nightmare, 0x53, true)),
        (ZIO, _) => Some(step_of(ZioTurn::BlackWaveKill, 0x54, false)),
        // `EnemyAttack_Zio2` (`ps4.asm:19519`): only the first action is
        // scripted; `loc_D3F0` (`ps4.asm:19531`) onward dispatches `$24(a4)` as the roll left it.
        (ZIO2, 0) => Some(step_of(ZioTurn::Barrier, 0x6B, true)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "zio_phase_tests.rs"]
mod phase_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zio3_runs_five_scripted_turns_then_ends_every_turn() {
        let turns: Vec<ZioTurn> = (0..7)
            .filter_map(|p| step(ZIO3, p))
            .map(|s| s.turn)
            .collect();
        assert_eq!(
            turns,
            [
                ZioTurn::Barrier,
                ZioTurn::Invocation,
                ZioTurn::Pause,
                ZioTurn::Nightmare,
                ZioTurn::BlackWaveExit,
                ZioTurn::Pause,
                ZioTurn::Pause,
            ]
        );
        // `loc_D31C` is the only arm that does not `addq.b #1`.
        let advancing: Vec<bool> = (0..7)
            .filter_map(|p| step(ZIO3, p))
            .map(|s| s.advances)
            .collect();
        assert_eq!(advancing, [true, true, true, true, true, false, false]);
    }

    #[test]
    fn zio_scripts_barrier_nightmare_then_the_kill_forever() {
        assert_eq!(
            step(ZIO, 0).map(|s| (s.turn, s.ability)),
            Some((ZioTurn::Barrier, 0x6B))
        );
        assert_eq!(
            step(ZIO, 1).map(|s| (s.turn, s.ability)),
            Some((ZioTurn::Nightmare, 0x53))
        );
        for phase in [2, 3, 200] {
            let arm = step(ZIO, phase).expect("loc_D392 runs for every later turn");
            assert_eq!(
                (arm.turn, arm.ability, arm.advances),
                (ZioTurn::BlackWaveKill, 0x54, false)
            );
        }
    }

    #[test]
    fn zio2_scripts_only_its_first_action() {
        assert_eq!(step(ZIO2, 0).map(|s| s.turn), Some(ZioTurn::Barrier));
        assert_eq!(step(ZIO2, 1), None);
        assert_eq!(step(ZIO2, 9), None);
    }

    #[test]
    fn no_other_enemy_reads_the_counter() {
        for enemy in (0..160u16).filter(|e| ![ZIO, ZIO2, ZIO3].contains(e)) {
            assert_eq!(step(enemy, 0), None, "enemy {enemy}");
        }
    }
}
