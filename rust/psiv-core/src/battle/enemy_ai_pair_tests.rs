//! The arms whose reading changed with lane A5: the BladeRight/HakenLeft pair
//! (`$0D`/`$0E`), Lashiec's `$11` and its `$FFFFEE86` latch, and the bit-4
//! clear `$12` makes before it reads the three slots.

use super::tests::{ability_of, hp, id, one_arm, roster, scan};
use super::*;
use crate::battle::rng::SliceRolls;

// --- arms $0D / $0E: the partner pair -------------------------------------

#[test]
fn each_partner_arm_fires_for_exactly_one_partner() {
    // 84 BladeRight names $0D: one HakenLeft beside it and no second
    // BladeRight. `moveq #-1, d5` plus one `addq.b #1` is the zero that fires
    // (`ps4.asm:21630`, `21641`, `21645-21646`).
    let blade = one_arm(0x0D, 58);
    let mut r = roster(&[
        (1, enemy_id::HAKEN_LEFT, [0x0E; 4], [59; 4]),
        (2, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4]),
    ]);
    let (outcome, drawn) = scan(&mut r, 7, &blade, &[]);
    assert_eq!(ability_of(&blade, outcome, 0), 58);
    assert_eq!(drawn, 0);

    // And 86 HakenLeft names $0E, the same loop with the ids swapped
    // (`ps4.asm:21557-21582`).
    let haken = one_arm(0x0E, 59);
    let (outcome, _) = scan(&mut r, 6, &haken, &[]);
    assert_eq!(ability_of(&haken, outcome, 0), 59);
}

#[test]
fn the_partner_arms_hold_off_without_exactly_one_partner() {
    let blade = one_arm(0x0D, 58);
    for (rows, why) in [
        // Alone: no HakenLeft leaves `d5` at `$FF` (the port's old reading
        // fired here, and so met COMBINE where the cartridge never does).
        (
            vec![(2u8, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4])],
            "no partner",
        ),
        // Beside other enemies but no HakenLeft: the Air Castle formations
        // `$175`/`$176` (BladeRight with two FrostSabers).
        (
            vec![
                (1, 71, [0; 4], [0; 4]),
                (2, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4]),
                (3, 71, [0; 4], [0; 4]),
            ],
            "no partner among others",
        ),
        // A second BladeRight leaves at once, whatever else stands.
        (
            vec![
                (1, enemy_id::HAKEN_LEFT, [0x0E; 4], [59; 4]),
                (2, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4]),
                (3, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4]),
            ],
            "a second of its own kind",
        ),
        // Two HakenLefts count `d5` to 1.
        (
            vec![
                (1, enemy_id::HAKEN_LEFT, [0x0E; 4], [59; 4]),
                (2, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4]),
                (3, enemy_id::HAKEN_LEFT, [0x0E; 4], [59; 4]),
            ],
            "two partners",
        ),
    ] {
        let mut r = roster(&rows);
        assert_eq!(scan(&mut r, 7, &blade, &[]).0, AiOutcome::Rolled, "{why}");
    }
}

#[test]
fn a_fallen_partner_or_rival_is_not_counted() {
    // `_tst.w 0(a2)` skips an emptied slot: formation `$1BA` (two BladeRights
    // and a HakenLeft) combines once one BladeRight is down.
    let blade = one_arm(0x0D, 58);
    let mut r = roster(&[
        (1, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4]),
        (2, enemy_id::BLADE_RIGHT, [0x0D; 4], [58; 4]),
        (3, enemy_id::HAKEN_LEFT, [0x0E; 4], [59; 4]),
    ]);
    assert_eq!(scan(&mut r, 6, &blade, &[]).0, AiOutcome::Rolled);
    r.get_mut(id(7)).expect("seated").mark_defeated();
    assert!(matches!(
        scan(&mut r, 6, &blade, &[]).0,
        AiOutcome::Replaced { .. }
    ));
}

// --- arm $11: HP25PercentOrLower ------------------------------------------

fn lashiec_scan(roster: &mut Roster, flags: AiFlags) -> AiOutcome {
    let record = one_arm(0x11, 98);
    let mut rolls = SliceRolls::new(&[]);
    instruction_block(roster, id(6), &record, flags, &mut rolls).expect("a table id")
}

#[test]
fn the_quarter_arm_fires_at_or_below_a_quarter_while_the_latch_is_down() {
    let mut r = roster(&[(1, 128, [0x11; 4], [98; 4])]);
    // `lsr.w #2` of 1000 is 250: `cmp.w curr_hp, d1 / bcs` holds at 250.
    hp(&mut r, 6, 250, 1000);
    assert!(matches!(
        lashiec_scan(&mut r, AiFlags::default()),
        AiOutcome::Replaced { slot: 0, .. }
    ));
    hp(&mut r, 6, 251, 1000);
    assert_eq!(lashiec_scan(&mut r, AiFlags::default()), AiOutcome::Rolled);
}

#[test]
fn the_raised_latch_holds_the_quarter_arm_off() {
    // `tst.b ($FFFFEE86).w / bne.s loc_E29C` (`ps4.asm:20492-20493`) comes
    // before the HP test: after one REINFORCE the arm never fires again.
    let mut r = roster(&[(1, 128, [0x11; 4], [98; 4])]);
    hp(&mut r, 6, 1, 1000);
    assert_eq!(
        lashiec_scan(&mut r, AiFlags { reinforced: true }),
        AiOutcome::Rolled
    );
}

// --- arm $12: the clear comes before the slot test -------------------------

#[test]
fn three_xe_a_thouls_clears_the_bits_even_when_a_slot_is_not_a_xe_a_thoul() {
    // `ps4.asm:20366-20371` clears bit 4 on slots 1..3 as soon as the actor's
    // own bit is set; the loop at 20375-20385 then finds slot 3 is not a
    // XeAThoul and the arm does not fire.
    let record = one_arm(0x12, 92);
    let mut r = roster(&[
        (1, enemy_id::XE_A_THOUL, [0x12; 4], [92; 4]),
        (2, enemy_id::XE_A_THOUL, [0x12; 4], [92; 4]),
        (3, 11, [0; 4], [0; 4]),
    ]);
    for slot in 6..=8u8 {
        r.get_mut(id(slot)).expect("seated").reaction_flags = reaction::MULTI_TARGET;
    }
    assert_eq!(scan(&mut r, 6, &record, &[]).0, AiOutcome::Rolled);
    for slot in 6..=8u8 {
        assert_eq!(
            r.get(id(slot)).expect("seated").reaction_flags & reaction::MULTI_TARGET,
            0,
            "slot {slot}'s bit 4"
        );
    }
}
