//! `Event_HuntersGuild` (`$66`, issue #81): the counter's eight jobs.
//!
//! Record: `docs/scenes/124_HuntersGuild.md`. The scene is the routine's legs
//! as ops, once each, as the routine runs them on an indexed row: the two
//! decisions on the whole table ([`dispatch`] on arrival, [`choice`] on a
//! picked row) are pure functions of the flags, [`GuildOp`] carries the
//! indexed reads, and the list window with its cursor is the session's
//! (`psiv-runtime/src/session/job_board.rs`), which the scene drives through
//! [`SceneOp::JobBoard`].

use crate::scene::{DialogueId, DialogueSource, DialogueWindow, GuildOp, JobBoardStep, SceneOp};
use crate::scene_runner::Scene;
use crate::trigger::EventIndex;

/// `DialogueTree26`, the guild map's own tree.
const TREE_26: u32 = 0x001F_1AD0;
/// `DialogueTree27`, which entries `$1F` and up are read from.
const TREE_27: u32 = 0x001F_2400;
/// `cmpi.b #$1F, d0 / bcs`: the entries that stay in tree 26.
pub(crate) const TREE_27_FROM: u8 = 0x1F;
/// `SFXID_Selection`.
const SFX_SELECTION: u8 = 0xF3;
/// `EventFlag_Elsydeon`: past it the first job's in-progress line retires it.
const ELSYDEON: u8 = 0xD9;

/// One row of `HuntersGuildData` (`$071D74`) and its `HuntersGuildMoneyTbl`
/// byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuildJob {
    /// Set when the job is taken.
    pub taken: u8,
    /// Set when the reward is earned.
    pub earned: u8,
    /// Set when the reward is collected.
    pub paid: u8,
    /// The line offering the job (with its yes/no).
    pub offer: u8,
    /// The line while it is in progress.
    pub progress: u8,
    /// The line paying the reward.
    pub reward: u8,
    /// Set once the job is offered.
    pub available: u8,
    /// Set once it no longer is (`$EB`, `EventFlag_GuildPlaceholder`, never).
    pub closed: u8,
    /// The reward, thousands of meseta.
    pub money: u8,
}

/// The table, top to bottom as the board lists it. The rows come from the
/// pack (`guild/guild.json`), never from source.
pub type GuildJobs = [GuildJob; JOBS];
const JOBS: usize = 8;

impl GuildJob {
    /// A `HuntersGuildData` row and its money byte.
    #[must_use]
    pub const fn from_row(row: [u8; 8], money: u8) -> GuildJob {
        GuildJob {
            taken: row[0],
            earned: row[1],
            paid: row[2],
            offer: row[3],
            progress: row[4],
            reward: row[5],
            available: row[6],
            closed: row[7],
            money,
        }
    }
}

/// What the routine's opening loop (`.checkquestloop`) decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildArrival {
    /// A job is taken and its reward not earned: its progress line.
    InProgress(usize),
    /// The first job is taken, unearned, and Elsydeon is found: it retires.
    FirstJobRetired,
    /// A job's reward is earned and not collected.
    Reward(usize),
    /// All eight are collected and were offered or closed.
    NoJobsLeft,
    /// The welcome and the board.
    Board,
}

/// `.checkquestloop`: the first row that is taken decides, unless it is
/// fully paid; a paid row counts when its `available` or `closed` flag is set.
pub fn dispatch(jobs: &GuildJobs, is_set: impl Fn(u8) -> bool) -> GuildArrival {
    let mut completed = 0;
    let mut row = 0;
    while row < JOBS {
        let job = jobs[row];
        if is_set(job.taken) {
            if !is_set(job.earned) {
                return if row == 0 && is_set(ELSYDEON) {
                    GuildArrival::FirstJobRetired
                } else {
                    GuildArrival::InProgress(row)
                };
            }
            if !is_set(job.paid) {
                return GuildArrival::Reward(row);
            }
            if is_set(job.available) || is_set(job.closed) {
                completed += 1;
            }
        }
        row += 1;
    }
    if completed == JOBS {
        GuildArrival::NoJobsLeft
    } else {
        GuildArrival::Board
    }
}

/// What a picked row does (`.selectjobselected`), tested in the routine's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildChoice {
    /// The reward is collected.
    AlreadyDone,
    /// Not offered yet.
    NotYet,
    /// No longer offered.
    Gone,
    /// The offer, with its yes/no.
    Offer,
}

