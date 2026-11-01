//! `Event_HuntersGuild` (`$66`): the routine's decisions on the quest table,
//! and the ops each of its legs runs. Record: `docs/scenes/124_HuntersGuild.md`.

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, Direction, EventIndex, Flag, GUILD_JOBS, GameState, GuildArrival, GuildChoice,
    JobBoardStep, JobColour, JobRow, SCENES, SceneEffect, SceneInput, SceneOp, SceneRunner,
    ScriptedActor, StepFrames, guild_choice, guild_dispatch, job_row, scene_for,
};

const ELSYDEON: u8 = 0xD9;

fn set(flags: &[u8]) -> impl Fn(u8) -> bool + '_ {
    move |flag| flags.contains(&flag)
}

#[test]
fn nothing_taken_shows_the_board() {
    assert_eq!(guild_dispatch(set(&[])), GuildArrival::Board);
}

#[test]
fn a_taken_job_says_its_progress_until_earned_then_pays() {
    for (row, job) in GUILD_JOBS.iter().enumerate() {
        assert_eq!(
            guild_dispatch(set(&[job.taken])),
            GuildArrival::InProgress(row)
        );
        assert_eq!(
            guild_dispatch(set(&[job.taken, job.earned])),
            GuildArrival::Reward(row)
        );
        assert_eq!(
            guild_dispatch(set(&[job.taken, job.earned, job.paid])),
            GuildArrival::Board,
            "a paid job that was never offered or closed is not counted, and the board shows"
        );
    }
}

#[test]
fn the_first_job_alone_retires_past_elsydeon() {
    assert_eq!(
        guild_dispatch(set(&[GUILD_JOBS[0].taken, ELSYDEON])),
        GuildArrival::FirstJobRetired
    );
    assert_eq!(
        guild_dispatch(set(&[GUILD_JOBS[1].taken, ELSYDEON])),
        GuildArrival::InProgress(1)
    );
}

#[test]
fn the_earliest_open_row_decides() {
    let (a, b) = (GUILD_JOBS[2], GUILD_JOBS[5]);
    assert_eq!(
        guild_dispatch(set(&[a.taken, b.taken, b.earned])),
        GuildArrival::InProgress(2)
    );
}

#[test]
fn eight_paid_and_offered_jobs_leave_none_and_seven_do_not() {
    let all: Vec<u8> = GUILD_JOBS
        .iter()
        .flat_map(|job| [job.taken, job.earned, job.paid, job.available])
        .collect();
    assert_eq!(guild_dispatch(set(&all)), GuildArrival::NoJobsLeft);
    let last = GUILD_JOBS[7];
    let short: Vec<u8> = all
        .iter()
        .copied()
        .filter(|&flag| flag != last.taken)
        .collect();
    assert_eq!(guild_dispatch(set(&short)), GuildArrival::Board);
}

#[test]
fn a_picked_row_tests_paid_then_offered_then_closed() {
    let job = GUILD_JOBS[7];
    let row = 7;
    assert_eq!(guild_choice(row, set(&[])), GuildChoice::NotYet);
    assert_eq!(guild_choice(row, set(&[job.available])), GuildChoice::Offer);
    assert_eq!(
        guild_choice(row, set(&[job.available, job.closed])),
        GuildChoice::Gone
    );
    assert_eq!(
        guild_choice(row, set(&[job.paid])),
        GuildChoice::AlreadyDone
    );
    assert_eq!(guild_choice(row, set(&[job.closed])), GuildChoice::NotYet);
}

#[test]
fn rows_draw_in_the_lines_the_list_loop_writes() {
    let job = GUILD_JOBS[0];
    let row = |colour, pending| JobRow { colour, pending };
    assert_eq!(job_row(0, set(&[])), row(JobColour::Closed, true));
    assert_eq!(
        job_row(0, set(&[job.available])),
        row(JobColour::Open, false)
    );
    assert_eq!(
        job_row(0, set(&[job.available, job.closed])),
        row(JobColour::Closed, false)
    );
    assert_eq!(job_row(0, set(&[job.paid])), row(JobColour::Done, false));
}

