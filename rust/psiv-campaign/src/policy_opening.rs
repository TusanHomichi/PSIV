//! The `psycho_wand_then_win` policy: open a scripted battle with one item,
//! then fight it as [`PartyPolicy`] does.
//!
//! Some story fights are won by an item, not by damage: the Psycho Wand,
//! used from the battle's ITEM menu, strips the false form from the enemy
//! Zio shows first (`next_arc.rs` `use_psycho_wand`, which asserts the
//! enemy's stats reload). A player opens the first round that accepts party
//! commands with the item: its first actor takes ITEM, picks it, and everyone
//! else fights. A scripted ambush lets the enemy act first without opening a
//! command window; that enemy-only round cannot spend the opening. After the
//! item round this is the party policy, unchanged.
//!
//! The item is the route's knowledge, named by the policy (a route author
//! chooses `psycho_wand_then_win` for the chapter that fights Zio); only a
//! scripted battle gets the opening, and random encounters are run from as
//! `run_then_win` does. The policy decides *what* to pick, an
//! [`Intent::Item`]; `battle.rs` steers the ITEM page to that row.

use psiv_runtime::{CommandMenuView, Runtime};

use crate::policy::{Intent, PartyPolicy, Policy};

/// The Psycho Wand's cartridge item id.
pub const PSYCHO_WAND: u8 = 0x39;

/// Use one item in the first player-command round, then fight to win.
#[derive(Debug)]
pub struct OpeningItemPolicy {
    name: &'static str,
    item: u8,
    inner: PartyPolicy,
    /// The battle in progress is scripted and its opening is not yet spent.
    armed: bool,
    current: Option<(u8, Intent)>,
}

impl OpeningItemPolicy {
    /// The policy that opens scripted battles with item `item`, named `name`.
    #[must_use]
    pub fn new(name: &'static str, item: u8) -> OpeningItemPolicy {
        OpeningItemPolicy {
            name,
            item,
            inner: PartyPolicy::running(name),
            armed: false,
            current: None,
        }
    }

    /// The opening choice: the item, when the pack holds it.
    fn opening(&self, runtime: &Runtime) -> Option<Intent> {
        if !runtime.game().inventory().contains(self.item) {
            return None;
        }
        let item = runtime.battle_items().find(|item| item.id == self.item)?;
        Some(Intent::Item {
            name: item.name.clone(),
            target: None,
        })
    }
}

impl Policy for OpeningItemPolicy {
    fn name(&self) -> &'static str {
        self.name
    }

    fn battle_begins(&mut self, boss: bool) {
        self.armed = boss;
        self.current = None;
        self.inner.battle_begins(boss);
    }

    fn choose(&mut self, menu: &CommandMenuView, runtime: &Runtime) -> Intent {
        let Some(actor) = menu.actor else {
            return Intent::Defend;
        };
        if let Some((who, intent)) = &self.current
            && *who == actor
        {
            return intent.clone();
        }
        if self.armed {
            self.armed = false;
            if let Some(intent) = self.opening(runtime) {
                crate::policy::trace_choice(menu, actor, &intent);
                self.current = Some((actor, intent.clone()));
                return intent;
            }
        }
        self.inner.choose(menu, runtime)
    }

    fn wants_run(&self, boss: bool) -> bool {
        self.inner.wants_run(boss)
    }

    fn end_round(&mut self) {
        self.current = None;
        self.inner.end_round();
    }

    fn refuse(&mut self) {
        if let Some((actor, _)) = self.current {
            self.current = Some((actor, Intent::ATTACK));
        } else {
            self.inner.refuse();
        }
    }
}
