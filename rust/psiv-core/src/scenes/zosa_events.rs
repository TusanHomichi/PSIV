//! Zosa's item shop (issue #56): the thief who runs off with the Pengu Feed.
//!
//! The record is `docs/scenes/161_PenguinFeedStolen.md`. The body was read
//! from the US image through its `EventPtrs` slot; the clone's labels are
//! navigation only.

use crate::geom::Direction;
use crate::scene::{ActorRef, Axis, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::scene_types::{ItemOp, ObjectOp};
use crate::trigger::EventIndex;

/// `Sound_StopMusic`.
const STOP_MUSIC: u8 = 0xFB;
/// `MusicID_DezorisDeDon`.
const MUSIC_DEZORIS_DE_DON: u8 = 0xB3;
/// `MusicID_NowOnSale`, the shop's own track.
const MUSIC_NOW_ON_SALE: u8 = 0x87;
/// `ItemID_PenguFeed`.
pub const ITEM_PENGU_FEED: u8 = 0x92;
/// `$FFFFC380`: slot 2, past `ZosaItemShop`'s two objects.
pub const THIEF: usize = 2;
/// `FieldObjectsJmpTbl` `$368`, `loc_4B30C`: walks to its `dest` words.
const THIEF_ROUTINE: u16 = 0x368;
/// The row the thief runs along.
const THIEF_Y: i32 = 0x270;

/// `move.w #(d1 * 4), d0 / jsr Event_UpdateObjFacing`, then two
/// `VInt_Prepare`: one beat of the thief's spin (`loc_7365E`).
const fn spin(facing: Direction) -> [SceneOp; 2] {
    [
        SceneOp::Face {
            actor: ActorRef::Npc(THIEF),
            facing,
        },
        SceneOp::WaitFrames { frames: 2 },
    ]
}

/// `d1` counts 0 to 3 and wraps, so the facings run down, up, right, left.
const SPIN_TURN: [SceneOp; 8] = {
    let [a, b] = spin(Direction::Down);
    let [c, d] = spin(Direction::Up);
    let [e, f] = spin(Direction::Right);
    let [g, h] = spin(Direction::Left);
    [a, b, c, d, e, f, g, h]
};

/// Index of the take in [`PENGUIN_FEED_STOLEN`], `loc_736D2`.
const TAKE: usize = 55;

/// `$0096`, `Event_PenguinFeedStolen`, retail `$0735F6..$0736E9`
/// (`ps4.asm:153259-153328`): map trigger `$7B` in `ZosaItemShop` fires it
/// on the row at `y = $260` while `GetItem` finds no Pengu Feed. The music
/// turns, a thief appears in slot 2, runs under the leader, spins five times,
/// runs off to the right and is gone; entry `$10` follows, the shop's music
/// comes back, and every Pengu Feed `GetItem` can find is taken.
pub static PENGUIN_FEED_STOLEN: Scene = Scene {
    name: "Event_PenguinFeedStolen",
    event: EventIndex(0x0096),
    ops: &[
        // 0-2: `$0735F6..$07360D`.
        SceneOp::PlaySound { id: STOP_MUSIC },
        SceneOp::Wait { ticks: 17 },
        SceneOp::PlaySound {
            id: MUSIC_DEZORIS_DE_DON,
        },
        // 3-5: `$07360E..$07363F`: the slot's words, one `RunSingleObject`,
        // the sprite rebuild and a `VInt_Prepare`.
        SceneOp::Object(ObjectOp::Spawn {
            slot: THIEF,
            routine: THIEF_ROUTINE,
            art_tile: 0x342,
            x: 0x150,
            y: THIEF_Y,
            solid: false,
        }),
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::WaitFrames { frames: 1 },
        // 6: `$073640..$073655`: `Event_MoveSingleObject` to the leader's X on
        // its own row.
        SceneOp::MoveActorToActorAxis {
            actor: ActorRef::Npc(THIEF),
            target: ActorRef::PartyMember(0),
            axis: Axis::X,
            wait: true,
        },
        // 7-46: `$073656..$07368B`: twenty beats.
        SPIN_TURN[0],
        SPIN_TURN[1],
        SPIN_TURN[2],
        SPIN_TURN[3],
        SPIN_TURN[4],
        SPIN_TURN[5],
        SPIN_TURN[6],
        SPIN_TURN[7],
        SPIN_TURN[0],
        SPIN_TURN[1],
        SPIN_TURN[2],
        SPIN_TURN[3],
        SPIN_TURN[4],
        SPIN_TURN[5],
        SPIN_TURN[6],
        SPIN_TURN[7],
        SPIN_TURN[0],
        SPIN_TURN[1],
        SPIN_TURN[2],
        SPIN_TURN[3],
        SPIN_TURN[4],
        SPIN_TURN[5],
        SPIN_TURN[6],
        SPIN_TURN[7],
        SPIN_TURN[0],
        SPIN_TURN[1],
        SPIN_TURN[2],
        SPIN_TURN[3],
        SPIN_TURN[4],
        SPIN_TURN[5],
        SPIN_TURN[6],
        SPIN_TURN[7],
        SPIN_TURN[0],
        SPIN_TURN[1],
        SPIN_TURN[2],
        SPIN_TURN[3],
        SPIN_TURN[4],
        SPIN_TURN[5],
        SPIN_TURN[6],
        SPIN_TURN[7],
        // 47-48: `$07368C..$07369B`: facing 0, `DoMainUpdatesLoop($1D)`.
        SceneOp::Face {
            actor: ActorRef::Npc(THIEF),
            facing: Direction::Down,
        },
        SceneOp::Wait { ticks: 30 },
        // 49-50: `$07369C..$0736AF`: off to `($2D0,$270)`, `clr.w (a4)`.
        SceneOp::MoveActorTo {
            actor: ActorRef::Npc(THIEF),
            x: 0x2D0,
            y: THIEF_Y,
            wait: true,
        },
        SceneOp::DespawnNpc {
            npc_index: THIEF,
            count: 1,
        },
        // 51: `$0736B0..$0736B9`: entry `$10`.
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x10)),
            window: DialogueWindow::Standard,
        },
        // 52-54: `$0736BA..$0736D1`.
        SceneOp::PlaySound { id: STOP_MUSIC },
        SceneOp::Wait { ticks: 17 },
        SceneOp::PlaySound {
            id: MUSIC_NOW_ON_SALE,
        },
        // 55-56: `loc_736D2`, `$0736D2..$0736E7`: take one and go again.
        SceneOp::Item(ItemOp::Take {
            item: ITEM_PENGU_FEED,
            if_missing: TAKE + 2,
        }),
        SceneOp::Jump { to: TAKE },
        // 57: `loc_736E8`, `rts`.
        SceneOp::End,
    ],
};
