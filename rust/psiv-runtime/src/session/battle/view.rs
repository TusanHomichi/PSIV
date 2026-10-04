//! What the battle shows: the read-only view the shell draws from.
//!
//! The battle mode owns every decision — which command window is open, which
//! actor answers, when a beat starts and how long it lasts, what the narration
//! window says, when the battle ends and what it pays. What crosses to the
//! shell is this struct: the drawing surface, the retail sound cues the frame
//! raised, and the two edges the presentation needs (the battle that began,
//! and the battle that is over).
//!
//! Every field is absolutely valued rather than a delta, so a shell that
//! renders the view every frame cannot drift: the menu is what is open now, the
//! party strip is the live roster, the enemy list is the roster with the
//! visibility the played beats decided.
//!
//! The per-menu lists (`rows`, `techniques`, `skills`, `targets`, `party`,
//! `enemies`) are the same data the native input drivers read out of
//! `Field::debug_play_state`, which is why they are ordinary view fields and
//! not a debug-only side channel.

use psiv_core::battle::{FighterId, ItemSource, Outcome};

use crate::events::BattleAnimationEvent;

/// One party pane of the status strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyStatus {
    /// Retail fighter slot, 1-5.
    pub fighter: u8,
    /// Display name.
    pub name: String,
    /// Current HP.
    pub hp: u16,
    /// Maximum HP.
    pub max_hp: u16,
    /// Current TP.
    pub tp: u16,
    /// Retail status word.
    pub status: u8,
}

/// What one pane of the status strip draws beside the name, and the CRAM line
/// its text is drawn on (`Battle_DrawCommandIcons`, `ps4.asm:11056`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneView {
    /// Retail fighter slot, 1-5, of the pane's seat.
    pub fighter: u8,
    /// The icon index the pane draws: 0 no command yet, 1-5 attack, tech,
    /// skill, item, defend, 6 paralysis, 7 sleep, 8 dead, 9 an empty seat.
    pub icon: u8,
    /// CRAM line of the pane's text: 3 normal, 0 poisoned, 1 asleep or
    /// paralyzed, 2 dead.
    pub ink_line: u8,
}

/// One enemy of the current battle, in fighter order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnemyStatus {
    /// Retail fighter slot, 5-9.
    pub fighter: u8,
    /// Display name, the name the narration uses too.
    pub name: String,
    /// Whether the story still shows this enemy's sprite.
    pub visible: bool,
    /// The enemy record this slot holds now. It changes when Fusion seats a
    /// MetaSlug in slot 1, which is the shell's cue to draw a different body.
    pub enemy_id: u16,
    /// The formation position byte of an enemy seated mid-battle (Fusion's
    /// `$14`); `None` for the formation's own enemies, whose positions the
    /// shell reads from the formation record.
    pub position: Option<u8>,
}

/// The beat the battle is playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleBeat {
    /// A beat with nothing of its own to show: the dwell is the only visible
    /// part, and the narration decides between a transient and a wide window.
    None,
    /// The battle's opening beat.
    Start,
    /// An actor swings; the party actor takes the attack pose.
    Attack(FighterId),
    /// An actor takes the physical-resistance pose.
    Defense(FighterId),
    /// A resolution's damage block.
    Damage {
        /// Who was hit.
        target: FighterId,
        /// The damage shown, `None` for a miss.
        amount: Option<u16>,
        /// Whether the retail damage block is the critical one.
        critical: bool,
    },
    /// A fighter leaves the field.
    Hide(FighterId),
    /// The victory banner's reward page.
    Reward,
    /// A level-up or learning page.
    LevelUp,
    /// The battle's last page.
    End(Outcome),
}

/// The current beat and how far it has run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeatView {
    /// What is playing.
    pub beat: BattleBeat,
    /// Dwell frames left on this beat.
    pub remaining: u16,
    /// The dwell the beat started with, so a caller can read its progress.
    pub total: u16,
    /// Whether the beat waits for a confirm press instead of timing out.
    pub waits_for_confirm: bool,
}

/// The damage block to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageView {
    /// Who was hit.
    pub target: FighterId,
    /// The amount drawn.
    pub amount: u16,
}

/// Which narration window the message belongs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    /// Nothing is drawn.
    None,
    /// The narrow transient line under the stage.
    Transient,
    /// The wide transient window.
    Wide,
    /// The `Victory!` banner.
    Victory,
    /// The banner with its experience and meseta page.
    VictoryRewards,
}

/// The per-character command window's page, spelled the way the native input
/// drivers read it (`menu.page`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPage {
    /// ATTACK / TECH / SKILL / ITEM / DEFEND.
    Actions,
    /// The techniques the actor knows.
    Techniques,
    /// The character skills the actor knows.
    Skills,
    /// The equipment and inventory items the actor can use.
    Items,
    /// The target list for the action being chosen.
    Targets(TargetKind),
}

