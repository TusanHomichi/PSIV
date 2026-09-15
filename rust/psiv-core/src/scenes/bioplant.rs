//! Ordinary BioPlant traversal events, transcribed from the US cartridge.
use crate::{ActorRef, EventIndex, Flag, Scene, SceneOp};

/// `$06B92E..$06B9AD`: animate the two Birth Valley door chunks, then set
/// temporary flag 0. The DMA delay is DBRA $10, hence 17 VBlanks per stage.
pub static BIOPLANT_DOOR: Scene = Scene {
    name: "Event_BioPlantDoorOpening",
    event: EventIndex(8),
    ops: &[
        SceneOp::PlaySound { id: 0xE2 },
        SceneOp::WriteActorMapChunks {
            actor: ActorRef::PartyMember(0),
            chunks: &[(0, 0, 0x42), (0, 32, 0x44)],
        },
        SceneOp::WaitFrames { frames: 17 },
        SceneOp::WriteActorMapChunks {
            actor: ActorRef::PartyMember(0),
            chunks: &[(0, 0, 0x43), (0, 32, 0x45)],
        },
        SceneOp::WaitFrames { frames: 17 },
        SceneOp::WriteActorMapChunks {
            actor: ActorRef::PartyMember(0),
            chunks: &[(0, 0, 0x29), (0, 32, 0x2A)],
        },
        SceneOp::WaitFrames { frames: 17 },
        SceneOp::SetFlag {
            flag: Flag::temp(0),
            value: true,
        },
    ],
};

const LEADER: ActorRef = ActorRef::PartyMember(0);

/// `$06C2BC`: the runtime checks the live collision-plane chunk is `$4F`
/// before accepting the interaction. Each DMA call holds one VBlank; the
/// following DBF is a CPU delay, not a second frame.
pub static ELEVATOR_DOOR: Scene = Scene {
    name: "Event_ElevatorDoorOpening",
    event: EventIndex(0x13),
    ops: &[
        SceneOp::PlaySound { id: 0xE5 },
        SceneOp::WriteActorMapChunks {
            actor: LEADER,
            chunks: &[(0, 0, 0x50)],
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::WriteActorMapChunks {
            actor: LEADER,
            chunks: &[(0, 0, 0x51)],
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::WriteActorMapChunks {
            actor: LEADER,
            chunks: &[(0, 0, 0x52)],
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::WriteActorMapChunks {
            actor: LEADER,
            chunks: &[(0, 0, 0x53)],
        },
        SceneOp::WaitFrames { frames: 1 },
    ],
};

/// `$06C33E`: use this map's transition table, open the arrival door, step
/// out, overlap the party, then close the chunk behind them.
pub static ELEVATOR_RIDE: Scene = Scene {
    name: "Event_RidingElevator",
    event: EventIndex(0x14),
    ops: &[
        SceneOp::PlaySound { id: 0xE0 },
        SceneOp::FadeOut,
        SceneOp::WaitFrames { frames: 14 },
        SceneOp::TakeMapTransition,
        SceneOp::WriteActorMapChunks {
            actor: LEADER,
            chunks: &[(0, 16, 0x53)],
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::FadeIn,
        SceneOp::WaitFrames { frames: 14 },
        SceneOp::MoveActorOffset {
            actor: LEADER,
            dx: 0,
            dy: 16,
            wait: true,
        },
        SceneOp::OverlapCharacters,
        SceneOp::PlaySound { id: 0xE5 },
        SceneOp::WriteActorMapChunks {
            actor: LEADER,
            chunks: &[(0, -16, 0x52)],
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::WriteActorMapChunks {
            actor: LEADER,
            chunks: &[(0, -16, 0x51)],
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::WriteActorMapChunks {
            actor: LEADER,
            chunks: &[(0, -16, 0x50)],
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::WriteActorMapChunks {
            actor: LEADER,
            chunks: &[(0, -16, 0x4F)],
        },
        SceneOp::WaitFrames { frames: 1 },
    ],
};