/// Runs the scene, closing each dialogue and answering each board wait from
/// `picks`, and returns the entries spoken and the board steps seen.
fn run(flags: &[u8], picks: &[Option<u8>], yes: bool) -> (Vec<u16>, Vec<JobBoardStep>, GameState) {
    let (lines, steps, state, _) = play(flags, picks, yes);
    (lines.iter().map(|&(line, _)| line).collect(), steps, state)
}

/// Each line with the tree it was read from, and the tree at the end.
fn run_trees(flags: &[u8], picks: &[Option<u8>]) -> (Vec<(u16, u32)>, u32) {
    let (lines, _, _, tree) = play(flags, picks, true);
    (lines, tree)
}

fn play(
    flags: &[u8],
    picks: &[Option<u8>],
    yes: bool,
) -> (Vec<(u16, u32)>, Vec<JobBoardStep>, GameState, u32) {
    let mut state = GameState::new();
    for flag in flags {
        state.set(Flag::event(u16::from(*flag))).unwrap();
    }
    let map = map_with(&["...."], vec![], vec![]);
    let scene = scene_for(EventIndex(0x66)).expect("registered");
    let cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(1, 0),
        Direction::Up,
    )];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut picks = picks.iter();
    let mut tree = 0x001F_1AD0;
    let mut lines = Vec::new();
    let mut steps = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..2000 {
        if runner.is_finished() {
            return (lines, steps, state, tree);
        }
        let effects = runner.tick(&map, &mut state, input);
        input = SceneInput::None;
        for effect in effects {
            match effect {
                SceneEffect::DialogueOpen(id) => {
                    lines.push((id.0, tree));
                    input = SceneInput::DialogueEnded;
                }
                SceneEffect::Presentation {
                    op: SceneOp::SetDialogueTree { rom_addr },
                } => tree = rom_addr,
                SceneEffect::ChoiceRequested => input = SceneInput::Choice(yes),
                SceneEffect::Presentation {
                    op: SceneOp::JobBoard { step },
                } => {
                    steps.push(step);
                    input = match step {
                        JobBoardStep::Choose => {
                            SceneInput::JobBoardPicked(*picks.next().expect("a pick is left"))
                        }
                        JobBoardStep::Begin | JobBoardStep::Boxes | JobBoardStep::Close => input,
                        _ => SceneInput::JobBoardDone,
                    };
                }
                _ => {}
            }
        }
    }
    panic!("the scene did not finish: lines {lines:x?}, steps {steps:?}");
}

#[test]
fn the_progress_and_reward_legs_speak_and_pay() {
    let job = GUILD_JOBS[4];
    let (lines, steps, state) = run(&[job.taken], &[], true);
    assert_eq!(lines, [u16::from(job.progress)]);
    assert!(steps.is_empty());
    assert!(!state.is_set(Flag::event(u16::from(job.paid))));

    let (lines, _, state) = run(&[job.taken, job.earned], &[], true);
    assert_eq!(lines, [u16::from(job.reward)]);
    assert!(state.is_set(Flag::event(u16::from(job.paid))));
    assert_eq!(state.money(), 50_000);
}

#[test]
fn the_first_jobs_retirement_clears_both_its_flags() {
    let job = GUILD_JOBS[0];
    let (lines, _, state) = run(&[job.taken, ELSYDEON], &[], true);
    assert_eq!(lines, [0x19]);
    assert!(!state.is_set(Flag::event(u16::from(job.taken))));
    assert!(!state.is_set(Flag::event(u16::from(job.earned))));
    assert_eq!(state.money(), 0);
}

#[test]
fn no_jobs_left_is_one_line() {
    let all: Vec<u8> = GUILD_JOBS
        .iter()
        .flat_map(|job| [job.taken, job.earned, job.paid, job.available])
        .collect();
    let (lines, steps, _) = run(&all, &[], true);
    assert_eq!(lines, [0x12]);
    assert!(steps.is_empty());
}

