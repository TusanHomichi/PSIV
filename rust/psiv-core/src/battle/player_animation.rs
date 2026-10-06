//! Animation-owned RNG before technique damage. The sprite coordinates are
//! presentation, but their draws rotate the same seed the damage pass reads.
//! Keep the integer timer consumers here even when no sprite is rendered.

use super::Rolls;

pub(super) fn technique(id: u8, rolls: &mut impl Rolls) {
    match id {
        12 => nazan(rolls),
        13..=15 => gravity(rolls),
        16 => {
            // loc_38DF8 (ps4.asm:73897-73913): a 60-tick phase emits
            // at positive multiples of eight, seven children. Each child's
            // initialization draws Y then X (73941-73959), before damage.
            for _ in 0..7 * 2 {
                rolls.next_roll();
            }
        }
        _ => {}
    }
}

pub(super) fn skill_before_damage(id: u8, rolls: &mut impl Rolls) {
    // BattleObj_Legeon / loc_3A206 (ps4.asm:75540-75573): eight
    // particles, Y then X, before the actor reaches its damage routine.
    if id == 13 {
        for _ in 0..16 {
            rolls.next_roll();
        }
    }
}

pub(super) fn skill_after_damage(id: u8, rolls: &mut impl Rolls) {
    // BattleObj_PosiboltChild / loc_3A706 (ps4.asm:75931-75970):
    // twelve trailing particles each draw a Y offset after the damage pass.
    if id == 15 {
        for _ in 0..12 {
            rolls.next_roll();
        }
    }
}

fn nazan(rolls: &mut impl Rolls) {
    // loc_388B6/loc_388DC (ps4.asm:73482-73504): initial delay & $F,
    // then a $BF-tick phase emits children while its timer exceeds $10.
    // BattleObj_NazanChild2 (73594-73613) draws its Y offset on the next
    // tick, before the parent, including when another child emits that tick.
    let mut delay = (rolls.next_roll() & 15) as i16;
    let mut child = false;
    for _ in 0..(0xBF - 0x10) {
        if child {
            rolls.next_roll();
        }
        delay -= 1;
        child = delay <= 0;
        if child {
            delay = (rolls.next_roll() & 15) as i16;
        }
    }
    if child {
        rolls.next_roll();
    }
}

fn gravity(rolls: &mut impl Rolls) {
    // BattleObj_GraChild (ps4.asm:73770-73797) emits every (& $F)+4
    // ticks while the parent's initial 4 and subsequent $28 timers run
    // (loc_38ADC/loc_38B18/loc_38B6C, 73639-73708). Each child draws
    // its coordinate in BattleObj_GraChild2 (73800-73817).
    let mut tick = 0;
    while tick < 4 + 0x28 {
        tick += (rolls.next_roll() & 15) + 4;
        rolls.next_roll();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::SliceRolls;

    #[test]
    fn nazan_counts_child_coordinates_separately_from_delay_draws() {
        let mut fast = SliceRolls::new(&[0]);
        nazan(&mut fast);
        assert_eq!(fast.drawn(), 1 + 175 * 2);
        let mut slow = SliceRolls::new(&[15]);
        nazan(&mut slow);
        assert_eq!(slow.drawn(), 1 + 11 * 2);
    }

    #[test]
    fn gravity_coordinate_values_do_not_control_emission_delays() {
        let mut fast = SliceRolls::new(&[0, 15]);
        gravity(&mut fast);
        assert_eq!(fast.drawn(), 22);
        let mut slow = SliceRolls::new(&[15, 0]);
        gravity(&mut slow);
        assert_eq!(slow.drawn(), 6);
        let mut unchanged = SliceRolls::new(&[0]);
        technique(11, &mut unchanged);
        assert_eq!(unchanged.drawn(), 0);
    }

    #[test]
    fn megid_draws_coordinates_for_seven_children_not_per_damage_target() {
        let mut rolls = SliceRolls::new(&[0, u16::MAX]);
        technique(16, &mut rolls);
        assert_eq!(rolls.drawn(), 14);
        let mut unchanged = SliceRolls::new(&[0]);
        technique(17, &mut unchanged);
        assert_eq!(unchanged.drawn(), 0);
    }
}
