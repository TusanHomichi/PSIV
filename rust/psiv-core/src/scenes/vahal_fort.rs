//! Vahal Fort's events (issue #82): the entrance and midway conversations, the
//! force-field barrier, the Dominators' aftermath, the Positron Bolt unit, the
//! belt-direction terminal and the two moving platforms.
//!
//! Records are `docs/scenes/110_*` onward and `docs/field/PLATFORMS_AND_BELTS.md`.
//! Every body was read from the US image through its `EventPtrs` slot
//! (`$05A2B4`); the clone's labels are navigation only.

use super::vahal_common::{
    PLATFORM_SETTLE_FRAMES, SFX_ALARM, SFX_ANDROID_SKILL_IMPLANT, SFX_CHEST_OPENED,
    SFX_CONVEYOR_BELT, SFX_POWER_DOWN, SOUND_STOP_SFX, platform_object, ride, set_event, sound,
    standard, test_temp, toggle_temp, wait,
};
use super::{CHAZ, DEMI, GRYZ, HAHN, KYRA, RAJA, RIKA, RUNE, SETH, WREN};
use crate::geom::Direction;
use crate::scene::{ActorRef, Axis, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::scene_runner::drift::CoordCmp;
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);

/// Vahal Fort F2 holds three map objects (`Elevator` x3), so `Field_LoadObject`
/// hands a platform the fourth object slot (`$FFFFC3C0`).
const PLATFORM_SLOT: usize = 3;

/// `$008D`, `Event_VahalFortEntrance`, retail `$07318E..$07319F`: Demi's
/// emergency call on the way in, then `EventFlag_VahalFort` (`$B4`).
pub static VAHAL_FORT_ENTRANCE: Scene = Scene {
    name: "Event_VahalFortEntrance",
    event: EventIndex(0x008D),
    ops: &[standard(0), set_event(0xB4)],
};

/// `$008E`, `Event_VahalFortMidway`, retail `$0731A0..$0731B1`: the
/// conversation in the conveyor-belt room, then `EventFlag_VahalFortMidway`
/// (`$B5`).
pub static VAHAL_FORT_MIDWAY: Scene = Scene {
    name: "Event_VahalFortMidway",
    event: EventIndex(0x008E),
    ops: &[standard(3), set_event(0xB5)],
};

/// `$0090`, `Event_VahalFortBarrier`, retail `$0731DA..$0731FF`: the first
/// touch of the force field explains it and sets `EventFlag_VahFortBarrier`
/// (`$B9`); later touches only say the party cannot get in.
pub static VAHAL_FORT_BARRIER: Scene = Scene {
    name: "Event_VahalFortBarrier",
    event: EventIndex(0x0090),
    ops: &[
        SceneOp::BranchFlag {
            flag: Flag::event(0xB9),
            if_set: 4,
            if_clear: 1,
        },
        standard(1),
        set_event(0xB9),
        SceneOp::End,
        standard(2),
    ],
};

/// One `Event_GetCharacter` / `move.w #x, $38(a4)` / `move.w #y, $3A(a4)`
/// pair: a character's destination, walked once the follow chain is off.
const fn go(who: crate::state::CharId, x: i32, y: i32) -> SceneOp {
    SceneOp::MoveActorTo {
        actor: ActorRef::Character(who),
        x,
        y,
        wait: false,
    }
}