#[test]
fn cancel_says_goodbye_and_takes_nothing() {
    let (lines, steps, _) = run(&[], &[None], true);
    assert_eq!(lines, [0x0F, 0x10, 0x13]);
    assert_eq!(
        steps,
        [
            JobBoardStep::Begin,
            JobBoardStep::Hold,
            JobBoardStep::ShowList,
            JobBoardStep::Boxes,
            JobBoardStep::Choose,
            JobBoardStep::Linger,
            JobBoardStep::Close,
        ]
    );
}

#[test]
fn yes_takes_the_job_and_no_returns_to_the_prompt() {
    let job = GUILD_JOBS[1];
    let (lines, steps, state) = run(&[job.available], &[Some(1)], true);
    assert_eq!(lines, [0x0F, 0x10, u16::from(job.offer)]);
    assert!(state.is_set(Flag::event(u16::from(job.taken))));
    assert!(steps.ends_with(&[
        JobBoardStep::Hold,
        JobBoardStep::Linger,
        JobBoardStep::Close
    ]));

    let (lines, _, state) = run(&[job.available], &[Some(1), None], false);
    assert_eq!(lines, [0x0F, 0x10, u16::from(job.offer), 0x10, 0x13]);
    assert!(!state.is_set(Flag::event(u16::from(job.taken))));
}

#[test]
fn refused_rows_speak_and_return_to_the_prompt() {
    let flags = [
        GUILD_JOBS[1].paid,
        GUILD_JOBS[0].available,
        GUILD_JOBS[0].closed,
        GUILD_JOBS[7].available,
        GUILD_JOBS[7].closed,
    ];
    // Row 1 paid; row 2 not offered; row 0's own line; row 7's.
    let (lines, _, _) = run(&flags, &[Some(1), Some(2), Some(0), Some(7), None], true);
    assert_eq!(
        lines,
        [
            0x0F, 0x10, 0x11, 0x10, 0x12, 0x10, 0x19, 0x10, 0x3A, 0x10, 0x13
        ]
    );
}

/// Every row's three lines, each read from the tree the routine's `cmpi.b
/// #$1F` picks, with tree 26 back afterwards.
#[test]
fn every_rows_lines_read_the_tree_their_entry_picks() {
    const TREE_26: u32 = 0x001F_1AD0;
    const TREE_27: u32 = 0x001F_2400;
    for (row, job) in GUILD_JOBS.iter().enumerate() {
        let picked = [Some(row as u8)];
        let legs = [
            (vec![job.taken], &[][..], job.progress),
            (vec![job.taken, job.earned], &[][..], job.reward),
            (vec![job.available], &picked[..], job.offer),
        ];
        // Row 0 is retired past Elsydeon, which its `closed` names; row 7's
        // offer has the same flag clear here.
        for (flags, picks, entry) in legs {
            let wanted = if entry < 0x1F { TREE_26 } else { TREE_27 };
            let (lines, trees) = run_trees(&flags, picks);
            let at = lines
                .iter()
                .position(|&(line, _)| line == u16::from(entry))
                .unwrap_or_else(|| panic!("row {row} never spoke {entry:#x}: {lines:x?}"));
            assert_eq!(lines[at].1, wanted, "row {row} entry {entry:#x}");
            assert_eq!(trees, TREE_26, "row {row}: tree 26 is back at the end");
        }
    }
}

#[test]
fn the_scene_holds_the_routines_ten_tree_loads() {
    let scene = scene_for(EventIndex(0x66)).unwrap();
    let loads = scene
        .ops
        .iter()
        .filter(|op| matches!(op, SceneOp::SetDialogueTree { .. }))
        .count();
    assert_eq!(loads, 10);
    assert!(
        SCENES
            .iter()
            .any(|scene| scene.name == "Event_HuntersGuild")
    );
}
