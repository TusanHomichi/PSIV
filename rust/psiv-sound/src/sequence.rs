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

/// A PSG envelope slice. `values` is retained as a convenient view for simple
/// callers; `controls` is the byte-exact retail stream, including RESET/HOLD/
/// JUMP/OFF controls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PsgEnvelope {
    pub values: Vec<i8>,
    pub controls: Vec<u8>,
}

impl PsgEnvelope {
    pub fn new(values: impl Into<Vec<i8>>) -> Self {
        let values = values.into();
        Self {
            controls: values.iter().map(|value| *value as u8).collect(),
            values,
        }
    }

    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        let controls = bytes.into();
        let values = controls
            .iter()
            .copied()
            .filter(|value| *value < 0x80)
            .map(|value| value as i8)
            .collect();
        Self { values, controls }
    }
}

/// Raw command bytes plus the channel-local metadata resolved by extraction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoundTrack {
    pub kind: TrackKind,
    pub channel: Option<u8>,
    /// Cursor into `bytes` where the track's first command lives. Extracted
    /// tracks retain the complete record because relative GOTO/GOSUB targets
    /// may land in shared subroutines before the track pointer.
    pub start_offset: usize,
    pub bytes: Vec<u8>,
    pub initial_voice: Option<u8>,
    pub initial_pan: u8,
    pub initial_transpose: i8,
    pub initial_volume: i8,
    pub psg_envelope: Option<usize>,
    pub tick_multiplier: u8,
    pub dac: bool,
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
            start_offset: 0,
            bytes: bytes.into(),
            initial_voice: None,
            initial_pan: 0xc0,
            initial_transpose: 0,
            initial_volume: 0,
            psg_envelope: None,
            tick_multiplier: 1,
            dac: false,
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

    pub fn volume(mut self, value: i8) -> Self {
        self.initial_volume = value;
        self
    }

    pub fn envelope(mut self, index: usize) -> Self {
        self.psg_envelope = Some(index);
        self
    }

    pub fn dac(mut self) -> Self {
        self.dac = true;
        self
    }

    pub fn start_offset(mut self, offset: usize) -> Self {
        self.start_offset = offset;
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
            if track.start_offset >= track.bytes.len() {
                return Err(SequenceError::InvalidStartOffset {
                    offset: track.start_offset,
                });
            }
            if track.tick_multiplier == 0 {
                return Err(SequenceError::ZeroTickMultiplier);
            }
        }
        Ok(())
    }
}

/// One extracted PCM bank entry. The byte stream is the raw signed-PCM data
/// read from the cartridge DAC bank; the YM bridge consumes it unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DacSample {
    pub id: u8,
    pub bytes: Vec<u8>,
}

/// All resolved sound records and DAC samples needed by the live driver.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SoundBank {
    pub sequences: Vec<SoundSequence>,
    pub dac_samples: Vec<DacSample>,
}

impl SoundBank {
    pub fn new(sequences: Vec<SoundSequence>, dac_samples: Vec<DacSample>) -> Self {
        Self {
            sequences,
            dac_samples,
        }
    }
}

/// Malformed extraction data is observable and diagnosable rather than
/// becoming a silent stream divergence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SequenceError {
    InvalidVoiceLength { actual: usize },
    InvalidChannel { kind: TrackKind, channel: u8 },
    InvalidStartOffset { offset: usize },
    ZeroTickMultiplier,
    UnexpectedEnd { command: u8, cursor: usize },
    InvalidJump { cursor: usize, target: isize },
    CallStackOverflow,
    CallStackUnderflow,
    UnsupportedMeta { value: u8 },
    CommandBudgetExceeded { cursor: usize },
    MissingVoice { index: usize },
    MissingEnvelope { index: usize },
    MissingSequence { id: u8 },
}

impl std::fmt::Display for SequenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "sound sequence error: {self:?}")
    }
}

impl std::error::Error for SequenceError {}
