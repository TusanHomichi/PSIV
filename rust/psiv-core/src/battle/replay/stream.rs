//! The verbatim stream: the cartridge's rolls, in its own order, and the
//! accounting that says the port consumed them and no more.
//!
//! Per round the port is handed a fresh slice of the fixture's rolls - the
//! calls the log's frames hold for that round, in the order the cartridge drew
//! them - and afterwards it must have taken exactly as many as the log holds.
//! No slack and no surplus: a consumer the port models but the cartridge does
//! not reach shows up as a surplus, and a call the log holds with no consumer
//! as slack. [`account_for_every_roll`] is the same claim per action, against
//! the roles the extractor labelled the calls with.

use super::*;

use crate::battle::*;

/// The cartridge's rolls for a round, in the order it drew them.
pub(crate) fn verbatim_round(fixture: &Fixture, round: u16) -> Vec<u16> {
    fixture
        .rolls
        .rolls()
        .iter()
        .filter(|roll| roll.round == round)
        .map(|roll| roll.roll)
        .collect()
}

/// The battle's whole stream, in the order the cartridge drew it - what a
/// replay is fed once its rounds start.
///
/// The priority draw is [`started`]'s, not a round's: `Battle::start` takes it
/// as part of setting the battle up, exactly as the cartridge resolves
/// `loc_B62A` while loading the battle rather than inside a round.
pub(crate) fn verbatim_all(fixture: &Fixture) -> Vec<u16> {
    fixture
        .rolls
        .rolls()
        .iter()
        .filter(|roll| roll.role != "priority")
        .map(|roll| roll.roll)
        .collect()
}

/// Plays one round on a fresh slice of the given stream.
pub(crate) fn play(
    battle: &mut Battle,
    fixture: &Fixture,
    data: &BattleData,
    round: &Round,
    stream: &[u16],
) -> (Vec<BattleEvent>, usize) {
    let mut rolls = SliceRolls::new(stream);
    let timeline = battle
        .round(&orders(fixture, round), data, &mut rolls)
        .expect("the fixture's battle resolves");
    (timeline, rolls.drawn())
}

/// Every roll the log's frames attribute to one action.
///
/// The roles are the ones an action's own frames hold: the hit pass (both
/// passes of it, `AlysKyraAttack_Init`'s included), the ability roll, the
/// ability **re-roll** that follows a draw equal to `$FFFFEEA8`, and the
/// damage runs. The order pass is the round's, not an action's, and comes from
/// the fixture's own `roll_count` for the round.
pub(crate) fn action_rolls(fixture: &Fixture, round: &Round, action: &Action) -> Vec<Roll> {
    fixture
        .rolls
        .rolls()
        .iter()
        .filter(|roll| roll.round == round.round && roll.action == action.actor)
        .filter(|roll| {
            matches!(
                roll.role.as_str(),
                "hit" | "ability" | "ability_reroll" | "damage"
            )
        })
        .cloned()
        .collect()
}

/// One action's rolls as the log's frames hold them, whatever role they carry.
///
/// [`action_rolls`] is the same table filtered to the roles a consumer is
/// expected to walk; this is every call the log puts in the action's frames,
/// which is what the count has to add up to.
pub(crate) fn action_calls(fixture: &Fixture, round: &Round, action: &Action) -> Vec<Roll> {
    fixture
        .rolls
        .rolls()
        .iter()
        .filter(|roll| roll.round == round.round && roll.action == action.actor)
        .cloned()
        .collect()
}

/// Replays every round of `fixture` on the cartridge's own stream.
///
/// Per round it asserts both halves of the claim: the timeline is the log's,
/// action for action, and the port drew exactly the rolls the log's frames
/// hold - no slack and no surplus. Returns the finished battle and each round's
/// timeline; a fixture the port cannot replay exactly fails here with the
/// divergence that parted them (`replay::data`'s data-driven test is the one
/// that reads a manifest entry for it first).
pub(crate) fn replay_verbatim(
    fixture: &Fixture,
    data: &BattleData,
    tape: &str,
) -> (Battle, Vec<Vec<BattleEvent>>) {
    let replay = replay_inner(fixture, data);
    if let Some(finding) = &replay.finding {
        let (log, port) = finding.expectation();
        panic!(
            "{tape}: round {} f{} diverges from the log ({}, {}): the log has {log}, \
             the port {port}; a fixture with a manifest entry is replayed through \
             replay::data",
            finding.round(),
            finding.frame(),
            finding.kind(),
            match finding {
                Finding::Action { divergence, .. } => format!("{divergence:?}"),
                Finding::Draws { .. } => "draw count".to_owned(),
            }
        );
    }
    (replay.battle, replay.timelines)
}