/// Which action a target list belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    /// ATTACK.
    Attack,
    /// A technique.
    Technique(u8),
    /// A character skill.
    Skill(u8),
    /// An item, from the copy it was selected in.
    Item {
        /// Cartridge item id.
        item: u8,
        /// The copy the order reserved.
        source: ItemSource,
    },
}

/// One row of the command window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRow {
    /// The label the window draws.
    pub label: String,
    /// Whether the row can be accepted.
    pub enabled: bool,
}

/// One technique of the actor's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechniqueEntry {
    /// One-based cartridge technique id.
    pub id: u8,
    /// Display name.
    pub name: String,
    /// TP the cast costs.
    pub cost: u8,
    /// Whether a cast would take effect: supported, affordable and not sealed.
    /// The window itself only refuses on TP (`ps4.asm:1721`, `2628`); a sealed member
    /// may choose the entry and the cast is paid and wasted
    /// (`CharTech_Cast`, `ps4.asm:14256`), so a policy reads this flag to avoid
    /// a wasted turn.
    pub available: bool,
}

/// One character skill of the actor's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntry {
    /// One-based cartridge skill id.
    pub id: u8,
    /// Display name.
    pub name: String,
    /// Uses left in the actor's learned slot.
    pub remaining: u8,
    /// Whether the actor can use it now.
    pub available: bool,
}

/// The per-character command strip: the cartridge's horizontal five-icon
/// window (`Battle_OpenCharComd`, `ps4.asm:2085`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StripView {
    /// `Battle_Total_Comd_Input`: the acting fighter's slot minus one. It
    /// picks the window's column (`loc_17A4`, `ps4.asm:2410`).
    pub slot: u8,
    /// Which of ATTACK, TECH, SKILL, ITEM, DEFEND have an icon. The routine
    /// leaves the cell blank when the actor has nothing to put there
    /// (`ps4.asm:2108-2160`).
    pub present: [bool; 5],
    /// The selected icon, 0-4 (`Battle_Char_Comd_Index`).
    pub cursor: u8,
    /// Frames the window has been open: the command cursor object's clock
    /// (`BattleObj_ComdCursor`, `ps4.asm:70465`), whose two mapping frames
    /// alternate while it runs.
    pub age: u32,
}

/// One entry of a list window page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListEntry {
    /// The name, as the cartridge prints it.
    pub name: String,
    /// The number beside the name: a technique's TP cost or a skill's uses.
    pub value: Option<u8>,
    /// `false` draws the entry in the disabled palette and ignores accept.
    pub enabled: bool,
}

/// One page of a technique, skill or item window: four rows, the selected
/// row, and whether neighbouring pages exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListView {
    /// Which window: [`MenuPage::Techniques`] (`Battle_TechWindow`, 12
    /// columns wide), [`MenuPage::Skills`] (`Battle_SkillWindow`, 16) or
    /// [`MenuPage::Items`] (`Battle_ItemWindow`, 14).
    pub window: MenuPage,
    /// `Battle_Total_Comd_Input`, for the window's column.
    pub slot: u8,
    /// The page, from zero.
    pub page: usize,
    /// The page's entries; fewer than four leaves the rest blank.
    pub entries: Vec<ListEntry>,
    /// The selected row on this page, 0-3.
    pub cursor: u8,
    /// A previous page exists (`Left` flips to it).
    pub more_before: bool,
    /// A next page exists (`Right` flips to it).
    pub more_after: bool,
}

/// The per-character command window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMenuView {
    /// The strip, on the actions page.
    pub strip: Option<StripView>,
    /// The list window, on the technique, skill and item pages.
    pub list: Option<ListView>,
    /// The window title.
    pub title: String,
    /// The page the window is on.
    pub page: MenuPage,
    /// The rows to draw.
    pub rows: Vec<MenuRow>,
    /// The selected row.
    pub cursor: usize,
    /// The actor answering right now.
    pub actor: Option<u8>,
    /// The character id of that actor.
    pub character: Option<u8>,
    /// The live party, for the target lists and the debug probe.
    pub party: Vec<PartyStatus>,
    /// The living enemies, for the target lists and the debug probe.
    pub enemies: Vec<u8>,
    /// The actor's techniques, newest learned first.
    pub techniques: Vec<TechniqueEntry>,
    /// The actor's skills, newest learned first.
    pub skills: Vec<SkillEntry>,
    /// The target list of the current page.
    pub targets: Vec<u8>,
}

/// One vehicle skill slot of the mounted window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillSlotView {
    /// One-based `VehicleAttackNames` id.
    pub id: u8,
    /// Uses left, read from the live battle record.
    pub current: u8,
    /// Uses the record carries.
    pub max: u8,
}

