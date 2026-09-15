//! Camp healing, `CalcHealingValue` ($67FC0). This uses UpdateRNGSeed,
//! including repeated-value rejection, rather than battle's raster rolls.
use crate::battle::Rolls;

/// Compute one field healing amount. Sixteen accepted values in 0..7 are
/// accumulated; the previous value starts at zero and equal rolls retry.
/// Power and integer operation order follow the field routine.
#[must_use]
pub fn field_healing(mental: u8, power: u16, rolls: &mut impl Rolls) -> u16 {
    let mut previous = 0;
    let mut sum = 0u16;
    for _ in 0..16 {
        let value = loop {
            let value = rolls.next_roll() & 7;
            if value != previous {
                break value;
            }
        };
        previous = value;
        sum += value;
    }
    let product = sum.wrapping_add(8).wrapping_mul(u16::from(mental));
    (product >> 7)
        .wrapping_add(u16::from(mental) >> 1)
        .wrapping_add(power)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::SliceRolls;

    #[test]
    fn the_first_zero_and_consecutive_equal_values_retry() {
        let values = [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 0];
        let mut rolls = SliceRolls::new(&values);
        assert_eq!(field_healing(24, 24, &mut rolls), 48);
        assert_eq!(rolls.drawn(), 34);
    }

    #[test]
    fn field_rolls_change_the_amount_and_preserve_odd_mental_rounding() {
        let mut low = SliceRolls::new(&[1, 0]);
        let mut high = SliceRolls::new(&[7, 6]);
        assert_eq!(field_healing(25, 24, &mut low), 39);
        assert_eq!(field_healing(25, 24, &mut high), 57);
        assert_eq!((low.drawn(), high.drawn()), (16, 16));
    }
}
