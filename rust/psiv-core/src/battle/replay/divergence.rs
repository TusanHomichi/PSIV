//! One finding: the first place a round's timeline disagrees with the log.
//!
//! These are the shapes `every_fixture_replays_as_recorded` reads, and the
//! names `replay_fixtures/divergences.json` carries; `docs/oracle/BATTLE_ORACLE_SWEEP.md`
//! §4 is where each one is triaged, as a port rule or as the harness's own
//! misreading. [`super::compare::divergence`] is what produces them.

use crate::battle::*;

/// The first place a round's timeline disagrees with the log.
#[derive(Debug, PartialEq)]
pub(crate) enum Divergence {
    /// The queue build.
    Queue {
        round: u16,
        port: Vec<FighterId>,
        log: Vec<FighterId>,
    },
    /// A fighter the log shows resolving targets never swung.
    NoSwing {
        frame: u32,
        actor: FighterId,
        log_targets: usize,
    },
    /// A swing's target list.
    Targets {
        frame: u32,
        actor: FighterId,
        port: Vec<FighterId>,
        log: Vec<FighterId>,
    },
    /// A target resolved for the log was never resolved by the port.
    NoResolution {
        frame: u32,
        actor: FighterId,
        target: FighterId,
    },
    /// A verdict or a damage value.
    Value {
        frame: u32,
        actor: FighterId,
        target: FighterId,
        port: Verdict,
        port_damage: Option<u16>,
        log_hit: u8,
        log_damage: Option<u16>,
    },
    /// HP left on the target.
    Hp {
        frame: u32,
        actor: FighterId,
        target: FighterId,
        port: u16,
        log: i32,
    },
    /// A death the port did not report.
    NoDeath { frame: u32, target: FighterId },
    /// The log's action ran an ability; the port ran something else.
    Ability {
        frame: u32,
        actor: FighterId,
        log_ability: u8,
        port: Option<u8>,
    },
    /// The log's action ran an ability the port does not implement, so it fell
    /// back to a physical swing.
    Unsupported {
        frame: u32,
        actor: FighterId,
        ability: u8,
    },
    /// The log's action spent the turn without an effect; the port did not.
    NotWasted {
        frame: u32,
        actor: FighterId,
        log_ability: u8,
    },
    /// The status bits the log's action gained on a fighter are not the bits
    /// the port's turn inflicted on it.
    Status {
        frame: u32,
        actor: FighterId,
        target: FighterId,
        log_status: u32,
        port_status: u32,
    },
    /// The fighters the log's action took from positive HP to zero are not the
    /// ones the port's turn reports as dead.
    Deaths {
        frame: u32,
        actor: FighterId,
        log: Vec<FighterId>,
        port: Vec<FighterId>,
    },
    /// The log's action moved a fighter's battle agility to a value the port's
    /// turn did not set.
    Stat {
        frame: u32,
        actor: FighterId,
        target: FighterId,
        field: String,
        log_value: i32,
        port_value: Option<u16>,
    },
}

impl Divergence {
    pub(crate) fn frame(&self) -> u32 {
        match self {
            Divergence::Queue { .. } => 0,
            Divergence::NoSwing { frame, .. }
            | Divergence::Targets { frame, .. }
            | Divergence::NoResolution { frame, .. }
            | Divergence::Value { frame, .. }
            | Divergence::Hp { frame, .. }
            | Divergence::NoDeath { frame, .. }
            | Divergence::Ability { frame, .. }
            | Divergence::Unsupported { frame, .. }
            | Divergence::Status { frame, .. }
            | Divergence::Stat { frame, .. }
            | Divergence::Deaths { frame, .. }
            | Divergence::NotWasted { frame, .. } => *frame,
        }
    }

    /// The name `replay_fixtures/divergences.json` carries for this finding.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Divergence::Queue { .. } => "queue",
            Divergence::NoSwing { .. } => "no-swing",
            Divergence::Targets { .. } => "targets",
            Divergence::NoResolution { .. } => "no-resolution",
            Divergence::Value { .. } => "value",
            Divergence::Hp { .. } => "hp",
            Divergence::NoDeath { .. } => "no-death",
            Divergence::Ability { .. } => "ability",
            Divergence::Unsupported { .. } => "unsupported",
            Divergence::NotWasted { .. } => "not-wasted",
            Divergence::Status { .. } => "status",
            Divergence::Stat { .. } => "stat",
            Divergence::Deaths { .. } => "deaths",
        }
    }

    /// What the log says, and what the port did: the pair a reader needs to
    /// judge the finding without re-running anything.
    pub(crate) fn expectation(&self) -> (String, String) {
        match self {
            Divergence::Queue { round, port, log } => (
                format!("round {round}'s queue {log:?}"),
                format!("queue {port:?}"),
            ),
            Divergence::NoSwing {
                actor, log_targets, ..
            } => (
                format!("actor {actor:?} resolving {log_targets} target(s)"),
                "no swing".to_owned(),
            ),
            Divergence::Targets { port, log, .. } => {
                (format!("targets {log:?}"), format!("targets {port:?}"))
            }
            Divergence::NoResolution { actor, target, .. } => (
                format!("{actor:?} resolving {target:?}"),
                "no resolution".to_owned(),
            ),
            Divergence::Value {
                target,
                port,
                port_damage,
                log_hit,
                log_damage,
                ..
            } => (
                format!("{target:?}: hit flag {log_hit:02X}, damage {log_damage:?}"),
                format!("{port:?} with {port_damage:?}"),
            ),
            Divergence::Hp {
                target, port, log, ..
            } => (format!("{target:?} at {log} HP"), format!("{port} HP")),
            Divergence::NoDeath { target, .. } => {
                (format!("{target:?} dying"), "no Died event".to_owned())
            }
            Divergence::Ability {
                log_ability, port, ..
            } => (
                format!("ability ${log_ability:02X}"),
                match port {
                    Some(ability) => format!("ability ${ability:02X}"),
                    None => "a physical swing".to_owned(),
                },
            ),
            Divergence::Unsupported { ability, .. } => (
                format!("ability ${ability:02X} resolving"),
                format!("UnsupportedAbility ${ability:02X} and a physical swing"),
            ),
            Divergence::NotWasted { log_ability, .. } => (
                format!("ability ${log_ability:02X} spending the turn"),
                "an effect".to_owned(),
            ),
            Divergence::Status {
                target,
                log_status,
                port_status,
                ..
            } => (
                format!("{target:?} gaining status {log_status:02X}"),
                format!("status {port_status:02X} inflicted"),
            ),
            Divergence::Deaths { log, port, .. } => (
                format!("{log:?} taken to zero HP"),
                format!("{port:?} reported dead"),
            ),
            Divergence::Stat {
                target,
                field,
                log_value,
                port_value,
                ..
            } => (
                format!("{target:?} {field} set to {log_value}"),
                port_value
                    .map_or_else(|| "no change".to_owned(), |value| format!("set to {value}")),
            ),
        }
    }
}
