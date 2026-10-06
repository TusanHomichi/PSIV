//! GRA's spark chain: the one damage route whose draws depend on what it draws.
//!
//! `EnemyAttack_DimensWorm`'s `$31` arm (`loc_F7BE`, `ps4.asm:21909-21931`)
//! loads three objects into the slots from `$FFFFDD00`: `$2E0` (`loc_1C99C`,
//! `ps4.asm:39050`), `$2DC` (`loc_1C73E`, `38879`) and `$2D8` (`loc_1C658`,
//! `38807`). Twenty counts into its third state `$2DC` walks the five party
//! slots (`ps4.asm:38951-38969`) and, for each occupied one whose status lacks
//! the death bit (`$40` for a profession-5 android, `$04` otherwise), loads one
//! spark `$2E4` (`loc_1C58E`, `38756`) from `$FFFFDA00` through `loc_1C8B6`
//! (`38984`), which takes one `UpdateRNGSeed2` call (38990) and stores
//! `(roll & 7) + 6` as the spark's countdown (38996-39003).
//!
//! Every frame a spark runs, it counts down (`subq.w #1, $1C(a4)`, 38768); the
//! frame it reaches zero it marks itself fired and, unless `$FFFFEE85` reads
//! `$A` (38771-38772), loads the next spark from `$FFFFDA00` with one more call
//! and a countdown read the same way (38773-38791). It then animates
//! (`Battle_AnimateSprite`, 38796) through `loc_25CCC0`'s five two-frame
//! mappings (`ps4.asm:302682-302741`), which raises the animation-done bit on
//! its eleventh run (`Battle_AnimateSprite`, `72526-72549`); a fired spark that
//! sees the bit frees its slot (38797-38802).
//!
//! `$FFFFEE85` is `$2E0`'s: it counts `$30` frames once the actor's sprite is
//! ready, then `$1E` more, and writes `$A` (39069-39087). `$2DC` waits for the
//! same sprite state, counts 8, plays `loc_1C8A0` through `loc_256F4`
//! (4 + 4 + 4 + 8 calls, `ps4.asm:38973-38982`, `49534-49566`) and counts `$14`
//! more before the first sparks, so the flag rises [`WINDOW`] frames after them,
//! in a slot past every spark's: a spark that fires in that frame still draws.
//!
//! Which frame a new spark first runs depends on where `Battle_LoadObject`
//! (`ps4.asm:311446`) puts it - the first free slot from `$FFFFDA00` - relative
//! to the spark that loaded it: a later slot runs in the same frame, an earlier
//! one in the next. So the chain is simulated slot by slot. The captured GRA
//! turns of `replay_fixtures/air_castle/` are the measurement this was checked
//! against (`docs/battle/ENEMY_ABILITIES_AIR_CASTLE.md`).

use super::super::Rolls;

/// Frames from the first sparks to `$FFFFEE85 = $A`: `$2DC`'s `8 + 20 + 19`
/// frames after the shared sprite test against `$2E0`'s `47 + 29`.
pub(super) const WINDOW: u16 = 30;

/// The run on which a spark's five two-frame mappings raise the done bit.
const ANIMATION_RUNS: u16 = 11;

/// One live spark object.
#[derive(Debug, Clone, Copy)]
struct Spark {
    /// `$1C(a4)`: frames left before it fires.
    countdown: u16,
    /// Frames it has run.
    runs: u16,
    /// `$4(a4)` bit 0: it has fired.
    fired: bool,
    /// The first frame (from the first sparks' frame, 0) it runs in.
    first: u16,
}

impl Spark {
    fn new(roll: u16, first: u16) -> Spark {
        Spark {
            countdown: (roll & 7) + 6,
            runs: 0,
            fired: false,
            first,
        }
    }
}

/// The first free slot from `$FFFFDA00`, as an index into the spark slots.
fn first_free(slots: &[Option<Spark>]) -> usize {
    slots
        .iter()
        .position(Option::is_none)
        .unwrap_or(slots.len())
}

fn place(slots: &mut Vec<Option<Spark>>, index: usize, spark: Spark) {
    if index == slots.len() {
        slots.push(Some(spark));
    } else {
        slots[index] = Some(spark);
    }
}

