use psiv_core::battle::{FighterId, Outcome};

pub(super) struct ActiveEvent {
    pub(super) remaining: u16,
    pub(super) wait_for_confirm: bool,
}

/// A request for the field to call the runtime epilogue after playback.
#[derive(Clone, Copy)]
pub(crate) struct FinishRequest {
    pub(crate) outcome: Outcome,
    pub(crate) reward_each: u16,
}

#[derive(Clone, Copy)]
pub(super) struct DamageDraw {
    pub(super) target: FighterId,
    pub(super) amount: u16,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MessageKind {
    None,
    Transient,
    Wide,
    Victory,
    VictoryRewards,
}

#[derive(Clone)]
pub(super) struct PartyStatus {
    pub(super) fighter: u8,
    pub(super) name: String,
    pub(super) hp: u16,
    pub(super) tp: u16,
}
