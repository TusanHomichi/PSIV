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

/// The chip-write trace owned by a [`SoundMachine`](crate::SoundMachine).
///
/// Tracing is opt-in ([`SoundMachine::with_register_trace`](crate::SoundMachine::with_register_trace)).
/// A traced machine appends one record per write, and a DAC sample writes
/// YM `$2A` about 21,000 times a second, so an untraced machine — the live
/// game's — must record nothing: an always-on trace grew without bound for
/// the life of the process (#92).
#[derive(Clone, Debug, Default)]
pub struct RegisterLog {
    /// `None` when tracing is off; writes then cost nothing.
    writes: Option<Vec<RegisterWrite>>,
}

impl RegisterLog {
    pub(crate) fn untraced() -> Self {
        Self { writes: None }
    }

    pub(crate) fn traced() -> Self {
        Self {
            writes: Some(Vec::new()),
        }
    }

    pub(crate) fn clear(&mut self) {
        if let Some(writes) = self.writes.as_mut() {
            writes.clear();
        }
    }

    pub(crate) fn push(&mut self, write: RegisterWrite) {
        if let Some(writes) = self.writes.as_mut() {
            writes.push(write);
        }
    }

    /// The writes since the previous take, or `None` on an untraced log.
    pub(crate) fn take(&mut self) -> Option<Vec<RegisterWrite>> {
        self.writes.as_mut().map(std::mem::take)
    }
}

impl RegisterLog {
    /// Whether this log records writes.
    pub fn is_traced(&self) -> bool {
        self.writes.is_some()
    }

    /// Views the writes without consuming them; `None` when tracing is off.
    pub fn writes(&self) -> Option<&[RegisterWrite]> {
        self.writes.as_deref()
    }
}

#[cfg(test)]
#[path = "trace_tests.rs"]
mod trace_tests;
