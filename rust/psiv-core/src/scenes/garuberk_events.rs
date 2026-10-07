//! The Garuberk Tower's doors and eyes (issues #56 and #83), the way to Dark
//! Force 2. Records `docs/scenes/103_GaruberkTowerDoors.md` and
//! `104_GaruberkTowerEyes.md`. Every body was read from the US image through
//! its `EventPtrs` slot (`$05A2B4`); the clone's labels are navigation only.
//!
//! A tower door is a pair of chunks in the layout: the one under the leader and
//! the one above it. Speaking at a closed door (chunk `$38` under the leader)
//! runs a door opening (`$35`, `$36`), which steps a table of chunk pairs at
//! the leader's live position. Standing in the open door afterwards fires
//! `RunEvent_EnterGrbkTwDoor` (`$22`), which picks a door entered (`$37`, `$38`):
//! it takes the map's own transition, opens the arrival door around the
//! leader, walks the leader one cell out and shuts the door behind. The tables
//! are the cartridge's (`loc_6F4D4`, `loc_6F588`, `loc_6F714`, `loc_6F894`);
//! the opening's `cmpi.b #$38, (a1)` guard is the runtime's interaction probe
//! (`CLOSED_DOOR_GUARDS`, `rust/psiv-runtime/src/field_triggers.rs`).

use crate::scene::{ActorRef, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);

/// `SFXID_EnemyAttack3` (`ps4.constants.asm:1011`): the door entered's step out.
const SFX_ENEMY_ATTACK_3: u8 = 0xD7;
/// `SFXID_EnemyAttack4` (`ps4.constants.asm:1012`): the door shutting.
const SFX_ENEMY_ATTACK_4: u8 = 0xD8;
/// `SFXID_Fusion` (`ps4.constants.asm:1013`): a door opening, an eye.
const SFX_FUSION: u8 = 0xD9;

/// `TempEveFlag_GrbkTwEyeball` (`ps4.constants.asm:1837`).
const TEMP_EYEBALL: u16 = 0x14;
/// `TempEveFlag_GrbkTwEyeball2` (`ps4.constants.asm:1839`).
const TEMP_EYEBALL_2: u16 = 0x16;

/// One table row's frames: `d0 = 3` around `RefreshPlane`, `RunMapUpdates` and
/// `DMAPlanes_VInt`, four map-update frames.
const ROW_FRAMES: SceneOp = SceneOp::Wait { ticks: 4 };

/// `PalFadeOut_ClrSpriteTbl` and `Pal_FadeIn`, held as the elevator ride holds
/// them (`docs/scenes/SCENE_PRESENTATION.md`: 14 renderer ticks).
const FADE_FRAMES: SceneOp = SceneOp::WaitFrames { frames: 14 };

/// A table row: the chunk above the leader (`-$20`) and the one under.
const fn row(chunks: &'static [(i32, i32, u16)]) -> SceneOp {
    SceneOp::WriteActorMapChunks {
        actor: LEADER,
        chunks,
    }
}

/// `$0035`, `Event_GaruberkTwDoorOpening1`, retail `$06F43A..$06F4EF`
/// (`ps4.asm:148638-148692`): the guard, `SFXID_Fusion`, then `loc_6F4D4`'s
/// nine rows, ending on `$34` above and `$3C` under.
pub static DOOR_OPENING_1: Scene = Scene {
    name: "Event_GaruberkTwDoorOpening1",
    event: EventIndex(0x0035),
    ops: &[
        SceneOp::PlaySound { id: SFX_FUSION },
        row(&[(0, -32, 0x30), (0, 0, 0x38)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x31), (0, 0, 0x39)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x32), (0, 0, 0x3A)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x31), (0, 0, 0x39)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x30), (0, 0, 0x38)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x31), (0, 0, 0x39)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x32), (0, 0, 0x3A)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x33), (0, 0, 0x3B)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x34), (0, 0, 0x3C)]),
        ROW_FRAMES,
    ],
};

/// `$0036`, `Event_GaruberkTwDoorOpening2`, retail `$06F4F0..$06F5A3`
/// (`ps4.asm:148694-148746`): the same, from `loc_6F588`, ending on `$3E`
/// above and `$3C` under.
pub static DOOR_OPENING_2: Scene = Scene {
    name: "Event_GaruberkTwDoorOpening2",
    event: EventIndex(0x0036),
    ops: &[
        SceneOp::PlaySound { id: SFX_FUSION },
        row(&[(0, -32, 0x35), (0, 0, 0x38)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x36), (0, 0, 0x39)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x37), (0, 0, 0x3A)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x36), (0, 0, 0x39)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x35), (0, 0, 0x38)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x36), (0, 0, 0x39)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x37), (0, 0, 0x3A)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x3D), (0, 0, 0x3B)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x3E), (0, 0, 0x3C)]),
        ROW_FRAMES,
    ],
};

