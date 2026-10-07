//! The conveyor belts of Vahal Fort F2 and Weapon Plant F3 (issue #82).
//!
//! `RunEvent_VahFortConveyorBelt` and `RunEvent_WpnPlntConveyorBelt` pick one
//! of these four events from where the leader stands and the terminals' temp
//! flags (`trigger_custom.rs`); each is the same carry loop with one direction
//! and one chunk range, which `SceneOp::ConveyorRide` is. Records are
//! `docs/field/PLATFORMS_AND_BELTS.md`; the bodies were read from the US
//! image through `EventPtrs[$1D..$20]` (`$06CE5C..$06D37B`).

use super::vahal_common::{SFX_CONVEYOR_BELT, SOUND_STOP_SFX, sound};
use crate::geom::Direction;
use crate::scene::SceneOp;
use crate::scene_runner::Scene;
use crate::trigger::EventIndex;

/// `cmpi.b #$A8, d7` / `cmpi.b #$AB, d7` (`$06CEDC`, `$06CEE2`): the vertical
/// belts' four chunk ids, one per animation frame of
/// `MapUpdate_VahFortConveyorBelts`.
const VERTICAL: (u8, u8) = (0xA8, 0xAB);
/// `cmpi.b #$AC, d7` / `cmpi.b #$AF, d7` (`$06D16A`, `$06D170`): the
/// horizontal belts'.
const HORIZONTAL: (u8, u8) = (0xAC, 0xAF);

/// The common body: `clr.b FieldObj_Step_Offset`, the carry, the followers
/// collapsed onto the leader (`Event_OverlapCharacters`), the offset back to 1
/// and the loop sound stopped.
const fn belt(direction: Direction, chunks: (u8, u8)) -> [SceneOp; 6] {
    [
        SceneOp::SetStepOffset { value: 0 },
        sound(SFX_CONVEYOR_BELT),
        SceneOp::ConveyorRide {
            direction,
            first_chunk: chunks.0,
            last_chunk: chunks.1,
        },
        SceneOp::OverlapCharacters,
        SceneOp::SetStepOffset { value: 1 },
        sound(SOUND_STOP_SFX),
    ]
}

static DOWN_OPS: [SceneOp; 6] = belt(Direction::Down, VERTICAL);
static UP_OPS: [SceneOp; 6] = belt(Direction::Up, VERTICAL);
static RIGHT_OPS: [SceneOp; 6] = belt(Direction::Right, HORIZONTAL);
static LEFT_OPS: [SceneOp; 6] = belt(Direction::Left, HORIZONTAL);

/// `$001D`, `Event_ConveyorBeltDown`, retail `$06CE5C..$06CFA3`: the leader's
/// destination is `curr_y_pos + $10` (`$06CF0A..$06CF12`).
pub static CONVEYOR_BELT_DOWN: Scene = Scene {
    name: "Event_ConveyorBeltDown",
    event: EventIndex(0x001D),
    ops: &DOWN_OPS,
};

/// `$001E`, `Event_ConveyorBeltUp`, retail `$06CFA4..$06D0EB`: `curr_y_pos - $10`
/// (`addi.w #$FFF0` at `$06D056`), over the same chunk ids as the down belt.
pub static CONVEYOR_BELT_UP: Scene = Scene {
    name: "Event_ConveyorBeltUp",
    event: EventIndex(0x001E),
    ops: &UP_OPS,
};

/// `$001F`, `Event_ConveyorBeltRight`, retail `$06D0EC..$06D233`: `curr_x_pos
/// + $10` (`$06D194..$06D19A`).
pub static CONVEYOR_BELT_RIGHT: Scene = Scene {
    name: "Event_ConveyorBeltRight",
    event: EventIndex(0x001F),
    ops: &RIGHT_OPS,
};

/// `$0020`, `Event_ConveyorBeltLeft`, retail `$06D234..$06D37B`: `curr_x_pos
/// - $10`, over the same chunk ids as the right belt.
pub static CONVEYOR_BELT_LEFT: Scene = Scene {
    name: "Event_ConveyorBeltLeft",
    event: EventIndex(0x0020),
    ops: &LEFT_OPS,
};
