//! Decoded `MapUpdateJmpTbl` programs; rules execute in psiv-runtime.

use serde::{Deserialize, Serialize};

/// One table entry, bound in the map's cartridge order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapUpdate {
    /// Byte index in the 64-entry jump table.
    pub index: u8,
    /// Disassembly symbol, for provenance.
    pub routine: String,
    /// Retail routine address.
    pub address: String,
    /// Decoded operation and its extracted inputs.
    pub program: UpdateProgram,
}

/// A flag read by an update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateGate {
    /// Physical flag door, independent of fork labels.
    pub bank: UpdateBank,
    /// Bit id in that bank.
    pub flag: u8,
    /// Required value.
    pub set: bool,
}

/// The three flag-test doors used by these routines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateBank {
    /// Base events, $F100.
    Event,
    /// Extended events/chests, $F120.
    Chest,
    /// Temporary events, $F140.
    Temp,
}

/// One CRAM-shadow word write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteWrite {
    /// CRAM slot, 0..63.
    pub slot: u8,
    /// Raw word, including hardware-ignored bits.
    pub word: u16,
}

/// Phase selection for a palette table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PalettePhase {
    /// `(Main_Frame_Count & mask) >> shift`.
    Frame {
        /// Retail phase mask.
        mask: u16,
        /// Shift including byte-offset to word-index conversion.
        shift: u8,
    },
    /// A wrapping byte counter, reset on reaching the extracted table length.
    Counter {
        /// $ECF2 = 0, $ECF3 = 1.
        slot: u8,
    },
}

/// One independently clocked set of palette writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteCycle {
    /// Execute when clock & frame_mask == 0.
    pub frame_mask: u16,
    /// Use preincremented $ECF2 as clock instead of Main_Frame_Count.
    pub timer: bool,
    /// How the table phase is selected.
    pub phase: PalettePhase,
    /// Exact table reads, with source displacements already resolved.
    pub frames: Vec<Vec<PaletteWrite>>,
}

/// Scroll helper's treatment of the sine byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollMode {
    /// Logical shifts of the sine byte, Garuberk horizontal scroll.
    Unsigned,
    /// Sign extension and ASR, The Edge horizontal scroll.
    Signed,
    /// Sign extension and ASR, the unused vertical scroll entry.
    Vertical,
}

/// Update instructions and extracted tables. Unsupported hardware inputs are
/// explicit records, never substituted with invented state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UpdateProgram {
    /// Retail `moveq #0,d0; rts`.
    Noop,
    /// Conditional palette table copies.
    Palette {
        /// All gates must hold unless bypass is set.
        gates: Vec<UpdateGate>,
        /// Reunion can bypass the shutdown gates.
        bypass: Option<u8>,
        /// Independent clocks, evaluated in cartridge order.
        cycles: Vec<PaletteCycle>,
        /// Literal writes on the stopped branch, if any.
        stopped: Vec<PaletteWrite>,
    },
    /// Camera-step damping: `step -= step ASR shift`.
    Camera {
        /// FG rather than BG.
        foreground: bool,
        /// X arithmetic shift.
        x_shift: u8,
        /// Y arithmetic shift.
        y_shift: u8,
    },
    /// Zio tunnels' sparse horizontal-scroll writes and BG X ASR 2.
    Tunnels,
    /// Sine-based scroll table writes.
    Scroll {
        /// Signedness and destination.
        mode: ScrollMode,
        /// 256 helper results, as unsigned bytes.
        samples: Vec<u8>,
    },
    /// Rykros countdown, flash sequence and RNG2 reload.
    Crystals {
        /// Four flash words from the cartridge table.
        colors: Vec<u16>,
        /// Literal resting color.
        rest: u16,
    },
    /// Chest predicates feeding a base event, or the unused chest clear.
    Flags {
        /// Required $F120 chest bits.
        chests: Vec<u8>,
        /// Destination bit.
        flag: u8,
        /// Clear a chest rather than set an event.
        clear_chest: bool,
    },
    /// The Edge clears fourteen words, then rotates five extracted colors.
    Edge {
        /// The five table words.
        colors: Vec<u16>,
    },
    /// The Edge's per-map palette blob reads.
    EdgeLine {
        /// Two-byte sliding window rather than 28-byte records.
        sliding: bool,
        /// Resolved backdrop and fourteen line-1 writes per phase.
        frames: Vec<Vec<PaletteWrite>>,
    },
    /// Ladea/Air Castle saved sound and secondary-map word writes.
    Music {
        /// $EC2A word.
        secondary_map: u16,
        /// $ECEC byte, not a sound-play command.
        sound: u8,
    },
    /// Random_Battles_Flag from the leader's current Y pixel word.
    Encounters {
        /// Disabled at and below this unsigned word.
        y_max: u16,
    },
    /// Exact input that the native runtime does not yet represent (#41).
    Unsupported {
        /// The absent retail buffer/subsystem.
        missing: String,
    },
}

