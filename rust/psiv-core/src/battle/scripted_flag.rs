//! The scripted-battle flag, `$FFFFEE87`.
//!
//! Unlike the Zio phase counter ([`super::zio`]) this byte is a latch, not a
//! counter: a boss's init routine raises it, the boss's first attack lowers it,
//! and the battle's opening priority reads it. Every access in the image:
//!
//! | access | where | what |
//! |---|---|---|
//! | set | `EnemyInit_Zio` (`ps4.asm:17895-17898`), init routine of `$8B`, `$8C`, `$98` | `st ($FFFFEE87).w` |
//! | set | `loc_BAEC` (`ps4.asm:17945-17947`), called by `EnemyInit_DarkForce1` (`$82`) and `EnemyInit_DarkForce2` (`$83`) | the same |
//! | set | `EnemyInit_ProfoundDarkness1` (`ps4.asm:18007-18009`), `$85` | the same |
//! | set | `EnemyInit_CarnivorousTree` (`ps4.asm:18105-18108`, label `loc_BD4E`), `$81` | the same, beside `st (Battle_Priority).w` |
//! | read, clear | the first action of `EnemyAttack_ProfoundDarkness1` (`ps4.asm:19824-19828`), `_DarkForce2` (`19971-19975`), `_DarkForce1` (`20031-20035`) and `_CarnivorousTree` (`20102-20105`) | `tst.b` then `clr.b` and a fixed object with the ability cleared |
//! | read | `loc_B62A` (`ps4.asm:17448-17449`), the opening-priority routine | `tst.b ($FFFFEE87).w / beq.s / st d0` |
//!
//! Three readers are modelled here: with the flag up `Battle_Priority`
//! becomes `$FF`, an enemy ambush, whatever the chance roll and the boss
//! forcing said (`ps4.asm:17436-17452`; `Battle_ProcessCOMD`'s
//! `tst.b (Battle_Priority).w / bmi` at `ps4.asm:7636` then skips the party's
//! command input, and `Battle_OrderTurns` queues the enemies alone), and Dark
//! Force 1's first action ([`first_action`]): object `$818`, a charge-up
//! that requests nothing, replaces the rolled ability, and the captured fight
//! (`docs/oracle/BATTLE_ORACLE_ZELAN.md`) shows the byte cleared and no slot
//! resolved; its later turns, with the latch down, take the ordinary ability
//! roll into the damage registry; and Dark Force 2's ([`first_action`]):
//! object `$83C` (`loc_3161C`, `ps4.asm:63924-64034`) decompresses the boss's
//! plane, scrolls it in and ends the turn - no `UpdateRNGSeed2`, no request.
//! The other two first-action readers dispatch fixed objects whose chains have
//! not been read (`$864`, `$808`); none of those arms is a row of the damage
//! tables yet, so they keep the explicit unsupported path (issue #62's
//! ProfoundDarkness1 FIREBREATH row is the one waiting on them).

/// Whether the init routine of `enemy_id` raises `$FFFFEE87`.
///
/// `EnemyInitRoutinesOffs` (`ps4.asm:17731`) is indexed by enemy id: `$81`
/// CarnivorousTree, `$82` DarkForce1, `$83` DarkForce2, `$85`
/// ProfoundDarkness1, `$8B`/`$8C`/`$98` Zio, Zio2 and Zio3.
#[must_use]
pub(super) const fn init_raises(enemy_id: u16) -> bool {
    matches!(enemy_id, 0x81 | 0x82 | 0x83 | 0x85 | 0x8B | 0x8C | 0x98)
}

/// The fixed object `enemy_id`'s attack routine spends a raised latch on, when
/// this port models it: `130` DarkForce1 (`EnemyAttack_DarkForce1`,
/// `ps4.asm:20030`), whose object `$818` is presentation only, and `131`
/// DarkForce2 (`EnemyAttack_DarkForce2`, `ps4.asm:19970`), whose `$83C` is too.
#[must_use]
pub(super) const fn first_action(enemy_id: u16) -> Option<super::FirstZioAction> {
    match enemy_id {
        0x82 => Some(super::FirstZioAction::DarkForceCharge),
        0x83 => Some(super::FirstZioAction::DarkForceReveal),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_init_table_names_exactly_the_cartridge_setters() {
        let raising: Vec<u16> = (0..160).filter(|id| init_raises(*id)).collect();
        assert_eq!(raising, [129, 130, 131, 133, 139, 140, 152]);
    }
}
