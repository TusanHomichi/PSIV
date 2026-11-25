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


# Structural sentinels from docs/sound/SOUND_DRIVER.md "Retail image", pinned
# by `(offset, length, sha256)` (#461) before any pointer is trusted.
ANCHORS = (
    (0xD0008, 16, "48ae0ed7d3a79e8c88e5124cc8ffc69da4fe8b8709afd522ba35951ff4bf342d"),
    (0xD0632, 16, "8f42ab1d80149d8dceb11469ab1721eb167553ef294cb3053321d21bf8de42fc"),
    (0xD1A60, 16, "a7cefee5cc804a226215cc3ce0ac786dc520094336121f2afcbd92a113021055"),
    (0xD1C40, 16, "1210d65f99486bd6b6f727402e1d60cc47e262fa92caa93d32ee38a84b81cc5a"),
    (0xD1D8E, 16, "6fd585b3ccf03a378abe80399c7ace8208f441613740d94dbab25d6e9b5ad744"),
    (0xD233A, 16, "b75f92ed9648ff84eeab57d7aa5146916991aa405d1be729b42f9958743a5b51"),
    (0xDE4B6, 16, "e3e1932092ad0c420f954f1c1194ac9179035237be850c3378d04ce8b4065d8f"),
    (0xDE5CE, 16, "b90a12058363ab898e8daae28fc2c1767e29a6e01e143fde4bc1d15f110f875d"),
    (0xD153E, 16, "469926635c603a634560174ef83525aeab397550b16b7fa97d024dd3fecf62c0"),
    (0xD1A3E, 16, "4c1e95f4e9c521289a646faa14c0c22a56bef497e79484c8bafc82bb76ef4f79"),
    (0xE0000, 16, "534f7e2fc495008f41c38538adf44bd891f79a1481c665b98a6244448dbb6359"),
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

