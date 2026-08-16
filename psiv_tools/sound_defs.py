"""Static, source-labelled definitions used by :mod:`psiv_tools.sound`.

Keeping the command vocabulary, pointer provenance and symbolic id tables out
of the extractor makes the decoder easier to audit.  The values are copied
from the checked-out ``reference/ps4disasm/sound`` files and are verified
against the retail ROM by ``sound.py``.
"""

from __future__ import annotations

from typing import Any


# DefCFlag.txt. Length includes the opcode; FF is the meta escape.
COMMAND_SPECS: dict[int, dict[str, Any]] = {
    0xE0: {"type": "PANAFMS", "subtype": "PAFMS_PAN", "length": 2},
    0xE1: {"type": "DETUNE", "subtype": None, "length": 2},
    0xE2: {"type": "SET_COMM", "subtype": None, "length": 2},
    0xE3: {"type": "DAC_PS4", "subtype": "PS4_VOLCTRL", "length": 2},
    0xE4: {"type": "DAC_PS4", "subtype": "PS4_LOOP", "length": 2},
    0xE5: {"type": "VOLUME", "subtype": "VOL_NN_FMP", "length": 3},
    0xE6: {"type": "VOLUME", "subtype": "VOL_NN_FM", "length": 2},
    0xE7: {"type": "HOLD", "subtype": None, "length": 1},
    0xE8: {"type": "NOTE_STOP", "subtype": "NSTOP_NORMAL", "length": 2},
    0xE9: {"type": "SET_LFO", "subtype": "LFO_AMSEN", "length": 3},
    0xEA: {"type": "TEMPO", "subtype": "TEMPO_SET", "length": 2},
    0xEB: {"type": "SND_CMD", "subtype": None, "length": 2},
    0xEC: {"type": "VOLUME", "subtype": "VOL_NN_PSG", "length": 2},
    0xED: {"type": "PANAFMS", "subtype": "PAFMS_PAN", "length": 2,
           "note": "DAC driver feature"},
    0xEE: {"type": "DAC_PS4", "subtype": "PS4_SET_SND", "length": 2},
    0xEF: {"type": "INSTRUMENT", "subtype": "INS_N_FM", "length": 2},
    0xF0: {"type": "MOD_SETUP", "subtype": None, "length": 5},
    0xF1: {"type": "MOD_ENV", "subtype": "MENV_FMP", "length": 3},
    0xF2: {"type": "TRK_END", "subtype": "TEND_STD", "length": 1},
    0xF3: {"type": "PSG_NOISE", "subtype": "PNOIS_SET", "length": 2},
    0xF4: {"type": "MOD_ENV", "subtype": "MENV_GEN", "length": 2},
    0xF5: {"type": "INSTRUMENT", "subtype": "INS_N_PSG", "length": 2},
    0xF6: {"type": "GOTO", "subtype": None, "length": 3,
           "relative_word_at": 1},
    0xF7: {"type": "LOOP", "subtype": None, "length": 5,
           "relative_word_at": 3},
    0xF8: {"type": "GOSUB", "subtype": None, "length": 3,
           "relative_word_at": 1},
    0xF9: {"type": "RETURN", "subtype": None, "length": 1},
    0xFA: {"type": "DAC_PS4", "subtype": "PS4_REVERSE", "length": 2},
    0xFB: {"type": "TRANSPOSE", "subtype": "TRNSP_ADD", "length": 2},
    0xFC: {"type": "DAC_PS4", "subtype": "PS4_VOLUME", "length": 2},
    0xFD: {"type": "DAC_PS4", "subtype": "PS4_TRKMODE", "length": 2},
    0xFE: {"type": "SPC_FM3", "subtype": None, "length": 5},
    0xFF: {"type": "META_CF", "subtype": None, "length": 1,
           "variable": True},
}

META_SPECS: dict[int, dict[str, Any]] = {
    0x00: {
        "type": "PAN_ANIM",
        "length": "3 when payload byte is 0; 7 otherwise",
        "payload": "one selector byte, then four bytes when selector is nonzero",
    },
}

