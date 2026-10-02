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

/// One enemy of the current battle, in fighter order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnemyStatus {
    /// Retail fighter slot, 5-9.
    pub fighter: u8,
    /// Display name, the name the narration uses too.
    pub name: String,
    /// Whether the story still shows this enemy's sprite.
    pub visible: bool,
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
    /// Whether the actor can cast it now.
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

/// The per-character command window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMenuView {
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
    Commands(CommandMenuView),
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
        BattleView {
            menu: Some(MenuView::Top { cursor: 0 }),
            ready: true,
            party,
            enemies,
            ..BattleView::default()
        }
    }
}
