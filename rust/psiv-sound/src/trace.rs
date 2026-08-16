//! Chip-write tracing.  This is the stable comparison surface for oracle
//! captures; PCM is deliberately not the primary fixture oracle.

/// The physical write target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Chip {
    Ym2612,
    Sn76489,
}

/// One logical chip write at a driver tick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterWrite {
    pub tick: u64,
    pub chip: Chip,
    /// YM port (0 or 1).  PSG writes use zero.
    pub port: u8,
    /// YM register.  PSG writes use zero because the byte is in `value`.
    pub register: u8,
    pub value: u8,
}

/// Append-only trace owned by a [`SoundMachine`](crate::SoundMachine).
#[derive(Clone, Debug, Default)]
pub struct RegisterLog {
    writes: Vec<RegisterWrite>,
}

impl RegisterLog {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn clear(&mut self) {
        self.writes.clear();
    }

    pub(crate) fn push(&mut self, write: RegisterWrite) {
        self.writes.push(write);
    }

    pub(crate) fn take(&mut self) -> Vec<RegisterWrite> {
        std::mem::take(&mut self.writes)
    }
}

impl RegisterLog {
    /// Views the writes without consuming them.
    pub fn writes(&self) -> &[RegisterWrite] {
        &self.writes
    }
}