/// `.selectjobselected`.
pub fn choice(jobs: &GuildJobs, row: usize, is_set: impl Fn(u8) -> bool) -> GuildChoice {
    let job = jobs[row];
    if is_set(job.paid) {
        GuildChoice::AlreadyDone
    } else if !is_set(job.available) {
        GuildChoice::NotYet
    } else if is_set(job.closed) {
        GuildChoice::Gone
    } else {
        GuildChoice::Offer
    }
}

/// How the list loop draws a row (`.loadjobtextloop`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobRow {
    /// The line it is drawn in.
    pub colour: JobColour,
    /// `GuildText_ListingPending` replaces the name.
    pub pending: bool,
}

/// A row's colour line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobColour {
    /// `$C000`, the window font's own line.
    Open,
    /// `$A000`: collected.
    Done,
    /// `$E000`: not offered, or no longer.
    Closed,
}

/// `.loadjobtextloop`: a collected job keeps its name in `$A000` and skips the
/// other tests; the two others write `$E000`, and only "not offered" swaps
/// the name.
pub fn job_row(jobs: &GuildJobs, row: usize, is_set: impl Fn(u8) -> bool) -> JobRow {
    let job = jobs[row];
    if is_set(job.paid) {
        return JobRow {
            colour: JobColour::Done,
            pending: false,
        };
    }
    let pending = !is_set(job.available);
    JobRow {
        colour: if pending || is_set(job.closed) {
            JobColour::Closed
        } else {
            JobColour::Open
        },
        pending,
    }
}

/// Which of a row's lines a leg reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GuildLine {
    /// Byte 3.
    Offer,
    /// Byte 4.
    Progress,
    /// Byte 5.
    Reward,
}

impl GuildLine {
    /// The entry on `job`.
    #[must_use]
    pub const fn entry(self, job: GuildJob) -> u8 {
        match self {
            GuildLine::Offer => job.offer,
            GuildLine::Progress => job.progress,
            GuildLine::Reward => job.reward,
        }
    }
}

// Where each leg starts.
const IN_PROGRESS: usize = 1;
const RETIRED: usize = IN_PROGRESS + 5;
const REWARD: usize = RETIRED + 5;
const NO_JOBS: usize = REWARD + 7;
const BOARD: usize = NO_JOBS + 2;
const PROMPT: usize = BOARD + 5;
const DONE_LINE: usize = PROMPT + 4;
const NOT_YET_LINE: usize = DONE_LINE + 3;
const FIRST_GONE_LINE: usize = NOT_YET_LINE + 3;
const GONE_LINE: usize = FIRST_GONE_LINE + 3;
const OFFER: usize = GONE_LINE + 5;
const FAREWELL: usize = OFFER + 8;
const LINGER: usize = FAREWELL + 1;

/// [`GuildOp::Dispatch`]'s target, and the row the leg reads.
pub(crate) fn dispatch_target(jobs: &GuildJobs, is_set: impl Fn(u8) -> bool) -> (usize, usize) {
    match dispatch(jobs, is_set) {
        GuildArrival::InProgress(row) => (IN_PROGRESS, row),
        GuildArrival::FirstJobRetired => (RETIRED, 0),
        GuildArrival::Reward(row) => (REWARD, row),
        GuildArrival::NoJobsLeft => (NO_JOBS, 0),
        GuildArrival::Board => (BOARD, 0),
    }
}

/// [`GuildOp::BranchChoice`]'s target; `None` is Cancel.
pub(crate) fn choice_target(
    jobs: &GuildJobs,
    row: Option<u8>,
    is_set: impl Fn(u8) -> bool,
) -> (usize, usize) {
    let Some(row) = row else {
        return (FAREWELL, 0);
    };
    let row = usize::from(row) % JOBS;
    let target = match choice(jobs, row, is_set) {
        GuildChoice::AlreadyDone => DONE_LINE,
        GuildChoice::NotYet => NOT_YET_LINE,
        // The first job's own line is entry `$19` of tree 26; the routine's
        // `beq` jumps over the tree load.
        GuildChoice::Gone if row == 0 => FIRST_GONE_LINE,
        GuildChoice::Gone => GONE_LINE,
        GuildChoice::Offer => OFFER,
    };
    (target, row)
}

const fn line(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Standard,
    }
}

const fn board(step: JobBoardStep) -> SceneOp {
    SceneOp::JobBoard { step }
}