impl MapUpdate {
    /// Check bounds before any runtime indexes a decoded program.
    pub(crate) fn valid(&self) -> bool {
        let writes = |frames: &[Vec<PaletteWrite>]| {
            !frames.is_empty()
                && frames.len() <= 256
                && frames.iter().all(|row| {
                    !row.is_empty() && row.len() <= 64 && row.iter().all(|w| w.slot < 64)
                })
        };
        self.index < 64
            && !self.routine.is_empty()
            && match &self.program {
                UpdateProgram::Palette {
                    cycles, stopped, ..
                } => {
                    !cycles.is_empty()
                        && cycles.iter().all(|cycle| {
                            writes(&cycle.frames)
                                && match cycle.phase {
                                    PalettePhase::Counter { slot } => slot < 2,
                                    PalettePhase::Frame { mask, shift } => {
                                        shift < 16
                                            && usize::from(mask >> shift) < cycle.frames.len()
                                    }
                                }
                        })
                        && stopped.iter().all(|w| w.slot < 64)
                }
                UpdateProgram::Camera {
                    x_shift, y_shift, ..
                } => (1..=7).contains(x_shift) && (1..=7).contains(y_shift),
                UpdateProgram::Scroll { samples, .. } => samples.len() == 256,
                UpdateProgram::Crystals { colors, .. } => colors.len() == 4,
                UpdateProgram::Edge { colors } => colors.len() == 5,
                UpdateProgram::EdgeLine { sliding, frames } => {
                    frames.len() == if *sliding { 14 } else { 28 } && writes(frames)
                }
                UpdateProgram::Unsupported { missing } => !missing.is_empty(),
                _ => true,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_table_bounds_are_rejected_before_frame_execution() {
        let mut entry = MapUpdate {
            index: 2,
            routine: "test".into(),
            address: "test".into(),
            program: UpdateProgram::Palette {
                gates: vec![],
                bypass: None,
                stopped: vec![],
                cycles: vec![PaletteCycle {
                    frame_mask: 7,
                    timer: false,
                    phase: PalettePhase::Frame { mask: 0, shift: 0 },
                    frames: vec![vec![PaletteWrite { slot: 0, word: 0 }]],
                }],
            },
        };
        assert!(entry.valid());
        let UpdateProgram::Palette { cycles, .. } = &mut entry.program else {
            unreachable!()
        };
        cycles[0].frames[0][0].slot = 64;
        assert!(!entry.valid(), "out-of-CRAM write negative control");
        let UpdateProgram::Palette { cycles, .. } = &mut entry.program else {
            unreachable!()
        };
        cycles[0].frames[0][0].slot = 0;
        cycles[0].phase = PalettePhase::Frame { mask: 8, shift: 0 };
        assert!(!entry.valid(), "out-of-table phase negative control");
        let UpdateProgram::Palette { cycles, .. } = &mut entry.program else {
            unreachable!()
        };
        cycles[0].phase = PalettePhase::Counter { slot: 2 };
        assert!(!entry.valid(), "absent counter negative control");
    }
}