/// What the command surface shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuView {
    /// The COMD / MACR / RUN window, before a character answers.
    Top {
        /// The selected row.
        cursor: usize,
    },
    /// The command window of the actor whose turn it is.
    Commands(Box<CommandMenuView>),
    /// The mounted surface's own skill window.
    VehicleSkills {
        /// The slots, in learned-slot order.
        slots: Vec<SkillSlotView>,
        /// The selected slot.
        cursor: usize,
    },
}

/// The battle the shell must build its art setup for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleStart {
    /// A random encounter: the pack's formation table by id.
    Encounter(u16),
    /// A scene-owned battle: the pack's boss table by event battle index.
    EventBattle(u16),
}

/// One frame of battle, ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct BattleView {
    /// The command surface, or `None` while a beat owns the frame.
    pub menu: Option<MenuView>,
    /// Whether the command window is open and accepting input.
    pub ready: bool,
    /// Whether the battle has reached its ending page.
    pub finishing: bool,
    /// The COMD / MACR / RUN cursor, the native drivers' `battle.cursor`.
    pub cursor: usize,
    /// The narration line.
    pub message: String,
    /// The narration window the line belongs in.
    pub message_kind: MessageKind,
    /// Experience the victory page shows, per member.
    pub reward_each: u16,
    /// Meseta the victory page shows.
    pub reward_meseta: u16,
    /// The damage block to draw, if this beat has one.
    pub damage: Option<DamageView>,
    /// The live party strip, in fighter order.
    pub party: Vec<PartyStatus>,
    /// What each of the five panes draws, by fighter slot.
    pub panes: Vec<PaneView>,
    /// The party bodies the plane has drawn, by fighter slot; `None` draws
    /// all of them. The cartridge clears the party rows when a command window
    /// opens (`Battle_OpenCharComd`, `ps4.asm:2099`) and draws a body back
    /// only as its owner acts, until the round ends and the options reopen.
    pub shown: Option<Vec<u8>>,
    /// Whether the red-cursor windows (the main options and the technique,
    /// skill and item lists) draw their selected row red this frame; the
    /// cartridge alternates red and blue (`Battle_UpdateRedCursor2`,
    /// `ps4.asm:1572`).
    pub cursor_red: bool,
    /// The battle's enemies, in fighter order, with their visibility.
    pub enemies: Vec<EnemyStatus>,
    /// Party fighters taking the attack pose this beat.
    pub poses: Vec<FighterId>,
    /// The beat playing and its progress.
    pub current: Option<BeatView>,
    /// The fighter whose transient line is drawn, when it is not the default
    /// column. The shell owns the column table.
    pub transient: Option<FighterId>,
    /// Retail `Sound_Index` writes this frame raised, in the order they
    /// happened.
    pub sounds: Vec<u8>,
    /// Enemy attack animation cues this frame started.
    pub animations: Vec<BattleAnimationEvent>,
    /// Whether the presentation is over and the field takes the frame back.
    pub close_ready: bool,
    /// A round the engine refused, for the shell to log. The mode stays open
    /// with the error page shown, exactly as the shell's own error path did.
    pub fault: Option<String>,
}

impl Default for BattleView {
    fn default() -> BattleView {
        BattleView {
            menu: None,
            ready: false,
            finishing: false,
            cursor: 0,
            message: String::new(),
            message_kind: MessageKind::None,
            reward_each: 0,
            reward_meseta: 0,
            damage: None,
            party: Vec::new(),
            panes: Vec::new(),
            shown: None,
            cursor_red: true,
            enemies: Vec::new(),
            poses: Vec::new(),
            current: None,
            transient: None,
            sounds: Vec::new(),
            animations: Vec::new(),
            close_ready: false,
            fault: None,
        }
    }
}

impl BattleView {
    /// The tape-07 command-idle fixture: a command window with no battle
    /// behind it.
    ///
    /// `PSIV_DEBUG_BATTLE=0x88` is the command-idle oracle capture, and its
    /// party and enemy placements are receipt-backed rather than the runtime's
    /// (tape 07's post-opening party against two Zoran Bults at positions
    /// `$0E` and `$1A`), so the shell builds those lists itself and hands them
    /// to this constructor. Nothing here can mutate game state, because there
    /// is no battle and no round: the frame only draws.
    #[must_use]
    pub fn command_idle(party: Vec<PartyStatus>, enemies: Vec<EnemyStatus>) -> BattleView {
        let mut bytes = [0; 5];
        let panes = super::panes::refresh(&mut bytes, &party);
        BattleView {
            menu: Some(MenuView::Top { cursor: 0 }),
            ready: true,
            panes,
            party,
            enemies,
            ..BattleView::default()
        }
    }
}