/// `$0037`, `Event_GaruberkTwDoorEntered1`, retail `$06F5A4..$06F723`
/// (`ps4.asm:148747-148848`): fade out, `DoMapTransitionData` on the map's own
/// table and `RefreshMap`; on the arrival map `$3C` under the leader and `$3E`
/// at `y - $10`, one refreshed frame, fade in; `SFXID_EnemyAttack3` and the
/// walk loop to `y + $10` (`tst.w $2A(a4)`); `Event_OverlapCharacters`;
/// `SFXID_EnemyAttack4` and `loc_6F714`'s five rows back to the closed door.
pub static DOOR_ENTERED_1: Scene = Scene {
    name: "Event_GaruberkTwDoorEntered1",
    event: EventIndex(0x0037),
    ops: &[
        SceneOp::FadeOut,
        FADE_FRAMES,
        SceneOp::TakeMapTransition,
        row(&[(0, 0, 0x3C), (0, -16, 0x3E)]),
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::FadeIn,
        FADE_FRAMES,
        SceneOp::PlaySound {
            id: SFX_ENEMY_ATTACK_3,
        },
        SceneOp::MoveActorOffset {
            actor: LEADER,
            dx: 0,
            dy: 16,
            wait: true,
        },
        SceneOp::OverlapCharacters,
        SceneOp::PlaySound {
            id: SFX_ENEMY_ATTACK_4,
        },
        row(&[(0, -32, 0x3E), (0, 0, 0x3C)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x3D), (0, 0, 0x3B)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x37), (0, 0, 0x3A)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x36), (0, 0, 0x39)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x35), (0, 0, 0x38)]),
        ROW_FRAMES,
    ],
};

/// `$0038`, `Event_GaruberkTwDoorEntered2`, retail `$06F724..$06F8A3`
/// (`ps4.asm:148849-148950`): as `$37`, with `$34` at `y - $10` and
/// `loc_6F894`'s rows.
pub static DOOR_ENTERED_2: Scene = Scene {
    name: "Event_GaruberkTwDoorEntered2",
    event: EventIndex(0x0038),
    ops: &[
        SceneOp::FadeOut,
        FADE_FRAMES,
        SceneOp::TakeMapTransition,
        row(&[(0, 0, 0x3C), (0, -16, 0x34)]),
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::FadeIn,
        FADE_FRAMES,
        SceneOp::PlaySound {
            id: SFX_ENEMY_ATTACK_3,
        },
        SceneOp::MoveActorOffset {
            actor: LEADER,
            dx: 0,
            dy: 16,
            wait: true,
        },
        SceneOp::OverlapCharacters,
        SceneOp::PlaySound {
            id: SFX_ENEMY_ATTACK_4,
        },
        row(&[(0, -32, 0x34), (0, 0, 0x3C)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x33), (0, 0, 0x3B)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x32), (0, 0, 0x3A)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x31), (0, 0, 0x39)]),
        ROW_FRAMES,
        row(&[(0, -32, 0x30), (0, 0, 0x38)]),
        ROW_FRAMES,
    ],
};

/// `$0039`, `Event_GaruberkTwEyeAction1`, retail `$06F8A4..$06F995`
/// (`ps4.asm:148951-149019`): three eye frames `KosDecomp`ed to `RAM_Start`,
/// `SFXID_Fusion`, `TempEveFlag_GrbkTwEyeball`, sixty frames of the eye, then
/// GaruberkTower_Part2's other layout, the one `MapDataMan_GaruberkTowerPart2`
/// loads while the flag is set.
pub static EYE_ACTION_1: Scene = Scene {
    name: "Event_GaruberkTwEyeAction1",
    event: EventIndex(0x0039),
    ops: &[
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x1C_7460,
                destination_ram: 0xFFFF_0000,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x1C_79F0,
                destination_ram: 0xFFFF_09E0,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x1C_7FE0,
                destination_ram: 0xFFFF_13C0,
            },
        },
        SceneOp::PlaySound { id: SFX_FUSION },
        SceneOp::SetFlag {
            flag: Flag::temp(TEMP_EYEBALL),
            value: true,
        },
        SceneOp::Presentation {
            op: PresentationOp::GaruberkEyeArtCycle { frames: 60 },
        },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::ReplaceMapLayout {
            fg: 0x1C_8F7A,
            bg: 0x1C_92BA,
        },
    ],
};

/// `$003A`, `Event_GaruberkTwEyeAction2`, retail `$06F996..$06FA33`
/// (`ps4.asm:149020-149073`): `SFXID_Fusion`, 120 frames of the eye, then
/// `TempEveFlag_GrbkTwEyeball2`, which `MapDataMan_GaruberkTowerPart6` reads at
/// the next load of Part6. The layout here does not change.
pub static EYE_ACTION_2: Scene = Scene {
    name: "Event_GaruberkTwEyeAction2",
    event: EventIndex(0x003A),
    ops: &[
        SceneOp::PlaySound { id: SFX_FUSION },
        SceneOp::Presentation {
            op: PresentationOp::GaruberkEyeArtCycle { frames: 120 },
        },
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::SetFlag {
            flag: Flag::temp(TEMP_EYEBALL_2),
            value: true,
        },
    ],
};
