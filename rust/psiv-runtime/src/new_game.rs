//! New-game initialization shared by the title and first-control boot paths.

use psiv_core::{Cell, CharId, Direction, Flag, GameState, StepFrames};
use psiv_data::{GameData, NewGame};

use crate::{BridgeError, Runtime, save};

pub(super) fn initial_state(init: &NewGame) -> GameState {
    let mut game = GameState::new();
    game.set_money(init.money);
    game.set_party(init.party.map(|id| id.map(CharId)));
    game.set_message_speed(init.message_speed);
    game.set_battle_speed(init.battle_speed);
    for id in &init.extended_event_flags {
        game.set(Flag::event(*id))
            .expect("validated initial event flag");
    }
    for id in &init.town_flags {
        game.set(Flag::town(*id))
            .expect("validated initial town flag");
    }
    game
}

impl Runtime {
    /// Constructs the state handed from retail START to `Event_GameStart`.
    /// Call `start_event` after arming battles. Unlike `new`, this does not
    /// pre-set the flags or party that the opening itself must establish.
    pub fn new_game(data: GameData, step_frames: StepFrames) -> Result<Self, BridgeError> {
        let init = data.new_game().ok_or_else(|| {
            BridgeError::Rejected("pack has no title initializer (game_start.json)".into())
        })?;
        let game = initial_state(init);
        let placement = save::RuntimePlacement::new(
            init.scene_map,
            // The scene hides the field and places both actors before their
            // first movement. Use a neutral on-map cell for the resident map.
            Cell::new(0, 0),
            Direction::Down,
            step_frames,
            init.world_index,
            0,
        );
        let followers = game.party_len().saturating_sub(1);
        save::construct_runtime(data, placement, game, followers)
    }
}