const HOLD: SceneOp = board(JobBoardStep::Hold);
const BACK_TO_PROMPT: SceneOp = SceneOp::Jump { to: PROMPT };

/// `cmpi.b #$1F, d0 / bcs`: past the tree load when the row's line is below.
const fn tree_test(line: GuildLine) -> SceneOp {
    SceneOp::Guild(GuildOp::SkipTreeLoad { line, skip: 1 })
}

const fn row_line(line: GuildLine) -> SceneOp {
    SceneOp::Guild(GuildOp::RunLine { line })
}

/// The ten `SetDialogueTree`s are the routine's ten `DialogueTreesToRAM` calls.
static OPS: &[SceneOp] = &[
    SceneOp::Guild(GuildOp::Dispatch),
    // `.doinprogress`.
    tree_test(GuildLine::Progress),
    SceneOp::SetDialogueTree { rom_addr: TREE_27 },
    row_line(GuildLine::Progress),
    SceneOp::SetDialogueTree { rom_addr: TREE_26 },
    SceneOp::End,
    // `.handlemiledead`: tree 27's entry `$19`, then both of the job's flags.
    SceneOp::SetDialogueTree { rom_addr: TREE_27 },
    line(0x19),
    SceneOp::SetDialogueTree { rom_addr: TREE_26 },
    SceneOp::Guild(GuildOp::RetireFirst),
    SceneOp::End,
    // `.finishquest`: the line, the flag, `mulu.w #1000` onto the purse.
    tree_test(GuildLine::Reward),
    SceneOp::SetDialogueTree { rom_addr: TREE_27 },
    row_line(GuildLine::Reward),
    SceneOp::SetDialogueTree { rom_addr: TREE_26 },
    SceneOp::Guild(GuildOp::SetPaid),
    SceneOp::Guild(GuildOp::AddReward),
    SceneOp::End,
    // `moveq #$12, d0` / `Event_GetAndRunDialogue`.
    line(0x12),
    SceneOp::End,
    // `.showjobs`: the welcome and its wait, then the list window.
    SceneOp::PlaySound { id: SFX_SELECTION },
    board(JobBoardStep::Begin),
    line(0x0F),
    HOLD,
    board(JobBoardStep::ShowList),
    // `.loadjobcursorboxes`: the boxes, the prompt, the cursor.
    board(JobBoardStep::Boxes),
    line(0x10),
    board(JobBoardStep::Choose),
    SceneOp::Guild(GuildOp::BranchChoice),
    // `.selectjobalreadycompleted`.
    line(0x11),
    HOLD,
    BACK_TO_PROMPT,
    // `.selectjobnotyetavailable`, whose entry `$12` the routine reads two
    // bytes in, past a portrait control naming the portrait already up.
    line(0x12),
    HOLD,
    BACK_TO_PROMPT,
    // `.selectjobnolongeravailable`: the first job's `$19` stays in tree 26.
    line(0x19),
    HOLD,
    BACK_TO_PROMPT,
    SceneOp::SetDialogueTree { rom_addr: TREE_27 },
    line(0x3A),
    SceneOp::SetDialogueTree { rom_addr: TREE_26 },
    HOLD,
    BACK_TO_PROMPT,
    // `.selectjobselected`'s last leg: the offer, `DoMapUpdate_JoypadLoop`,
    // `tst.b Yes_No_Option`.
    tree_test(GuildLine::Offer),
    SceneOp::SetDialogueTree { rom_addr: TREE_27 },
    row_line(GuildLine::Offer),
    SceneOp::SetDialogueTree { rom_addr: TREE_26 },
    HOLD,
    SceneOp::BranchChoice {
        if_yes: OFFER + 6,
        if_no: PROMPT,
    },
    SceneOp::Guild(GuildOp::SetTaken),
    SceneOp::Jump { to: LINGER },
    // The Cancel leg's goodbye, then `.terminateguildwindowloop`: the text
    // stays up until a press, and the windows come down.
    line(0x13),
    board(JobBoardStep::Linger),
    board(JobBoardStep::Close),
    SceneOp::End,
];

/// `$0066`, `Event_HuntersGuild`, retail `$0718E6..$071D73`, its data to
/// `$071E55` (`ps4.asm:150995` on): interaction area `$09` on `HuntersGuild`.
pub static HUNTERS_GUILD: Scene = Scene {
    name: "Event_HuntersGuild",
    event: EventIndex(0x0066),
    ops: OPS,
};
