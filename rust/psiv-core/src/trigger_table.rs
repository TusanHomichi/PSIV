//! `RunEventsJmpTbl` transcribed: all 128 retail entries.
//!
//! Source: `ps4.asm:115011` (the table) and the `RunEvent_*` routines at
//! `ps4.asm:115146-116620`. Each entry below carries its table index, the
//! routine's label and its line number.
//!
//! Distinct routine labels are 106 — index `$5C..$70` are twenty-one slots
//! pointing at one `RunEvent_MileSandWorm`, and `$7D..$7F` all reach
//! `RunEvent_NoEvent`.
//!
//! Flag ids are the numeric values of the `EventFlag_*` / `ChestFlag_*` /
//! `TempEveFlag_*` constants; the symbol name is in the comment beside each
//! entry so the two can be checked against `ps4.constants.asm`.

use crate::state::Flag;
use crate::trigger::{
    AxisPredicate, Condition, CustomTrigger, EventIndex, PositionPredicate, Trigger,
};

const NONE: &[Flag] = &[];

/// A flags-plus-position entry.
const fn cond(
    require_set: &'static [Flag],
    require_clear: &'static [Flag],
    x: AxisPredicate,
    y: AxisPredicate,
    event: u16,
) -> Trigger {
    Trigger::Condition(Condition {
        require_set,
        require_clear,
        position: PositionPredicate::new(x, y),
        event: EventIndex(event),
    })
}

/// A flags-only entry — the commonest shape, 57 of the 128.
const fn flags(
    require_set: &'static [Flag],
    require_clear: &'static [Flag],
    event: u16,
) -> Trigger {
    cond(
        require_set,
        require_clear,
        AxisPredicate::Any,
        AxisPredicate::Any,
        event,
    )
}

use AxisPredicate::{AtLeast, AtMost, Between, Exact};
use CustomTrigger as Cx;
use Trigger::{Custom, Never};

