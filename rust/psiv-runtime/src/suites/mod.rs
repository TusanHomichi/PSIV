//! The gameplay suites that drive the runtime's own rules, kept inside the
//! crate.
//!
//! These files were the crate's integration tests (`rust/psiv-runtime/tests/`)
//! until node S6 closed the runtime's mutators: a test that ticks the field
//! with an [`Input`](psiv_core::Input), builds `RoundOrders`, seeds the RNG or
//! acknowledges a scene's dialogue is a test of a *rule*, and a `pub(crate)`
//! rule is invisible to a target outside the crate. Moving them here — next to
//! the code they exercise, like `encounters_elements_tests` and
//! `dialogue/runner_tests` already were — is what let every runtime mutator
//! become crate-private. The game-level tests (a session driven by pads) stay
//! in `rust/psiv-runtime/tests/` and `rust/psiv-campaign/tests/`.

mod boss_battles;
mod boss_talk_repro;
mod camp_abilities;
mod camp_equipment;
mod camp_order;
mod camp_recovery;
mod camp_status;
mod combat_death;
mod combat_enemy_attacks;
mod combat_enemy_effects;
mod combat_enemy_poison;
mod combat_fission;
mod combat_items;
mod combat_recovery;
mod combat_rewards;
mod combat_rimit;
mod combat_route_damage;
mod combat_skills;
mod combat_techniques;
mod dialogue_choice;
mod dialogue_resume;
mod dorin_dialogue;
mod encounters;
mod golden;
mod new_game;
mod next_arc;
mod next_arc_battles;
mod next_arc_scenes;
mod opening;
mod player_abilities;
mod presentation_order;
mod progression;
mod retail_boundaries;
mod save;
mod talk_repro;
mod visibility;