/// `$0091`, `Event_DominatorsDefeated`, retail `$073200..$0733B9`: after the
/// Dominators fall the party gathers before Daughter's terminal, Wren
/// confronts her, her barrier's palette dies away and the leader steps back.
///
/// The fifth member (whoever of Hahn, Gryz, Demi, Raja, Kyra or Seth holds
/// the last slot, tested in that order) stands at `($2F0,$2E0)`; Chaz, Rune,
/// Rika and Wren are placed without a membership test.
pub static DOMINATORS_DEFEATED: Scene = Scene {
    name: "Event_DominatorsDefeated",
    event: EventIndex(0x0091),
    ops: &[
        // `$073200..$07324A`: Chaz, Rune, Rika and Wren.
        go(CHAZ, 0x2C0, 0x2E0),
        go(RUNE, 0x2E0, 0x2E0),
        go(RIKA, 0x300, 0x2E0),
        go(WREN, 0x2E0, 0x2B0),
        // `$073250..$073294`: the fifth member, first hit wins.
        SceneOp::BranchIfPartyMember {
            who: HAHN,
            if_present: 10,
            if_absent: 5,
        },
        SceneOp::BranchIfPartyMember {
            who: GRYZ,
            if_present: 12,
            if_absent: 6,
        },
        SceneOp::BranchIfPartyMember {
            who: DEMI,
            if_present: 14,
            if_absent: 7,
        },
        SceneOp::BranchIfPartyMember {
            who: RAJA,
            if_present: 16,
            if_absent: 8,
        },
        SceneOp::BranchIfPartyMember {
            who: KYRA,
            if_present: 18,
            if_absent: 9,
        },
        SceneOp::BranchIfPartyMember {
            who: SETH,
            if_present: 20,
            if_absent: 22,
        },
        // `loc_73296`: `($2F0,$2E0)` for the member found, then past the rest.
        go(HAHN, 0x2F0, 0x2E0),
        SceneOp::Jump { to: 22 },
        go(GRYZ, 0x2F0, 0x2E0),
        SceneOp::Jump { to: 22 },
        go(DEMI, 0x2F0, 0x2E0),
        SceneOp::Jump { to: 22 },
        go(RAJA, 0x2F0, 0x2E0),
        SceneOp::Jump { to: 22 },
        go(KYRA, 0x2F0, 0x2E0),
        SceneOp::Jump { to: 22 },
        go(SETH, 0x2F0, 0x2E0),
        SceneOp::Jump { to: 22 },
        // `$0732A2`: `FieldObj_Step_Offset` 0, `Char_Move_Flags` bits 0 and 1
        // (follow chain off, Y first), then `DoMainUpdatesLoop($77)`.
        SceneOp::SetStepOffset { value: 0 },
        SceneOp::SetFollowMode { bits: 0b11 },
        wait(120),
        // `$0732C0`: `loc_5A97C` with d0 = 4, every character faces up.
        SceneOp::FaceParty {
            facing: Direction::Up,
        },
        // `$0732C6`: dialogue `$05` up to its first yield ("Fo...rn...").
        standard(5),
        // `$0732CE..$0732E0`: 120 iterations of `RunMapUpdates` and `VInt_Prepare`.
        wait(120),
        sound(SFX_POWER_DOWN),
        // `$0732EC..$073326`: the barrier palette (`Palette_Line_2`, words 12
        // and 13) steps down a four-row table, sixteen frames a row.
        SceneOp::Presentation {
            op: PresentationOp::SetPaletteWords {
                offset: 0x18,
                first: 0x06CE,
                second: 0x0CE0,
            },
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::Presentation {
            op: PresentationOp::SetPaletteWords {
                offset: 0x18,
                first: 0x06CE,
                second: 0x0CE0,
            },
        },
        SceneOp::WaitFrames { frames: 16 },
        SceneOp::Presentation {
            op: PresentationOp::SetPaletteWords {
                offset: 0x18,
                first: 0x04AC,
                second: 0x0AC0,
            },
        },
        SceneOp::WaitFrames { frames: 16 },
        SceneOp::Presentation {
            op: PresentationOp::SetPaletteWords {
                offset: 0x18,
                first: 0x028A,
                second: 0x08A0,
            },
        },
        SceneOp::WaitFrames { frames: 16 },
        SceneOp::Presentation {
            op: PresentationOp::SetPaletteWords {
                offset: 0x18,
                first: 0x0068,
                second: 0x0680,
            },
        },
        SceneOp::WaitFrames { frames: 16 },
        // `$073310`: the `$FFFF` terminator's `VInt_Prepare`, then
        // `loc_5A79C` (`DMAPlanes_VInt` x30, no map updates).
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::WaitFrames { frames: 30 },
        // `$073334..$07333C`: Wren faces down, `$073342..$07334A` the
        // dialogue resumes after its yield.
        SceneOp::Face {
            actor: ActorRef::Character(WREN),
            facing: Direction::Down,
        },
        SceneOp::RunDialogueResume,
        // `$073350`: `EventFlag_DaughterShutDown` (`$BB`).
        set_event(0xBB),
        // `$07335A`: both follow-chain bits off, then the leader to
        // `($2E0,$2F0)`.
        SceneOp::SetFollowMode { bits: 0 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x2E0,
            y: 0x2F0,
            wait: true,
        },
        // `$073378`: `DoMainUpdatesLoop($3B)`.
        wait(60),
        SceneOp::SetStepOffset { value: 1 },
        // `$073386..$073394`: `Event_MoveCamera` on the leader's live
        // position at speed 2, a tail `jmp`.
        SceneOp::Presentation {
            op: PresentationOp::CameraToActor {
                actor: LEADER,
                speed: 2,
            },
        },
    ],
};

/// `$006F`, `Event_PosiBolt`, retail `$072520..$07259D`: Wren installs the
/// Positron Bolt unit from the chest at `$FFFFC500` (object 8, the chest after
/// Vahal Fort F3's eight map objects). `EventFlag_MuskCats` `$90` is the
/// cartridge's own name for the flag; here it records the unit as taken.
pub static POSI_BOLT: Scene = Scene {
    name: "Event_PosiBolt",
    event: EventIndex(0x006F),
    ops: &[
        sound(SFX_CHEST_OPENED),
        SceneOp::Face {
            actor: ActorRef::Npc(8),
            facing: Direction::Up,
        },
        wait(40),
        standard(6),
        wait(40),
        sound(SFX_ANDROID_SKILL_IMPLANT),
        wait(40),
        sound(SFX_ANDROID_SKILL_IMPLANT),
        wait(20),
        sound(SFX_ANDROID_SKILL_IMPLANT),
        wait(60),
        SceneOp::RunDialogueResume,
        SceneOp::SetCharacterSkill {
            who: WREN,
            slot: 6,
            skill: 0x0F,
        },
        set_event(0x90),
    ],
};

/// `$001B`, `Event_VahalFortTerminal`, retail `$06CDD4..$06CE17`: the small
/// terminal flashes the screen red twice and flips the belts' direction flag
/// for the room's half the leader stands in (`$0B` from `x = $1F0`, `$0C`
/// below it).
pub static VAHAL_FORT_TERMINAL: Scene = Scene {
    name: "Event_VahalFortTerminal",
    event: EventIndex(0x001B),
    ops: &[
        sound(SFX_ALARM),
        SceneOp::Presentation {
            op: PresentationOp::FadeToRed { lines: 3 },
        },
        sound(SFX_ALARM),
        SceneOp::Presentation {
            op: PresentationOp::FadeFromRed { lines: 3 },
        },
        SceneOp::BranchIfActorCoord {
            actor: LEADER,
            axis: Axis::X,
            cmp: CoordCmp::Below,
            value: 0x1F0,
            if_true: 7,
            if_false: 5,
        },
        toggle_temp(0x0B),
        SceneOp::End,
        toggle_temp(0x0C),
    ],
};

/// `$0015`, `Event_VahFortMovingPlatform1`, retail `$06C478..$06C607`: the
/// platform at chunks (22,17)-(23,17) (temp flag `$09` clear) or (22,22)-(23,22)
/// (set) carries the party to the other end, 80 frames at two pixels a frame.
pub static VAH_FORT_MOVING_PLATFORM_1: Scene = Scene {
    name: "Event_VahFortMovingPlatform1",
    event: EventIndex(0x0015),
    ops: &[
        test_temp(0x09, 12, 1),
        // Flag clear: start row 0, ride row 0.
        platform_object(PLATFORM_SLOT, 0x2C0, 0x220),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: &[(22, 17, 0xC2), (23, 17, 0xC3)],
        },
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        ride(PLATFORM_SLOT, 0x0200, 80),
        toggle_temp(0x09),
        SceneOp::WriteMapChunks {
            chunks: &[(22, 22, 0xCC), (23, 22, 0xCD)],
        },
        SceneOp::DespawnNpc {
            npc_index: PLATFORM_SLOT,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
        SceneOp::End,
        // Flag set: start row 1, ride row 1.
        platform_object(PLATFORM_SLOT, 0x2C0, 0x2C0),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: &[(22, 22, 0xC0), (23, 22, 0xC1)],
        },
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        ride(PLATFORM_SLOT, -0x0200, 80),
        toggle_temp(0x09),
        SceneOp::WriteMapChunks {
            chunks: &[(22, 17, 0xCC), (23, 17, 0xCD)],
        },
        SceneOp::DespawnNpc {
            npc_index: PLATFORM_SLOT,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
    ],
};

/// `$0016`, `Event_VahFortMovingPlatform2`, retail `$06C608..$06C797`: the
/// second platform, at chunks (22,29)-(23,29) (temp flag `$0A` clear) or
/// (22,24)-(23,24) (set), which rises 80 frames and sinks back.
pub static VAH_FORT_MOVING_PLATFORM_2: Scene = Scene {
    name: "Event_VahFortMovingPlatform2",
    event: EventIndex(0x0016),
    ops: &[
        test_temp(0x0A, 12, 1),
        // Flag clear: the platform rests at the bottom and rises.
        platform_object(PLATFORM_SLOT, 0x2C0, 0x3A0),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: &[(22, 29, 0xC0), (23, 29, 0xC1)],
        },
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        ride(PLATFORM_SLOT, -0x0200, 80),
        toggle_temp(0x0A),
        SceneOp::WriteMapChunks {
            chunks: &[(22, 24, 0xCC), (23, 24, 0xCD)],
        },
        SceneOp::DespawnNpc {
            npc_index: PLATFORM_SLOT,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
        SceneOp::End,
        // Flag set: it rests at the top and sinks.
        platform_object(PLATFORM_SLOT, 0x2C0, 0x300),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: &[(22, 24, 0xC0), (23, 24, 0xC1)],
        },
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        ride(PLATFORM_SLOT, 0x0200, 80),
        toggle_temp(0x0A),
        SceneOp::WriteMapChunks {
            chunks: &[(22, 29, 0xCC), (23, 29, 0xCD)],
        },
        SceneOp::DespawnNpc {
            npc_index: PLATFORM_SLOT,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
    ],
};