/// [`replay_verbatim`] without the assertion: the replay, and where it parted
/// from the log if it did.
pub(crate) fn replay_inner(fixture: &Fixture, data: &BattleData) -> Replay {
    let mut battle = started(fixture, data);
    let stream = verbatim_all(fixture);
    let mut rolls = SliceRolls::new(&stream);
    let mut timelines = Vec::new();
    let mut draws = Vec::new();
    let mut finding = None;
    for round in &fixture.rounds {
        let before = rolls.drawn();
        let timeline = battle
            .round(&orders(fixture, round), data, &mut rolls)
            .expect("the fixture's battle resolves");
        let drawn = rolls.drawn() - before;
        if finding.is_none() {
            finding = match divergence(round, &timeline) {
                Some(divergence) => Some(Finding::Action {
                    round: round.round,
                    divergence,
                }),
                None if drawn != round.roll_count as usize => Some(Finding::Draws {
                    round: round.round,
                    frame: round.order_frame,
                    log: round.roll_count as usize,
                    port: drawn,
                }),
                None => None,
            };
        }
        draws.push((round.round, round.roll_count as usize, drawn));
        timelines.push(timeline);
    }
    Replay {
        battle,
        timelines,
        finding,
        draws,
    }
}

/// What a verbatim replay produced: the battle, each round's timeline, and the
/// first divergence the fixture's manifest entry has to explain.
pub(crate) struct Replay {
    pub(crate) battle: Battle,
    pub(crate) timelines: Vec<Vec<BattleEvent>>,
    pub(crate) finding: Option<Finding>,
    /// Per round: the roll count the log's frames hold, and the count the port
    /// drew on this run's stream (`(round, log, port)`).
    pub(crate) draws: Vec<(u16, usize, usize)>,
}

/// The first place a fixture and the port part company.
#[derive(Debug)]
pub(crate) enum Finding {
    /// An action resolved differently. Its `frame` is the action's first frame.
    Action { round: u16, divergence: Divergence },
    /// Every action matched, but the round drew a different number of rolls.
    Draws {
        round: u16,
        frame: u32,
        log: usize,
        port: usize,
    },
}

impl Finding {
    /// The frame the finding sits at, as the manifest names it.
    pub(crate) fn frame(&self) -> u32 {
        match self {
            Finding::Action { divergence, .. } => divergence.frame(),
            Finding::Draws { frame, .. } => *frame,
        }
    }

    /// The finding's kind, as `replay_fixtures/divergences.json` names it.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Finding::Action { divergence, .. } => divergence.kind(),
            Finding::Draws { .. } => "draws",
        }
    }

    pub(crate) fn round(&self) -> u16 {
        match self {
            Finding::Action { round, .. } | Finding::Draws { round, .. } => *round,
        }
    }

    /// What the log shows there, and what the port did instead.
    pub(crate) fn expectation(&self) -> (String, String) {
        match self {
            Finding::Action { divergence, .. } => divergence.expectation(),
            Finding::Draws { log, port, .. } => (
                format!("{log} roll(s) in the round's frames"),
                format!("{port} roll(s) drawn"),
            ),
        }
    }
}

/// Every call the log's frames hold is one the port accounts for.
///
/// The count is per action, against the roles that action's frames carry: the
/// round's own order pass is the difference between the round's `roll_count`
/// and the sum over its actions. A roll with no consumer - the divergence this
/// test exists to catch - shows up as an action whose log count is larger than
/// the port's, and a consumer with no roll in the log shows up in the other
/// direction.
pub(crate) fn account_for_every_roll(fixture: &Fixture, tape: &str) {
    let rolls = fixture.rolls.rolls();
    for round in &fixture.rounds {
        let order = rolls
            .iter()
            .filter(|roll| roll.round == round.round && roll.role == "order")
            .count();
        let mut actions = 0;
        for action in &round.actions {
            let port = action_rolls(fixture, round, action);
            assert_eq!(
                action.roll_count as usize,
                action_calls(fixture, round, action).len(),
                "{tape}: actor {} at f{}: the fixture's own count",
                action.actor,
                action.start_frame
            );
            assert_eq!(
                action.roll_count as usize,
                port.len(),
                "{tape}: actor {} at f{} draws the log's {} calls",
                action.actor,
                action.start_frame,
                action.roll_count
            );
            for roll in &port {
                if roll.role == "damage" {
                    let target = roll.target.expect("a damage run has a target");
                    let resolved: Vec<u8> = action
                        .targets
                        .iter()
                        .filter(|target| target.hit != "FF")
                        .map(|target| target.id)
                        .collect();
                    assert!(
                        resolved.contains(&target),
                        "{tape}: actor {} at f{}: a damage run is labelled {} but the \
                         log resolves {resolved:?}",
                        action.actor,
                        roll.frame,
                        target
                    );
                }
            }
            actions += port.len();
        }
        assert_eq!(
            round.roll_count as usize,
            actions + order,
            "{tape}: round {}'s frames hold the order pass and its actions, nothing else",
            round.round
        );
    }
}
