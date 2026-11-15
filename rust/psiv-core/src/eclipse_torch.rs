//! The nine `$338` objects: US `$04FAE0`, `FieldObj_Animate2` `$04472E`,
//! and sequences `$050396/$0503A8/$0503BA`. Only an object pass advances them.

/// `$6` selects a long pointer in `$502C2`; each sequence has four mappings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum EclipseTorchSequence {
    /// Sequence `$50396`, duration byte 7.
    Fast = 0,
    /// Sequence `$503A8`, duration byte 11.
    Medium = 4,
    /// Sequence `$503BA`, duration byte 15.
    Slow = 8,
}

impl EclipseTorchSequence {
    /// The spawn's `$11`, also the selected sequence's reload duration byte.
    #[must_use]
    pub const fn initial_delay(self) -> u8 {
        match self {
            Self::Fast => 7,
            Self::Medium => 11,
            Self::Slow => 15,
        }
    }
}

/// Literal scene creation, in secondary-slot order (`$0702E4..$07041D`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EclipseTorchSpawn {
    /// Secondary object slot.
    pub slot: usize,
    /// Literal world pixel X.
    pub x: i32,
    /// Literal world pixel Y.
    pub y: i32,
    /// Spawn sequence and its retained initial delay.
    pub sequence: EclipseTorchSequence,
}

/// A read-only draw snapshot and the deterministic routine's private state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EclipseTorchObject {
    spawn: EclipseTorchSpawn,
    animation: crate::ActorAnimation,
    initialized: bool,
    active: bool,
}

impl EclipseTorchObject {
    /// Initialize a spawn without running its object routine.
    #[must_use]
    pub const fn new(spawn: EclipseTorchSpawn) -> Self {
        Self {
            spawn,
            animation: crate::ActorAnimation {
                word: spawn.sequence as u16,
                index: 0,
                duration: spawn.sequence.initial_delay(),
                durations: match spawn.sequence {
                    EclipseTorchSequence::Fast => &[7; 4],
                    EclipseTorchSequence::Medium => &[11; 4],
                    EclipseTorchSequence::Slow => &[15; 4],
                },
            },
            initialized: false,
            active: true,
        }
    }

    /// The unchanged scene creation record.
    #[must_use]
    pub const fn spawn(&self) -> EclipseTorchSpawn {
        self.spawn
    }

    /// No sprite has been built before the first owning object pass.
    #[must_use]
    pub const fn mapping(&self) -> Option<u8> {
        if self.active && self.initialized {
            Some(self.animation.index)
        } else {
            None
        }
    }

    /// Whether the routine has cleared its object word.
    #[must_use]
    pub const fn active(&self) -> bool {
        self.active
    }

    /// The current `$11` countdown byte.
    #[must_use]
    pub const fn duration(&self) -> u8 {
        self.animation.duration
    }

    /// `loc_4FAE0` tests index 3/duration 0 BEFORE `FieldObj_Animate2`.
    /// The latter subtracts a byte and advances on a signed underflow.
    pub fn object_pass(&mut self) {
        if !self.active {
            return;
        }
        self.initialized = true;
        if self.animation.index == 3 && self.animation.duration == 0 {
            self.active = false;
            return;
        }
        self.animation.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eclipse_torch_every_mapping_and_exact_clear_pass() {
        for (sequence, period, lifetime) in [
            (EclipseTorchSequence::Fast, 8, 32),
            (EclipseTorchSequence::Medium, 12, 48),
            (EclipseTorchSequence::Slow, 16, 64),
        ] {
            let mut object = EclipseTorchObject::new(EclipseTorchSpawn {
                slot: 0,
                x: 0,
                y: 0,
                sequence,
            });
            assert_eq!(object.mapping(), None);
            assert_eq!(object.duration(), period - 1);
            for pass in 1..=lifetime + 1 {
                object.object_pass();
                assert_eq!(object.active(), pass < lifetime, "{sequence:?} pass {pass}");
                if pass < lifetime {
                    assert_eq!(object.mapping(), Some(pass / period));
                    assert_eq!(object.duration(), period - 1 - pass % period);
                } else {
                    assert_eq!(object.mapping(), None);
                }
            }
        }
    }
}

#[cfg(test)]
mod scene_tests {

    #[test]
    fn eclipse_torch_nine_literal_spawns_and_groups() {
        let ops = crate::SCENES
            .iter()
            .find(|scene| scene.name == "Event_EclipseTorchUsed")
            .unwrap()
            .ops;
        let start = ops
            .iter()
            .position(|op| matches!(op, crate::SceneOp::CreateEclipseTorch(_)))
            .unwrap();
        let expected = [
            (0xBA0, 0xC0, 0, 7),
            (0xB80, 0xA0, 4, 11),
            (0xBA0, 0xA0, 4, 11),
            (0xBC0, 0xA0, 4, 11),
            (0xB60, 0x80, 8, 15),
            (0xB80, 0x80, 8, 15),
            (0xBA0, 0x80, 8, 15),
            (0xBC0, 0x80, 8, 15),
            (0xBE0, 0x80, 8, 15),
        ];
        for (slot, &(x, y, sequence, delay)) in expected.iter().enumerate() {
            let crate::SceneOp::CreateEclipseTorch(spawn) = ops[start + slot] else {
                panic!("consecutive creations without a wait")
            };
            assert_eq!(
                (
                    spawn.slot,
                    spawn.x,
                    spawn.y,
                    spawn.sequence as u16,
                    spawn.sequence.initial_delay()
                ),
                (slot, x, y, sequence, delay)
            );
        }
        assert_eq!(
            ops.iter()
                .filter(|op| matches!(op, crate::SceneOp::CreateEclipseTorch(_)))
                .count(),
            9
        );
    }

    /// Decode operands from the ignored local image, never embed ROM bytes.
    #[test]
    fn eclipse_torch_spawns_and_delays_match_decoded_us_operands() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../Phantasy Star IV (USA).md");
        let Ok(rom) = std::fs::read(path) else {
            eprintln!("US ROM absent; skipping operand comparison");
            return;
        };
        let word = |at| u16::from_be_bytes(rom[at..at + 2].try_into().unwrap());
        let long = |at| u32::from_be_bytes(rom[at..at + 4].try_into().unwrap());
        assert_eq!(word(0x702E6), 0x338, "object id");
        assert_eq!(word(0x702EA), 0x3AF, "loaded art tile");
        let ops = crate::SCENES
            .iter()
            .find(|scene| scene.name == "Event_EclipseTorchUsed")
            .unwrap()
            .ops;
        for spawn in ops.iter().filter_map(|op| {
            if let crate::SceneOp::CreateEclipseTorch(spawn) = op {
                Some(spawn)
            } else {
                None
            }
        }) {
            let at = 0x702EC + spawn.slot * 34;
            assert_eq!(word(at + 2), 0xC300 + spawn.slot as u16 * 0x40, "slot");
            assert_eq!(word(at + 12), spawn.x as u16, "X operand");
            assert_eq!(word(at + 18), spawn.y as u16, "Y operand");
            assert_eq!(
                rom[at + 25],
                spawn.sequence.initial_delay(),
                "initial delay"
            );
            assert_eq!(word(at + 30), spawn.sequence as u16, "sequence operand");
            let sequence = long(0x502C2 + spawn.sequence as usize) as usize;
            assert_eq!(rom[sequence], 4, "mapping count");
            assert_eq!(
                rom[sequence + 1],
                spawn.sequence.initial_delay(),
                "reload delay"
            );
        }
    }
}