MUSIC_NAMES = (
    "TonoeDePon", "Inn", "MotabiaVillage", "MotabiaTown", "OrganicBeat",
    "DezorisTown1", "NowOnSale", "BehindTheCircuit", "MachineCenter",
    "InTheCave", "Winners", "FieldMotabia", "LandMaster", "RequiemForLutz",
    "MeetThemHeadOn", "RyucrossField", "DungeonArrange1", "Fal",
    "TempleNgangbius", "Thray", "DefeatAtABlow", "CyberneticCarnival",
    "TerribleSight", "EdgeOfDarkness", "DezorisField1", "Tower",
    "TakeOffLandeel", "DezorisTown2", "DezorisField2", "AHappySettlement",
    "Suspicion", "TheKingOfTerrors", "TheAgeOfFables", "Abyss",
    "EnemyAppearance", "HerLastBreath", "Pain", "JijyNoRag",
    "DungeonArrange2Cont", "TheBlackBlood", "RedAlert", "Laughter", "Mystery",
    "EndOfTheMillennium", "Explosion", "StaffRoll", "ThePromisingFuture1",
    "PaoPao", "DungeonArrange2", "ThePromisingFuture2", "DezorisDeDon", "Ooze",
)

SFX_NAMES = (
    "Rod", "Shot", "Slasher", "AttackMiss", "EnemyKilled", "EnemyAttack1",
    "TechCast", "BuffCast", "HealTechCast", "Foi", "Legeon", "Megid",
    "Phonon", "FireBreath", "Efess", "Moonshad", "LaserAttack", "Zan", "Gra",
    "Vol", "Saner", "Rimit", "Brose", "Res", "Recovery", "Tandle",
    "AndroidSkillImplant", "Rifle", "SleepGas", "Eliminat", "Spark", "WarCry",
    "MoleAttack", "MechEnemyAlarm", "EnemyAttack3", "EnemyAttack4", "Fusion",
    "EnemyAttack5", "Alarm", "Deban", "GraveOpening", "Teleport", "Stairs",
    "RidingElevator", "ChestOpened", "DoorOpened", "SpaceshipPropelled", "PowerDown",
    "ElevatorOpen", "BarrierBroken", "ConveyorBelt", "Claw", "Unused1",
    "BlackWave", "EnemySpellCast", "Lightning", "AnotherGate", "IceBroken",
    "FallingIntoHole", "Telepipe", "Souvenir", "MovingCursor", "Selection",
    "Surprise", "Sword", "Nothing", "Unused2",
)

SPECIAL_NAMES = ("SpaceshipRadar", "LandRover", "Hydrofoil")


# Structural sentinels from SOUND_SCOUT.md.  They are checked before any
# pointer is trusted, so a different clone cannot silently produce a pack.
ANCHORS = (
    (0xD0008, "4D F9 00 FF 50 00 42 2E 00 0E 4A 2E 00 07 66 00"),
    (0xD0632, "4E 75 7E 00 1E 2E 00 09 67 00 05 DE 1D 7C 00 80"),
    (0xD1A60, "00 0D 1D 10 00 0D E5 C2 00 0D 1C 40 00 0D E4 B6"),
    (0xD1C40, "00 0D 1D 8E 00 0D 23 86 00 0D 24 94 00 0D 29 4E"),
    (0xD1D8E, "05 AC 06 03 02 09 05 34 00 00 00 41 F4 16 01 A5"),
    (0xD233A, "21 31 01 71 01 9B 1F 15 5F 07 0A 09 0C 08 08 05"),
    (0xDE4B6, "00 0D E5 CE 00 0D E5 F8 00 0D E6 20 00 0D E6 4E"),
    (0xDE5CE, "00 11 01 01 80 05 00 0A 00 04 EF 00 84 03 89 04"),
    (0xD153E, "F3 F3 31 00 01 0E 00 06 00 10 FE 0D 20 F9 C3 00"),
    (0xD1A3E, "00 00 00 01 00 02 00 03 00 04 00 05 01 00 00 07"),
    (0xE0000, "50 80 50 0C 50 80 50 0C A0 8C A0 04 A0 8C A0 04"),
)