/// Every call the chain makes for `living` sparked party members, in order,
/// and how many there were.
///
/// The first sparks are loaded in one frame by `$2DC`, whose slot follows
/// theirs, so each first runs the next frame. Objects other than sparks in
/// the `$FFFFDA00` range only renumber the free slots, which keeps their order:
/// at most two sparks per chain are alive at once, so ten fit before `$2E0`'s
/// slot and none is ever run after the flag in the frame it rises.
pub(super) fn draws(living: usize, rolls: &mut impl Rolls) -> u16 {
    let mut slots: Vec<Option<Spark>> = Vec::new();
    let mut taken = 0u16;
    for _ in 0..living {
        let roll = rolls.next_roll();
        taken += 1;
        let index = first_free(&slots);
        place(&mut slots, index, Spark::new(roll, 1));
    }
    for frame in 1..=WINDOW {
        let mut index = 0;
        while index < slots.len() {
            let Some(mut spark) = slots[index] else {
                index += 1;
                continue;
            };
            if spark.first > frame {
                index += 1;
                continue;
            }
            spark.runs += 1;
            if !spark.fired {
                spark.countdown -= 1;
                if spark.countdown == 0 {
                    spark.fired = true;
                    let roll = rolls.next_roll();
                    taken += 1;
                    // The spawner still holds its own slot here.
                    slots[index] = Some(spark);
                    let child = first_free(&slots);
                    let first = if child > index { frame } else { frame + 1 };
                    place(&mut slots, child, Spark::new(roll, first));
                }
            }
            slots[index] = if spark.fired && spark.runs >= ANIMATION_RUNS {
                None
            } else {
                Some(spark)
            };
            index += 1;
        }
    }
    taken
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::rng::SliceRolls;

    /// Captured `forced_167` GRA turns (`build/a5-evidence/captures/gra-167`):
    /// the calls from the first sparks to the damage request, three living
    /// party members each time. The simulation must take exactly these.
    const CAPTURED: [&[u16]; 3] = [
        &[
            0x5517, 0x794D, 0x8D3C, 0x9270, 0x1421, 0x5066, 0x16FB, 0xC1B9, 0xB49D, 0x295B, 0xE338,
            0xFBE7,
        ],
        &[
            0xABBF, 0xA2E3, 0x9F98, 0xD2B4, 0xD935, 0x99CA, 0xAA77, 0x2CD0, 0x4461, 0x443E, 0xA8F8,
            0x5D11,
        ],
        &[
            0x969B, 0x18CF, 0x5B91, 0xD683, 0xB3B9, 0xFE5B, 0x0BA2, 0x5F6A, 0xBFCA, 0x4F06, 0x82A2,
            0x22DA, 0xEC92,
        ],
    ];

    #[test]
    fn the_chain_takes_exactly_the_captured_calls() {
        for stream in CAPTURED {
            // A tail the chain must not reach.
            let mut padded = stream.to_vec();
            padded.extend([0xFFFF; 8]);
            let mut rolls = SliceRolls::new(&padded);
            assert_eq!(usize::from(draws(3, &mut rolls)), stream.len());
            assert_eq!(rolls.drawn(), stream.len());
        }
    }

    #[test]
    fn no_living_member_means_no_spark_and_no_call() {
        let mut rolls = SliceRolls::new(&[0x1234]);
        assert_eq!(draws(0, &mut rolls), 0);
        assert_eq!(rolls.drawn(), 0);
    }

    #[test]
    fn the_countdown_is_the_low_three_bits_plus_six() {
        // Every first spark at countdown 13 (`& 7 = 7`): one fires per chain at
        // frame 13, its child (`& 7 = 7` again) at 13 + 13 - 1 = 25 when it
        // lands in a later slot; nothing else fits in thirty frames.
        let mut rolls = SliceRolls::new(&[0x0007]);
        assert_eq!(draws(1, &mut rolls), 3);
        // Countdown 6 everywhere fires on frames 6, 11, 17, 22 and 28: a child
        // that lands after its parent runs the same frame (6 -> 11), one that
        // lands in the slot its grandparent just freed runs the next (11 -> 17).
        let mut rolls = SliceRolls::new(&[0x0000]);
        assert_eq!(draws(1, &mut rolls), 6);
        // Five chains at the fastest countdown: the most a GRA can take.
        let mut rolls = SliceRolls::new(&[0x0000]);
        assert_eq!(draws(5, &mut rolls), 30);
    }
}
