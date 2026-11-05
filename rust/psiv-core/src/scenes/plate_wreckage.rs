//! The Plate System (issue #81): Phonon from the Plate System's chest and the
//! engine on its fourth floor. The Wreckage's system (`$29`) joins them later.
//!
//! Records are `docs/scenes/204_Phonon.md` and `205_PlateSysEngine.md`.
//! Every body was read from the US image through its `EventPtrs` slot; the
//! clone's labels are navigation only.

use super::vahal_common::{
    SFX_ANDROID_SKILL_IMPLANT, SFX_CHEST_OPENED, SFX_POWER_DOWN, sound, standard, wait,
};
use super::{CHAZ, DEMI, GRYZ, RIKA, RUNE};
use crate::geom::Direction;
use crate::scene::{ActorRef, SceneOp};
use crate::scene_presentation::{CameraSubject, PresentationOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `SFXID_Megid`.
const SFX_MEGID: u8 = 0xC0;
/// `SFXID_Recovery`.
const SFX_RECOVERY: u8 = 0xCD;
/// `Palette_Table_Buffer_2` and `Palette_Table_Buffer`.
const PALETTE_BUFFER_2: u32 = 0xFFFF_FB80;
/// See [`PALETTE_BUFFER_2`].
const PALETTE_BUFFER: u32 = 0xFFFF_FB00;

/// PlateSystem F3 holds three elevators, so its one chest is object 3
/// (`$FFFFC3C0`).
const PHONON_CHEST: ActorRef = ActorRef::Npc(3);

/// `move.b #mode, ($FFFFED52).w` then `jsr Pal_VariableFadeOut`/`In`.
const fn fade(fade_in: bool, mode: u8) -> SceneOp {
    SceneOp::Presentation {
        op: PresentationOp::VariablePaletteFade { fade_in, mode },
    }
}

/// `lea Palette_Table_Buffer_2, a0 / lea Palette_Table_Buffer, a1 / move.w
/// #$3F, d7 / trap #1`: sixty-four words back.
const RESTORE_PALETTE: SceneOp = SceneOp::Presentation {
    op: PresentationOp::CopyRamWords {
        source_ram: PALETTE_BUFFER_2,
        destination_ram: PALETTE_BUFFER,
        words: 64,
    },
};

/// `$006C`, `Event_Phonon`, retail `$0722F6..$072427`: the chest in Plate
/// System F3 opens; with Demi in the party she takes the Phonon unit (entry
/// `$4E`, the flashes, skill slot 5 := `SkillID_Phonon`); without her entry
/// `$4F` plays, the lid shuts and `ChestFlag_Phonon` (`$42`) is cleared so
/// the chest can be opened again.
pub static PHONON: Scene = Scene {
    name: "Event_Phonon",
    event: EventIndex(0x006C),
    ops: &[
        // 0-2
        sound(SFX_CHEST_OPENED),
        SceneOp::Face {
            actor: PHONON_CHEST,
            facing: Direction::Up,
        },
        wait(40),
        // 3: `FindCharacterSlot(CharID_Demi)`, `bmi.w loc_72404`.
        SceneOp::BranchIfPartyMember {
            who: DEMI,
            if_present: 4,
            if_absent: 29,
        },
        // 4-18
        standard(0x4E),
        fade(false, 5),
        wait(40),
        sound(SFX_ANDROID_SKILL_IMPLANT),
        wait(40),
        sound(SFX_ANDROID_SKILL_IMPLANT),
        wait(15),
        sound(SFX_ANDROID_SKILL_IMPLANT),
        wait(10),
        sound(SFX_MEGID),
        wait(30),
        sound(SFX_RECOVERY),
        wait(60),
        sound(SFX_ANDROID_SKILL_IMPLANT),
        wait(40),
        // 19-25: the flash, twice.
        RESTORE_PALETTE,
        wait(1),
        fade(true, 2),
        fade(false, 0),
        RESTORE_PALETTE,
        wait(1),
        fade(true, 0),
        // 26-28
        SceneOp::RunDialogueResume,
        SceneOp::SetCharacterSkill {
            who: DEMI,
            slot: 5,
            skill: 0x11,
        },
        SceneOp::End,
        // 29-32: `loc_72404`.
        standard(0x4F),
        sound(SFX_CHEST_OPENED),
        SceneOp::Face {
            actor: PHONON_CHEST,
            facing: Direction::Down,
        },
        SceneOp::SetFlag {
            flag: Flag::chest(0x42),
            value: false,
        },
    ],
};

const LEADER: ActorRef = ActorRef::PartyMember(0);

/// `Event_GetCharacter(who)` then `$38`/`$3A` := `(x, y)`: the follow logic
/// walks them there once `Char_Move_Flags` bit 0 frees them from the chain.
const fn dest(who: crate::state::CharId, x: i32, y: i32) -> SceneOp {
    SceneOp::SetActorDest {
        actor: ActorRef::Character(who),
        x,
        y,
    }
}

const fn face(who: crate::state::CharId, facing: Direction) -> SceneOp {
    SceneOp::Face {
        actor: ActorRef::Character(who),
        facing,
    }
}

/// `Palette_Line_2 + $18` (`Palette_Table_Buffer + $38`, line 2's word 12),
/// as the word offset [`PresentationOp::SetPaletteWords`] takes.
const ENGINE_LIGHT_WORDS: u16 = 0x1C;

/// `loc_6E842`'s sixteen passes over `loc_6E908`, each followed by
/// `loc_6E922`'s eight `VInt_Prepare`s. The second word is read `$E` past the
/// advanced pointer, so the sixth and twelfth passes read the first word of
/// `loc_6E922`'s code. The table's words stay in the pack
/// (`presentation/panels.json`, `palettes`).
const ENGINE_LIGHT: SceneOp = SceneOp::Presentation {
    op: PresentationOp::PaletteCycleFromTable {
        source_rom_addr: 0x0006_E908,
        offset: ENGINE_LIGHT_WORDS,
        second_skip: 0xE,
        steps: 16,
        frame_delay: 8,
    },
};

/// `$002D`, `Event_PlateSysEngine`, retail `$06E75A..$06E907`: the party
/// gathers at Plate System F4's engine, Demi reads it (entry `$41`), its
/// light pulses down and goes out, the leader steps back and
/// `EventFlag_PlateEngine` (`$61`) is set.
pub static PLATE_SYS_ENGINE: Scene = Scene {
    name: "Event_PlateSysEngine",
    event: EventIndex(0x002D),
    ops: &[
        // 0-5: the party's places around the engine; Rune only if present.
        dest(CHAZ, 0x1E0, 0x190),
        dest(GRYZ, 0x1C0, 0x180),
        dest(DEMI, 0x1E0, 0x170),
        dest(RIKA, 0x200, 0x190),
        SceneOp::BranchIfPartyMember {
            who: RUNE,
            if_present: 5,
            if_absent: 6,
        },
        dest(RUNE, 0x230, 0x170),
        // 6-8: `FieldObj_Step_Offset := 0`, `bset #0, Char_Move_Flags`,
        // `DoMainUpdatesLoop($4F)`.
        SceneOp::SetStepOffset { value: 0 },
        SceneOp::SetFollowMode { bits: 0b1 },
        wait(80),
        // 9-14
        face(CHAZ, Direction::Up),
        face(GRYZ, Direction::Up),
        face(RIKA, Direction::Up),
        face(DEMI, Direction::Up),
        SceneOp::BranchIfPartyMember {
            who: RUNE,
            if_present: 14,
            if_absent: 15,
        },
        face(RUNE, Direction::Right),
        // 15-16
        standard(0x41),
        sound(SFX_POWER_DOWN),
        // 17: the light.
        ENGINE_LIGHT,
        // 18-19: the last colour, immediates, then five more `loc_6E922` waits.
        SceneOp::Presentation {
            op: PresentationOp::SetPaletteWords {
                offset: ENGINE_LIGHT_WORDS,
                first: 0x0EC0,
                second: 0x00CE,
            },
        },
        SceneOp::WaitFrames { frames: 40 },
        // 20: `RunMapUpdates`, `VInt_Prepare`: one frame that updates the map.
        wait(1),
        // 21-24: Demi looks right, then down.
        face(DEMI, Direction::Right),
        wait(15),
        face(DEMI, Direction::Down),
        wait(30),
        // 25: `popdlg` / `Event_RunDialogue`.
        SceneOp::RunDialogueResume,
        // 26-28: `bclr #0, Char_Move_Flags`, `Event_MoveSingleObject` on
        // `Character_1` to `($1E0,$1C0)`, `DoMainUpdatesLoop($3B)`.
        SceneOp::SetFollowMode { bits: 0 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1E0,
            y: 0x1C0,
            wait: true,
        },
        wait(60),
        // 29-30: `FieldObj_Step_Offset := 1`, `Event_MoveCamera` to the
        // leader at speed 1.
        SceneOp::SetStepOffset { value: 1 },
        SceneOp::PanCamera {
            subject: CameraSubject::Actor(LEADER),
            speed: 1,
        },
        // 31-32: `loc_53F14` on `Map_Palettes_Addr`, `EventFlag_PlateEngine`.
        SceneOp::ReloadMapPalette,
        SceneOp::SetFlag {
            flag: Flag::event(0x61),
            value: true,
        },
    ],
};