# name, source label, start, end, expected SHA-256
KNOWN_REGIONS = (
    ("fm_init_bytes", "FMInitBytes", 0xD07D2, 0xD07D9,
     "6924eb8239a41e488bdbb60a2f20b777f74cbf0371deec0e19800f09014adbde"),
    ("psg_init_bytes", "PSGInitBytes", 0xD07DA, 0xD07DD,
     "03786718360da1008e6726ca6909e61d2ea1ab64de32a9f2d2ed3c7627692809"),
    ("spc_fm3_registers", "SpcFM3Regs", 0xD0490, 0xD0498,
     "b132776d98b8c3554166ceef1c85db7e42cc78f91686bcdb04b1b7745c7c246e"),
    ("pan_animation_pointers", "PanAnimPtrs", 0xD0530, 0xD053C,
     "d4a7f50561faa2515754df35e070ee457e91a316b5da0a34f72f24308170ad94"),
    ("pan_animation_data", "loc_D053C", 0xD053C, 0xD0545,
     "1afdc86179f443a9d51213420a42fe7c66bd43d7a24fdab372c891c040f8877c"),
    ("fm_frequencies", "FMFreqs", 0xD0DE2, 0xD0DFA,
     "87e72fc7d74a85da4226cb2f853a53fa0504bbe1a712c0133384261017a8281e"),
    ("psg_frequencies", "PSGFreqs", 0xD0FA2, 0xD102E,
     "a1c0729cf3c324276e18224a6e85ec287f8bc5f50ce8fdbcb848aa2dde720ecd"),
    ("fm_algorithm_operator_masks", "FMAlgo_OpMask", 0xD1290, 0xD1298,
     "dd032305bb259d03b1edb3e4d1f8e890e6d2d4d44a50f91b69ec32379b3a9fab"),
    ("fm_operator_registers", "FMInsOperators", 0xD1308, 0xD131C,
     "a1f4bea53ceae82039c34ffa19c96eae11fe84e6c7ebbe237b0b4bc13e49e09d"),
    ("fm_volume_registers", "Volume_Ops", 0xD131C, 0xD1320,
     "92c012d99746d23137cc2bd18be64ba90ca36d4e5729ce270711cd665e783536"),
    ("fm3_frequency_values", "FM3_FreqVals", 0xD1510, 0xD1518,
     "9fc87412072b834e8cdd829657edd046542a1d8f3cb795337bb2582f014c05dd"),
    ("z80_dac_driver", "Z80_DacDriver", 0xD153E, 0xD1A3E,
     "071a6f30f1da6979f893bdd9abce6aea9eb99942e915315e15a4a7cb66dcdf44"),
    ("dac_bank_table", "zBankTbl", 0xD1A3E, 0xD1A60,
     "bb9852472df81a64b856b6caa1115ea90c7e176449eb5898e225f4a2ada49583"),
    ("driver_pointer_block", "loc_D1A60", 0xD1A60, 0xD1A80,
     "b57bdad239202d2f9e72492134bd0858f7097e2bf033a237a10ecff737b40b59"),
    ("modulation_envelope_pointers", "ModEnvPtrs", 0xD1A80, 0xD1AA0,
     "865cd6a5ee14c697b9254ffc553097ba277656fcae2b990856fed7d9a8d895bd"),
    ("volume_envelope_pointers", "VolEnvPtrs", 0xD1B44, 0xD1B6C,
     "6238a04a8b3fdde04a575f89eeac6ba6a83202729d0d0a4dee4f92b7b2135baa"),
    ("sound_priorities", "SndPriorities", 0xD1D10, 0xD1D8E,
     "521c23e6f32d3b5f2d62f88b6b493bb1a554551b3736a2a5be37871cf8bc539e"),
    ("music_pointer_table", "MusicPtrs", 0xD1C40, 0xD1D10,
     "ceab042f0d5b042666bae9440ec3b38d397500c0ff092b80eb471c53a1f64f89"),
    ("sfx_pointer_table", "SFXPtrs", 0xDE4B6, 0xDE5C2,
     "8119d1146debaa111ee47f8c6735cd567ffadaddf7ea73142e923184fd503649"),
    ("special_sfx_pointer_table", "SpcSFXPtrs", 0xDE5C2, 0xDE5CE,
     "e78ab175adbb99d813d210294b2b7357d6f1307d569c295a34654406818083d3"),
)

EXPECTED_SHORT_BYTES = {
    "fm_init_bytes": "06 00 01 02 04 05 06",
    "psg_init_bytes": "80 A0 C0",
    "spc_fm3_registers": "AD A9 AC A8 AE AA A6 A2",
    "fm_algorithm_operator_masks": "08 08 08 08 0A 0E 0E 0F",
    "fm_operator_registers": "30 38 34 3C 50 58 54 5C 60 68 64 6C 70 78 74 7C 80 88 84 8C",
    "fm_volume_registers": "40 48 44 4C",
    "fm3_frequency_values": "00 00 01 80 01 F4 02 60",
}