/// The table. Index it with a `RunEventsJmpTbl` index.
pub const TRIGGERS: [Trigger; 128] = [
    // $00 RunEvent_Null00 (115146)
    Never,
    // $01 RunEvent_Null01 (115149)
    Never,
    // $02 RunEvent_Null02 (115152)
    Never,
    // $03 RunEvent_FindingAlys (115156) — EventFlag_AlysFound clear
    cond(
        NONE,
        &[Flag::event(0x08)],
        Exact(0x260),
        AtLeast(0xF0),
        0x0003,
    ),
    // $04 RunEvent_Null04 (115171)
    Never,
    // $05 RunEvent_Null05 (115174)
    Never,
    // $06 RunEvent_MachineCenter (115178) — Zio set, MachineCenter clear
    cond(
        &[Flag::event(0x42)],
        &[Flag::event(0x43)],
        Between(0x710, 0x750),
        Exact(0xAD0),
        0x0006,
    ),
    // $07 RunEvent_Null07 (115198)
    Never,
    // $08 RunEvent_BasementContainers (115202) — BasementContainers clear
    flags(NONE, &[Flag::event(0x0D)], 0x000C),
    // $09 RunEvent_MeetingSayaUnused (115215) — Saya set
    flags(&[Flag::event(0x12)], NONE, 0x000D),
    // $0A RunEvent_SuspicionOnPrincipal (115223) — PrincipalMeeting set, PrincipalSuspicious clear
    flags(&[Flag::event(0x09)], &[Flag::event(0x0E)], 0x000F),
    // $0B RunEvent_Null0B (115234)
    Never,
    // $0C RunEvent_BioPlantAlarm (115238) — TempEveFlag_BioPlantAlarm clear.
    // A $F140 temp flag, unrelated to ChestFlag_Alshline of the same id.
    cond(
        NONE,
        &[Flag::temp(0x08)],
        AxisPredicate::Any,
        Exact(0x180),
        0x0012,
    ),
    // $0D RunEvent_RidingElevator (115250) — reads a map layout byte
    Custom(Cx::RidingElevator),
    // $0E RunEvent_VahFortMovingPlatform (115267) — XYRange half-open probes
    Custom(Cx::VahFortMovingPlatform),
    // $0F RunEvent_WpnPlntMovingPlatform (115307) — XYRange half-open probes
    Custom(Cx::WpnPlntMovingPlatform),
    // $10 RunEvent_VahFortConveyorBelt (115383) — 12 XYRange probes
    Custom(Cx::VahFortConveyorBelt),
    // $11 RunEvent_WpnPlntConveyorBelt (115482) — 12 XYRange probes
    Custom(Cx::WpnPlntConveyorBelt),
    // $12 RunEvent_Recovery (115574) — tile-collision rising edge
    Custom(Cx::Recovery),
    // $13 RunEvent_AfterIgglanova (115583) — Igglanova set, AfterIgglanova clear
    flags(&[Flag::event(0x0B)], &[Flag::event(0x0F)], 0x0025),
    // $14 RunEvent_Dorin (115594) — Dorin set, GryzJoined clear
    flags(&[Flag::event(0x36)], &[Flag::event(0x30)], 0x8004),
    // $15 RunEvent_UsingAlshline (115605) — AlshlineFound set, IgglanovaZema clear
    flags(&[Flag::event(0x32)], &[Flag::event(0x33)], 0x8005),
    // $16 RunEvent_MeetingSaya (115616) — Saya clear
    cond(
        NONE,
        &[Flag::event(0x12)],
        AxisPredicate::Any,
        AtMost(0x280),
        0x000D,
    ),
    // $17 RunEvent_ZemaIgglanovaDefeated (115627) — IgglanovaZema set, AfterIgglanovaZema clear
    flags(&[Flag::event(0x33)], &[Flag::event(0x37)], 0x8006),
    // $18 RunEvent_FindingAlshline (115638) — ChestFlag_Alshline set,
    // AlshlineFound clear. A $F120 bit; $0C's temp flag is a different array.
    flags(&[Flag::chest(0x08)], &[Flag::event(0x32)], 0x0028),
    // $19 RunEvent_RuneLadeaTower (115649) — RuneJoinedAgain clear
    cond(
        NONE,
        &[Flag::event(0x62)],
        AtLeast(0x3C0),
        AxisPredicate::Any,
        0x002E,
    ),
    // $1A RunEvent_MeetingRika (115660) — BioPlantEscape clear
    cond(
        NONE,
        &[Flag::event(0x34)],
        AxisPredicate::Any,
        Exact(0x1A0),
        0x8007,
    ),
    // $1B RunEvent_GettingLandRover (115672) — ChestFlag_ControlKey set, LandRover clear
    cond(
        &[Flag::chest(0x0A)],
        &[Flag::event(0x44)],
        AxisPredicate::Any,
        AtLeast(0x1C0),
        0x002B,
    ),
    // $1C RunEvent_SavingDemi (115686) — Zio clear
    cond(
        NONE,
        &[Flag::event(0x42)],
        AxisPredicate::Any,
        Exact(0x170),
        0x8008,
    ),
    // $1D RunEvent_AlysWounded (115698) — Zio set, DemiJoined clear
    flags(&[Flag::event(0x42)], &[Flag::event(0x47)], 0x8009),
    // $1E RunEvent_PsycoWandFound (115709) — ChestFlag_PsycoWand set, AfterAlysDeath2 clear
    flags(&[Flag::chest(0x09)], &[Flag::event(0x67)], 0x800A),
    // $1F RunEvent_ZioNurvus (115720) — ZioNurvus clear
    cond(
        NONE,
        &[Flag::event(0x65)],
        AxisPredicate::Any,
        Exact(0x1E0),
        0x0034,
    ),
    // $20 RunEvent_ZioDefeated (115732) — ZioNurvus set, GryzGone clear
    flags(&[Flag::event(0x65)], &[Flag::event(0x68)], 0x800B),
    // $21 RunEvent_EnterSpaceship (115743) — no flag test
    cond(NONE, NONE, Between(0x1E0, 0x1F0), Exact(0x120), 0x800D),
    // $22 RunEvent_EnterGrbkTwDoor (115755) — two map layout byte reads
    Custom(Cx::EnterGrbkTwDoor),
    // $23 RunEvent_KuranEnterSpaceship (115787) — no flag test
    cond(NONE, NONE, AxisPredicate::Any, Exact(0x2F0), 0x800D),
    // $24 RunEvent_AirCstlEnterSpaceship (115795) — no flag test
    cond(NONE, NONE, AxisPredicate::Any, Exact(0x370), 0x800D),
    // $25 RunEvent_SilenceTmEnterSpaceship (115802) — no flag test
    cond(NONE, NONE, AxisPredicate::Any, Exact(0x120), 0x800D),
    // $26 RunEvent_ChazHouseRest (115810) — TempEveFlag_ChazHouse clear
    flags(NONE, &[Flag::temp(0x18)], 0x003B),
    // $27 RunEvent_ClrChazHouseRest (115818) — TempEveFlag_ChazHouse set
    flags(&[Flag::temp(0x18)], NONE, 0x003C),
    // $28 RunEvent_SpaceshipSabotage (115826) — WrenJoined+Canceller set, ChaosSorcr clear
    cond(
        &[Flag::event(0x70), Flag::event(0x72)],
        &[Flag::event(0x71)],
        AxisPredicate::Any,
        Exact(0x2F0),
        0x800E,
    ),
    // $29 RunEvent_CrashLanding (115843) — ChaosSorcr set
    flags(&[Flag::event(0x71)], NONE, 0x800F),
    // $2A RunEvent_FindingPsycoWand (115851) — GyLaguiah clear
    cond(
        NONE,
        &[Flag::event(0x69)],
        Between(0x1E0, 0x1F0),
        Exact(0x1B0),
        0x002F,
    ),
    // $2B RunEvent_FindingLandale (115866) — TylerGrave set, DezoSpaceport clear
    cond(
        &[Flag::event(0x84)],
        &[Flag::event(0x82)],
        Between(0x1A0, 0x1B0),
        Exact(0x520),
        0x8010,
    ),
    // $2C RunEvent_EnterKuran (115884) — Kuran clear
    flags(NONE, &[Flag::event(0x86)], 0x003D),
    // $2D RunEvent_NearDarkForce (115892) — NearDarkForce1 clear
    cond(
        NONE,
        &[Flag::event(0x87)],
        AxisPredicate::Any,
        Exact(0x200),
        0x003E,
    ),
    // $2E RunEvent_FindDarkForce (115903) — DarkForce1 clear
    cond(
        NONE,
        &[Flag::event(0x83)],
        AxisPredicate::Any,
        Exact(0x0D0),
        0x003F,
    ),
    // $2F RunEvent_JuzaDefeated (115914) — Juza set, JuzaDefeated clear
    flags(&[Flag::event(0x41)], &[Flag::event(0x48)], 0x0041),
    // $30 RunEvent_RuneFlaeli (115925) — RuneJoined set, TonoePathOpen clear
    flags(&[Flag::event(0x11)], &[Flag::event(0x13)], 0x0027),
    // $31 RunEvent_OutsideRajaTemple (115936) — Snowstorm clear
    flags(NONE, &[Flag::event(0x80)], 0x0043),
    // $32 RunEvent_DarkForce1Defeated (115944) — DarkForce1 set, IceDigger clear
    flags(&[Flag::event(0x83)], &[Flag::event(0x89)], 0x8011),
    // $33 RunEvent_MeetingLeRoof (115955) — LeRoof clear
    flags(NONE, &[Flag::event(0xD1)], 0x0048),
    // $34 RunEvent_LeRoofAgain (115963) — Strength+Courage tower chests set, LeRoofStory1 clear
    flags(
        &[Flag::event(0xD5), Flag::event(0xD3)],
        &[Flag::event(0xD6)],
        0x801C,
    ),
    // $35 RunEvent_CarnivorousTrees (115977) — two complementary arms
    Custom(Cx::CarnivorousTrees),
    // $36 RunEvent_EclipseTorchUsed (116011) — Lashiec set, EclipseTorch clear
    cond(
        &[Flag::event(0x9B)],
        &[Flag::event(0x9C)],
        Between(0xB80, 0xBB0),
        AtMost(0x0E0),
        0x0047,
    ),
    // $37 RunEvent_FindDarkForce2 (116029) — DarkForce2 clear
    cond(
        NONE,
        &[Flag::event(0x9E)],
        AxisPredicate::Any,
        Exact(0x100),
        0x004E,
    ),
    // $38 RunEvent_DarkForce2Defeated (116040) — DarkForce2 set, SnowstormGone clear
    flags(&[Flag::event(0x9E)], &[Flag::event(0xA1)], 0x8017),
    // $39 RunEvent_LutzRevelation (116051) — LutzRevelation clear
    cond(
        NONE,
        &[Flag::event(0x97)],
        Between(0x1E0, 0x210),
        Between(0x1D0, 0x1F0),
        0x8014,
    ),
    // $3A RunEvent_MeetingSeth (116068) — SethJoined clear
    cond(
        NONE,
        &[Flag::event(0xC1)],
        Exact(0x770),
        Exact(0x9B0),
        0x8019,
    ),
    // $3B RunEvent_AeroPrism (116081) — ChestFlag_AeroPrism set, DarkForce3 clear
    flags(&[Flag::chest(0x0D)], &[Flag::event(0xC5)], 0x801A),
    // $3C RunEvent_DarkForce3Defeated (116092) — DarkForce3 set, DarkForce3Defeated clear
    flags(&[Flag::event(0xC5)], &[Flag::event(0xC6)], 0x0050),
    // $3D RunEvent_ReshelBattle (116103) — Reshel clear
    flags(NONE, &[Flag::event(0x8B)], 0x0053),
    // $3E RunEvent_ClmCenterForcedBattle (116111) — DezoGyLaguiah+DarkForce2 clear
    flags(NONE, &[Flag::event(0x92), Flag::event(0x9E)], 0x0054),
    // $3F RunEvent_ClmCenterAfterBattle (116122) — DezoGyLaguiah set; ClimateCenter+DarkForce2 clear
    flags(
        &[Flag::event(0x92)],
        &[Flag::event(0xA4), Flag::event(0x9E)],
        0x0055,
    ),
    // $40 RunEvent_FightingDElmLars (116136) — DElmLars+DarkForce2 clear
    cond(
        NONE,
        &[Flag::event(0x93), Flag::event(0x9E)],
        AxisPredicate::Any,
        Exact(0x0F0),
        0x0056,
    ),
    // $41 RunEvent_DElmLarsDefeated (116150) — DElmLars set; DElmLarsDefeated+DarkForce2 clear
    flags(
        &[Flag::event(0x93)],
        &[Flag::event(0xA5), Flag::event(0x9E)],
        0x0057,
    ),
    // $42 RunEvent_FindingAirCastle (116164) — EclipseTorchStolen set, AirCastleFound clear
    flags(&[Flag::event(0x98)], &[Flag::event(0x99)], 0x8015),
    // $43 RunEvent_EnterAirCastle (116175) — AirCastle clear
    flags(NONE, &[Flag::event(0x9F)], 0x0058),
    // $44 RunEvent_FindXeAThoul (116183) — XeAThoul clear
    cond(
        NONE,
        &[Flag::event(0x9A)],
        Between(0x1D0, 0x220),
        AtMost(0x220),
        0x0059,
    ),
    // $45 RunEvent_AirCstlFakeChest (116198) — ChestFlag_EclpsTorch set, Spector clear
    flags(&[Flag::chest(0x0C)], &[Flag::event(0xA6)], 0x005A),
    // $46 RunEvent_LashiecAppearing (116209) — Spector set, Lashiec clear
    flags(&[Flag::event(0xA6)], &[Flag::event(0x9B)], 0x005D),
    // $47 RunEvent_LashiecDefeated (116220) — Lashiec set
    flags(&[Flag::event(0x9B)], NONE, 0x8016),
    // $48 RunEvent_GumbiousBishop (116228) — Hydrofoil clear
    cond(
        NONE,
        &[Flag::event(0x9D)],
        Between(0x1E0, 0x1F0),
        AtMost(0x180),
        0x8018,
    ),
    // $49 RunEvent_StrengthTowerTop (116243) — StrengthTowerTop clear
    flags(NONE, &[Flag::event(0xE2)], 0x005E),
    // $4A RunEvent_CourageTowerTop (116251) — CourageTowerTop clear
    flags(NONE, &[Flag::event(0xE3)], 0x005F),
    // $4B RunEvent_DeVarsDefeated (116259) — DeVars set, DeVarsDefeated clear
    flags(&[Flag::event(0xD4)], &[Flag::event(0xE5)], 0x0064),
    // $4C RunEvent_SaLewsDefeated (116270) — SaLews set, SaLewsDefeated clear
    flags(&[Flag::event(0xD2)], &[Flag::event(0xE6)], 0x0065),
    // $4D RunEvent_Null4D (116281) — `moveq #1,d7 / rts`: fires, writes no index
    Trigger::AlwaysWithoutIndex,
    // $4E RunEvent_ReFaze (116285) — AlysFight set, ReFaze clear
    flags(&[Flag::event(0xE4)], &[Flag::event(0xE1)], 0x0063),
    // $4F RunEvent_BeforeElsydeonCave (116296) — LeRoofStory1 set, ElsydeonCave clear
    flags(&[Flag::event(0xD6)], &[Flag::event(0xD8)], 0x801D),
    // $50 RunEvent_Reunion (116307) — Elsydeon set, Reunion clear
    flags(&[Flag::event(0xD9)], &[Flag::event(0xDA)], 0x801F),
    // $51 RunEvent_AngerTowerTop (116318) — AlysFight clear
    cond(
        NONE,
        &[Flag::event(0xE4)],
        Between(0x1C0, 0x1D0),
        Between(0x250, 0x260),
        0x0069,
    ),
    // $52 RunEvent_AngerTowerExitTop (116337) — AngerTowerEnd clear
    cond(
        NONE,
        &[Flag::event(0xE7)],
        Between(0x1A0, 0x1B0),
        Exact(0x260),
        0x006A,
    ),
    // $53 RunEvent_ProfoundDarkness (116354) — ProfoundDarkness clear
    cond(
        NONE,
        &[Flag::event(0xE8)],
        AxisPredicate::Any,
        Exact(0x160),
        0x8020,
    ),
    // $54 RunEvent_Ending (116366) — ProfoundDarkness set
    flags(&[Flag::event(0xE8)], NONE, 0x8021),
    // $55 RunEvent_CancellerReminder (116374) — WrenJoined set; CancellerReminder + ChestFlag_Canceller clear
    cond(
        &[Flag::event(0x70)],
        &[Flag::event(0x73), Flag::chest(0x0B)],
        Between(0x1E0, 0x1F0),
        Exact(0x310),
        0x002A,
    ),
    // $56 RunEvent_EnterAngerTower (116397) — AngerTower clear
    flags(NONE, &[Flag::event(0xE0)], 0x0099),
    // $57 RunEvent_SoldiersTempleCaveDialogue1 (116405) — SethConversation1 clear
    flags(NONE, &[Flag::event(0xC2)], 0x009A),
    // $58 RunEvent_SoldiersTempleCaveDialogue2 (116413) — SethConversation2 clear
    flags(NONE, &[Flag::event(0xC3)], 0x009B),
    // $59 RunEvent_SoldiersTempleReached (116421) — SoldiersTemple clear
    flags(NONE, &[Flag::event(0xC4)], 0x009C),
    // $5A RunEvent_FindingAeroPrism (116429) — ChestFlag_AeroPrism set, AeroPrism1 clear
    flags(&[Flag::chest(0x0D)], &[Flag::event(0xC8)], 0x009D),
    // $5B RunEvent_ReenterPiata (116441) — PrincipalConfession clear
    flags(NONE, &[Flag::event(0x0C)], 0x009E),
    // $5C..$70 RunEvent_MileSandWorm (116449) — 21 slots on one routine; RNG gated.
    // The source marks $5C..$6F "Unused" and $70 as the live one.
    Custom(Cx::MileSandWorm), // $5C
    Custom(Cx::MileSandWorm), // $5D
    Custom(Cx::MileSandWorm), // $5E
    Custom(Cx::MileSandWorm), // $5F
    Custom(Cx::MileSandWorm), // $60
    Custom(Cx::MileSandWorm), // $61
    Custom(Cx::MileSandWorm), // $62
    Custom(Cx::MileSandWorm), // $63
    Custom(Cx::MileSandWorm), // $64
    Custom(Cx::MileSandWorm), // $65
    Custom(Cx::MileSandWorm), // $66
    Custom(Cx::MileSandWorm), // $67
    Custom(Cx::MileSandWorm), // $68
    Custom(Cx::MileSandWorm), // $69
    Custom(Cx::MileSandWorm), // $6A
    Custom(Cx::MileSandWorm), // $6B
    Custom(Cx::MileSandWorm), // $6C
    Custom(Cx::MileSandWorm), // $6D
    Custom(Cx::MileSandWorm), // $6E
    Custom(Cx::MileSandWorm), // $6F
    Custom(Cx::MileSandWorm), // $70
    // $71 RunEvent_InsideMonsenCave (116468) — InsideCave clear
    flags(NONE, &[Flag::event(0x2E)], 0x007C),
    // $72 RunEvent_SavingTallas (116475) — FractOoze set, TallasSaved clear
    flags(&[Flag::event(0x2F)], &[Flag::event(0x3E)], 0x007E),
    // $73 RunEvent_KingRappyDefeated (116486) — KingRappy set, SekreasReason clear
    flags(&[Flag::event(0xAF)], &[Flag::event(0xBA)], 0x0089),
    // $74 RunEvent_ZemaServantBattle (116497) — SilverSoldier set, Servants clear
    flags(&[Flag::event(0xB1)], &[Flag::event(0xB2)], 0x008A),
    // $75 RunEvent_EnterVahalFort (116508) — ZemaOldMan set, VahalFort clear
    flags(&[Flag::event(0xB3)], &[Flag::event(0xB4)], 0x008D),
    // $76 RunEvent_VahalFortMidway (116519) — VahalFort set, VahalFortMidway clear
    flags(&[Flag::event(0xB4)], &[Flag::event(0xB5)], 0x008E),
    // $77 RunEvent_DominatorsDefeated (116530) — Dominators set, DaughterShutDown clear
    flags(&[Flag::event(0xB6)], &[Flag::event(0xBB)], 0x0091),
    // $78 RunEvent_EnterWeaponPlant (116541) — WeaponPlant clear
    flags(NONE, &[Flag::event(0xC7)], 0x0092),
    // $79 RunEvent_StrippersDance (116549) — TempEveFlag_StrippersDance clear
    cond(
        NONE,
        &[Flag::temp(0x1B)],
        Between(0x1E0, 0x270),
        AtMost(0x240),
        0x0094,
    ),
    // $7A RunEvent_ClrStrippersDance (116564) — TempEveFlag_StrippersDance set
    flags(&[Flag::temp(0x1B)], NONE, 0x0095),
    // $7B RunEvent_PenguFeedStolen (116572) — scans the inventory
    Custom(Cx::PenguFeedStolen),
    // $7C RunEvent_PiataChazAlone (116585) — PiataChazControl clear
    flags(NONE, &[Flag::event(0x15)], 0x00A0),
    // $7D..$7F — all `bra.w RunEvent_NoEvent` (116617)
    Never,
    Never,
    Never,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameState;
    use crate::trigger::{PixelPos, TriggerContext, TriggerResult};

    fn ctx<'a>(state: &'a GameState, x: i32, y: i32) -> TriggerContext<'a> {
        TriggerContext {
            state,
            at: PixelPos { x, y },
            standing: None,
            previously_standing: None,
        }
    }

    #[test]
    fn the_table_has_exactly_the_retail_entry_count() {
        assert_eq!(TRIGGERS.len(), 128);
    }

    #[test]
    fn the_shape_census_matches_the_disassembly() {
        let mut never = 0;
        let mut always = 0;
        let mut custom = 0;
        let mut flags_only = 0;
        let mut positional = 0;
        for trigger in &TRIGGERS {
            match trigger {
                Trigger::Never => never += 1,
                Trigger::AlwaysWithoutIndex => always += 1,
                Trigger::Custom(_) => custom += 1,
                Trigger::Condition(c) => {
                    if c.position == PositionPredicate::ANYWHERE {
                        flags_only += 1;
                    } else {
                        positional += 1;
                    }
                }
            }
        }
        assert_eq!(never, 10, "null stubs");
        assert_eq!(always, 1, "the $4D always-fires stub");
        assert_eq!(custom, 30, "10 custom labels over 30 slots");
        assert_eq!(flags_only, 57, "flags with no position test");
        assert_eq!(positional, 30, "flags plus a position predicate");
        assert_eq!(never + always + custom + flags_only + positional, 128);
    }

    #[test]
    fn finding_alys_is_the_documented_golden_entry() {
        // "flag AlysFound clear + y >= $F0 + x == $260 -> event 3"
        let Trigger::Condition(c) = TRIGGERS[0x03] else {
            panic!("$03 should be a plain condition");
        };
        assert_eq!(c.require_set, NONE);
        assert_eq!(c.require_clear, &[Flag::event(0x08)]);
        assert_eq!(c.position.x, Exact(0x260));
        assert_eq!(c.position.y, AtLeast(0xF0));
        assert_eq!(c.event, EventIndex(3));
        assert!(!c.event.is_cutscene());

        let state = GameState::new();
        assert_eq!(
            TRIGGERS[0x03].evaluate(&ctx(&state, 0x260, 0xF0)),
            TriggerResult::Fire(EventIndex(3))
        );
        // One pixel short on y, and the wrong column, both miss.
        assert_eq!(
            TRIGGERS[0x03].evaluate(&ctx(&state, 0x260, 0xEF)),
            TriggerResult::NoEvent
        );
        assert_eq!(
            TRIGGERS[0x03].evaluate(&ctx(&state, 0x250, 0x200)),
            TriggerResult::NoEvent
        );

        // And once Alys is found it never fires again.
        let mut found = GameState::new();
        found.set(Flag::event(0x08)).unwrap();
        assert_eq!(
            TRIGGERS[0x03].evaluate(&ctx(&found, 0x260, 0xF0)),
            TriggerResult::NoEvent
        );
    }

    #[test]
    fn a_two_flag_requirement_needs_both() {
        // $28 SpaceshipSabotage: WrenJoined AND Canceller set, ChaosSorcr clear.
        let mut state = GameState::new();
        state.set(Flag::event(0x70)).unwrap();
        assert_eq!(
            TRIGGERS[0x28].evaluate(&ctx(&state, 0, 0x2F0)),
            TriggerResult::NoEvent,
            "one of the two set flags is not enough"
        );
        state.set(Flag::event(0x72)).unwrap();
        assert_eq!(
            TRIGGERS[0x28].evaluate(&ctx(&state, 0, 0x2F0)),
            TriggerResult::Fire(EventIndex(0x800E))
        );
        state.set(Flag::event(0x71)).unwrap();
        assert_eq!(
            TRIGGERS[0x28].evaluate(&ctx(&state, 0, 0x2F0)),
            TriggerResult::NoEvent,
            "the clear-requirement now fails"
        );
    }

    #[test]
    fn a_chest_flag_requirement_reads_the_chest_bank() {
        // $18 FindingAlshline: ChestFlag_Alshline set, EventFlag_AlshlineFound clear.
        let mut state = GameState::new();
        state.set(Flag::event(0x08)).unwrap();
        assert_eq!(
            TRIGGERS[0x18].evaluate(&ctx(&state, 0, 0)),
            TriggerResult::NoEvent,
            "an event flag with the same id must not satisfy a chest requirement"
        );
        state.set(Flag::chest(0x08)).unwrap();
        assert_eq!(
            TRIGGERS[0x18].evaluate(&ctx(&state, 0, 0)),
            TriggerResult::Fire(EventIndex(0x28))
        );
    }

    #[test]
    fn cutscene_indices_are_flagged() {
        let cutscenes = TRIGGERS
            .iter()
            .filter_map(|t| match t {
                Trigger::Condition(c) if c.event.is_cutscene() => Some(c.event),
                _ => None,
            })
            .count();
        assert_eq!(
            cutscenes, 28,
            "entries dispatching through the cutscene table"
        );
    }

    #[test]
    fn the_always_stub_is_not_a_no_op() {
        let state = GameState::new();
        assert_eq!(
            TRIGGERS[0x4D].evaluate(&ctx(&state, 0, 0)),
            TriggerResult::FireWithoutIndex
        );
    }
}
