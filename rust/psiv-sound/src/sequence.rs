//! Extraction-neutral PSIV sound records and validation.

pub(crate) const FM_CHANNELS: usize = 6;
pub(crate) const PSG_CHANNELS: usize = 3;

/// The physical track family. FM channel numbers are 0..5; PSG channel
/// numbers are 0..2. A track may request no channel and use allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackKind {
    Fm,
    Psg,
}

/// One PSIV FM instrument: algorithm/feedback, twenty operator bytes, and
/// four total-level bytes, matching the driver's 25-byte local voice record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FmVoice {
    pub algorithm: u8,
    pub operators: [u8; 20],
    pub levels: [u8; 4],
}

impl FmVoice {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SequenceError> {
        if bytes.len() != 25 {
            return Err(SequenceError::InvalidVoiceLength {
                actual: bytes.len(),
            });
        }
        let mut operators = [0; 20];
        operators.copy_from_slice(&bytes[1..21]);
        let mut levels = [0; 4];
        levels.copy_from_slice(&bytes[21..25]);
        Ok(Self {
            algorithm: bytes[0],
            operators,
            levels,
        })
    }

    pub fn fixture() -> Self {
        Self {
            algorithm: 0x07,
            operators: [
                0x01, 0x01, 0x01, 0x01, 0x1f, 0x1f, 0x1f, 0x1f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x0f, 0x0f, 0x0f, 0x0f,
            ],
            levels: [0x10, 0x10, 0x10, 0x10],
        }
    }
}

/// A PSG envelope slice. The values are signed attenuation deltas; control
/// bytes from the retail envelope table are intentionally left for the
/// extraction lane to normalize into this small runtime view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PsgEnvelope {
    pub values: Vec<i8>,
}

impl PsgEnvelope {
    pub fn new(values: impl Into<Vec<i8>>) -> Self {
        Self {
            values: values.into(),
        }
    }
}

/// Raw command bytes plus the channel-local metadata resolved by extraction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoundTrack {
    pub kind: TrackKind,
    pub channel: Option<u8>,
    pub bytes: Vec<u8>,
    pub initial_voice: Option<u8>,
    pub initial_pan: u8,
    pub initial_transpose: i8,
    pub tick_multiplier: u8,
}

impl SoundTrack {
    pub fn fm(channel: u8, bytes: impl Into<Vec<u8>>) -> Self {
        Self::with_channel(TrackKind::Fm, Some(channel), bytes)
    }

    pub fn psg(channel: u8, bytes: impl Into<Vec<u8>>) -> Self {
        Self::with_channel(TrackKind::Psg, Some(channel), bytes)
    }

    pub fn fm_auto(bytes: impl Into<Vec<u8>>) -> Self {
        Self::with_channel(TrackKind::Fm, None, bytes)
    }

    pub fn psg_auto(bytes: impl Into<Vec<u8>>) -> Self {
        Self::with_channel(TrackKind::Psg, None, bytes)
    }

    pub fn with_channel(kind: TrackKind, channel: Option<u8>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            kind,
            channel,
            bytes: bytes.into(),
            initial_voice: None,
            initial_pan: 0xc0,
            initial_transpose: 0,
            tick_multiplier: 1,
        }
    }

    pub fn voice(mut self, index: u8) -> Self {
        self.initial_voice = Some(index);
        self
    }

    pub fn pan(mut self, value: u8) -> Self {
        self.initial_pan = value;
        self
    }
}

/// A sequence is intentionally an extraction-neutral Rust value. The future
/// `runtime-pack/sound/` loader should map raw/resolved JSON directly into it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoundSequence {
    pub id: u8,
    pub tempo: u8,
    pub fm_voices: Vec<FmVoice>,
    pub psg_envelopes: Vec<PsgEnvelope>,
    pub tracks: Vec<SoundTrack>,
}

impl SoundSequence {
    pub fn new(
        id: u8,
        tempo: u8,
        fm_voices: Vec<FmVoice>,
        psg_envelopes: Vec<PsgEnvelope>,
        tracks: Vec<SoundTrack>,
    ) -> Self {
        Self {
            id,
            tempo: tempo.max(1),
            fm_voices,
            psg_envelopes,
            tracks,
        }
    }

    pub fn fixture_tone() -> Self {
        Self::new(
            0x84,
            1,
            vec![FmVoice::fixture()],
            Vec::new(),
            vec![SoundTrack::fm(
                0,
                vec![0xef, 0x00, 0x8c, 0x20, 0x80, 0x20, 0xf2],
            )],
        )
    }

    pub fn validate(&self) -> Result<(), SequenceError> {
        for track in &self.tracks {
            if let Some(channel) = track.channel {
                let limit = match track.kind {
                    TrackKind::Fm => FM_CHANNELS,
                    TrackKind::Psg => PSG_CHANNELS,
                };
                if usize::from(channel) >= limit {
                    return Err(SequenceError::InvalidChannel {
                        kind: track.kind,
                        channel,
                    });
                }
            }
            if track.tick_multiplier == 0 {
                return Err(SequenceError::ZeroTickMultiplier);
            }
        }
        Ok(())
    }
}

/// Malformed extraction data is observable and diagnosable rather than
/// becoming a silent stream divergence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SequenceError {
    InvalidVoiceLength { actual: usize },
    InvalidChannel { kind: TrackKind, channel: u8 },
    ZeroTickMultiplier,
    UnexpectedEnd { command: u8, cursor: usize },
    InvalidJump { cursor: usize, target: isize },
    CallStackOverflow,
    CallStackUnderflow,
    UnsupportedMeta { value: u8 },
    CommandBudgetExceeded { cursor: usize },
    MissingVoice { index: usize },
    MissingEnvelope { index: usize },
}

impl std::fmt::Display for SequenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "sound sequence error: {self:?}")
    }
}

impl std::error::Error for SequenceError {}
