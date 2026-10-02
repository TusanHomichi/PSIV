//! The debug selectors that bypass the front door.
//!
//! Both halves of a boot are the runtime's now: the fixtures
//! (`psiv-runtime/src/session/debug.rs`) and the shell's own field boot
//! (`Session::start(...).power_on()`/`field()`,
//! `psiv-runtime/src/session/start.rs`, the fallback spawn included), each of
//! which hands back a session. What stays here is the shell's own half: the
//! list of selectors that must not wait behind the retail front door.

use crate::input::requested_save_slot;

/// Title-screen bypasses used by save/debug fix loops. These are deliberately
/// checked before the title is started: a screenshot or a battle/camp/shop
/// selector must still reach the surface it was written to inspect.
pub(crate) fn title_bypassed() -> bool {
    let title_shot = std::env::var("PSIV_DEBUG_TITLE_SHOT").is_ok_and(|value| value == "1");
    requested_save_slot().is_some()
        || std::env::var_os("PSIV_DEBUG_BATTLE").is_some()
        || std::env::args().any(|argument| argument.starts_with("--psiv-debug-battle="))
        || std::env::var_os("PSIV_DEBUG_VEHICLE").is_some()
        || std::env::var_os("PSIV_DEBUG_VEHICLE_BATTLE").is_some()
        || std::env::var("PSIV_DEBUG_CAMP").is_ok_and(|value| value == "1")
        || std::env::var_os("PSIV_DEBUG_SHOP").is_some()
        || std::env::var_os("PSIV_DEBUG_EVENT").is_some()
        || (std::env::var_os("PSIV_DEBUG_SHOT").is_some() && !title_shot)
}

#[cfg(test)]
mod new_game_tests {
    use psiv_core::{CharId, Flag};
    use psiv_data::GameData;
    use psiv_runtime::Session;
    use std::path::Path;

    /// The retail title initializer: the session's own `new_game` — the call
    /// the title's START makes — copies the money and flag banks `loc_44414`
    /// writes before the opening runs. Debug scene fixtures deliberately
    /// construct their own state, so this is the only place the shipped START
    /// state is pinned.
    #[test]
    fn title_start_preserves_the_retail_initial_state() {
        let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
        if !pack.join("manifest.json").is_file() {
            eprintln!("runtime pack not present; skipping");
            return;
        }
        let data = GameData::load(pack).expect("pack loads");
        let session = Session::start(data).new_game().expect("new game starts");
        let runtime = session.runtime();
        // Retail loc_44414, verified by psiv_tools/newgame.py's opcode pins.
        assert_eq!(
            runtime.game().money(),
            500,
            "START must keep the starting meseta"
        );
        assert_eq!(runtime.game().party_members(), vec![CharId(0), CharId(1)]);
        for id in [
            0x127, 0x128, 0x12A, 0x134, 0x138, 0x150, 0x15B, 0x15D, 0x16B, 0x178, 0x1A7,
        ] {
            assert!(
                runtime.game().is_set(Flag::event(id)),
                "initial flag {id:#x}"
            );
        }
        for id in [0, 16, 25] {
            assert!(runtime.game().is_set(Flag::town(id)), "initial town {id}");
        }
        for id in [7, 21] {
            assert!(
                runtime.game().is_clear(Flag::event(id)),
                "opening must earn flag {id}"
            );
        }
    }
}
