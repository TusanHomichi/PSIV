"""Stable symbolic IDs transcribed from the public PSIV disassembly constants.

These are *symbols*, not localized display strings from the ROM text system. Keeping
that distinction explicit lets the extractor expose useful identities now while a
later text-decoder slice can supply the cartridge's actual rendered names.
"""

ITEM_SYMBOLS = """
Dagger HuntKnife Boomerang LthrCloth LthrHelm LthrCrown LthrBand StelSword Slasher LthrShield
CrbnSuit CrbnShield CrbnHelm CrbnCrown Circlet WoodCane WhiteMantl TitnSword TitnDagger TitnSlashr
TitnAxe BroadAxe TitnMail TitnShield TitnHelm TitnCrown Nothing GrptSuit GrptShield CrmcSword
GrptCrown Claw CrmcKnife CrmcShield LasrSlashr SaberClaw StrugglAx CrmcMail CrmcHelm LasrSword
LasrClaw LasrBarrir Impacter TitnArmor Headgear StunShot LasrAxe LasrKnife CrmcArmor TitnGear
PsyMail PsyShield PsyCrown PsyCirclt ForceCane PsyRobe PsycoWand FradeMantl WaveShot SpacedArmr
CrmcGear PlsmRifle PulseLasr PlsmSword PlsmClaw PlsmDagger PlsmField SilvRod SilvMantl SilvCirclt
SilvMail SilvShield SilvHelm SilvCrown ZircGear NapalmShot ZircoArmer FlameSword ThundrClaw TorndDaggr
DreamRod PhantaRobe SilvrTusk PulseVulcn CompoArmr CompoGear RflcMail RflcShield RflcRobe LacoSword
LacoDagger LacoClaw LacoSlashr GuardRod PlsmLaunch ElstArmor ElstGear LacoRod GenocyClaw SwiftHelm
MoonSlashr PowShield LacoMail LacoHelm LacoCrown LacoCirclt LacoShield CyberSuit GuardSword PhotnErasr
LacoArmor LacoGear MahlayDggr GuardClaw GuardArmor GuardRobe GuardMail Nothing2 Elsydeon LacoAxe
SonicBustr DefeatAxe Nothing3 MahlayMail Monomate Dimate Trimate Antidote CureParal MoonDew
StarDew Telepipe Escapipe SolDew GuardShild MahlayShld ShadwBlade AlisSword Dynamite Nothing4
Alshline EclpsTorch AeroPrism RepairKit ShortCake PenguFeed Perolymate Pennant WoodCarvin LandRover
IceDigger HydroFoil ControlKey Canceller PalmaRing MotaRing DezoRing RykrRing AlgoRing MahlayRing
""".split()

ENEMY_SYMBOLS = """
Helex MonsterFly GunnerBit SensorBit ProtectBit ForcedFly Neowhistle Seeker Sweeper Xanafalgue
ZoranBult Gicefalgue Igglanova Guilgenova Locusta Fanbite Grasshound Slave Servant Blauzen
Silvalt Goldine Tarantella ArthroPod WorkerPod Wiredine LifeDeletr BalDuel DragerDuel JurafaDuel
Crawler CarrionCr Caterpillr Blob ZolSlug JrOoze MetaSlug SnowSlug FractOoze Tower
CRayTube ArmDrone SatMinion StarDrone FloatMine CommndBall VopalSphre Warren286 Siren386 Browren486
FloatMine2 Loader Debugger Dominator Whistle Tracer SandNewt Mistralgec FlameNewt StoneHeads
CrminHeads BlindHeads AbeFrog GerotLux DarkMaraud DeathBearr ChaosBrngr Scorpirus Rajago BiterFly
ShadowSabr FrostSaber BloodSaber DimensWorm OuterBeast FlattrPlnt FlyScreamr TechPlant ToadStool Shrieker
SandWorm DesrtLeach Leviathan Ripper BladeRight Piercer HakenLeft TwinArms SoldrFiend Ismounos
Depcen HewGilla Elmelew MiniWorm InfantWorm SnowWorm Centaur KingSaber DarkRider TechUser
TechMaster DarkWitch Speard ZiosGuard Acacia ShadMirage Haunt Spector Phantom Zombie
Ghoul ChaosSorcr Illusionst ImagioMage Juza Greneris Radhin GyLaguiah LwAddmer CulaBellr
DeVars SaLews DElmLars XeAThoul LeFawGan GiLeFarg Ryre ReFaze Lashiec CarnivorousTree
DarkForce1 DarkForce2 DarkForce3 ProfoundDarkness1 ProfoundDarkness2 ProfoundDarkness3 SandWorm2 FractOoze2 ChaosSorcr2 Zio
Zio2 DezoOwl Skytiara Owltalon SnowMole RedMole HungryMole Rappy BlueRappy KingRappy
InfantWorm2 Prophallus Zio3
""".split()

assert len(ITEM_SYMBOLS) == 160, len(ITEM_SYMBOLS)
assert len(ENEMY_SYMBOLS) == 153, len(ENEMY_SYMBOLS)

ENEMY_SKILL_SYMBOLS = """
Nothing FlameBolt RailGun LasrCannon Lightning Fission Fission2 SpiralBld MotrCannon TwinClaw
StasisBall Combine Combine2 MicroMissl FlamLaunch Thread Poison Fusion CellSplit Warning
ChargCnnon Flash Waiting Explosion Detonation CyanicBomb Fission3 FlareShot Barrier Spark
DblSlash PhononMasr FireBreath RayBreath SuperSonic PoisonMist SleepGas Shift Saner Doran
Seals Rimit Needle Airslash Deban Giwat Vol Distortion Gra Gigra
AcidBreath Voice Gizan StrngLight SandStorm Earthquake Maelstrom Combine3 Combine4 BladeShine
HakenBolt Gires FlodBreath Wat Nothing2 RaySpear ThrowLancr Foi Res Sar
Zan Gifoi Gisar StarDust ShadowBind EvilEye Corrsion DthSpell Hewn BadSmell
MindBlst Tandle Nightmare BlackWave Legeon ForceFlash Gelun LghtBreath DisruptArm Flaeli
Bindwa ThndrBlast Tandil Megid ThndHalbrt Posession AnothrGate Reinforce Burstroc ShdwBreath
LightShowr DestrocRay Nothing3 Nothing4 Canceling WindStorm MagBarrir BlackWave2 RoundEyes LovelEyes
Casting BlackWave3
""".split()

assert len(ENEMY_SKILL_SYMBOLS) == 112, len(ENEMY_SKILL_SYMBOLS)

# Map ids are 0-based indexes into the PtrMap_* pointer table; names are the
# MapID_* constants from ps4.constants.asm (417 entries, contiguous).
MAP_SYMBOLS = [
    "Motavia",  # 0x000
    "Dezolis",  # 0x001
    "Rykros",  # 0x002
    "Null3",  # 0x003
    "Null4",  # 0x004
    "Null5",  # 0x005
    "Null6",  # 0x006
    "Null7",  # 0x007
    "Null8",  # 0x008
    "Null9",  # 0x009
    "NullA",  # 0x00A
    "NullB",  # 0x00B
    "NullC",  # 0x00C
    "NullD",  # 0x00D
    "NullE",  # 0x00E
    "NullF",  # 0x00F
    "Piata",  # 0x010
    "PiataAcademy",  # 0x011
    "PiataAcademyNearBasement",  # 0x012
    "PiataAcademy_F1",  # 0x013
    "AcademyPrincipalOffice",  # 0x014
    "AcademyBasement",  # 0x015
    "AcademyBasement_B1",  # 0x016
    "AcademyBasement_B2",  # 0x017
    "PiataDorm",  # 0x018
    "PiataInn",  # 0x019
    "PiataHouse1",  # 0x01A
    "PiataItemShop",  # 0x01B
    "PiataHouse2",  # 0x01C
    "Mile",  # 0x01D
    "MileDead",  # 0x01E
    "MileWeaponShop",  # 0x01F
    "MileHouse1",  # 0x020
    "MileItemShop",  # 0x021
    "MileHouse2",  # 0x022
    "MileInn",  # 0x023
    "Zema",  # 0x024
    "ZemaHouse1",  # 0x025
    "ZemaWeaponShop",  # 0x026
    "ZemaInn",  # 0x027
    "ZemaHouse2",  # 0x028
    "ZemaHouse2_B1",  # 0x029
    "ZemaItemShop",  # 0x02A
    "BirthValley",  # 0x02B
    "BirthValley_B1",  # 0x02C
    "ValleyMazeUnused",  # 0x02D
    "Null2E",  # 0x02E
    "Null2F",  # 0x02F
    "Null30",  # 0x030
    "Null31",  # 0x031
    "Null32",  # 0x032
    "Null33",  # 0x033
    "Null34",  # 0x034
    "Null35",  # 0x035
    "Null36",  # 0x036
    "Null37",  # 0x037
    "Null38",  # 0x038
    "Krup",  # 0x039
    "KrupKindergarten",  # 0x03A
    "KrupWeaponShop",  # 0x03B
    "KrupItemShop",  # 0x03C
    "KrupHouse",  # 0x03D
    "KrupInn",  # 0x03E
    "KrupInn_F1",  # 0x03F
    "Molcum",  # 0x040
    "Tonoe",  # 0x041
    "TonoeStorageRoom",  # 0x042
    "TonoeGryzHouse",  # 0x043
    "TonoeHouse1",  # 0x044
    "TonoeHouse2",  # 0x045
    "TonoeInn",  # 0x046
    "TonoeBasement",  # 0x047
    "TonoeBasement_B1",  # 0x048
    "TonoeBasement_B2",  # 0x049
    "TonoeBasement_B3",  # 0x04A
    "Nalya",  # 0x04B
    "NalyaHouse1",  # 0x04C
    "NalyaHouse2",  # 0x04D
    "NalyaItemShop",  # 0x04E
    "NalyaHouse3",  # 0x04F
    "NalyaHouse4",  # 0x050
    "NalyaHouse5",  # 0x051
    "NalyaInn",  # 0x052
    "NalyaInn_F1",  # 0x053
    "Aiedo",  # 0x054
    "AiedoBakery",  # 0x055
    "AiedoBakery_B1",  # 0x056
    "HuntersGuild",  # 0x057
    "HuntersGuildStorage",  # 0x058
    "StripClubDressingRoom",  # 0x059
    "StripClub",  # 0x05A
    "AiedoWeaponShop",  # 0x05B
    "AiedoPrison",  # 0x05C
    "AiedoHouse1",  # 0x05D
    "ChazHouse",  # 0x05E
    "AiedoHouse2",  # 0x05F
    "AiedoHouse3",  # 0x060
    "AiedoHouse4",  # 0x061
    "AiedoHouse5",  # 0x062
    "AiedoSupermarket",  # 0x063
    "AiedoPub",  # 0x064
    "RockyHouse",  # 0x065
    "AiedoHouse6",  # 0x066
    "AiedoHouse7",  # 0x067
    "Kadary",  # 0x068
    "KadaryChurch",  # 0x069
    "KadaryPub",  # 0x06A
    "KadaryPub_F1",  # 0x06B
    "KadaryStorageRoom",  # 0x06C
    "KadaryHouse1",  # 0x06D
    "KadaryHouse2",  # 0x06E
    "KadaryHouse3",  # 0x06F
    "KadaryItemShop",  # 0x070
    "KadaryInn",  # 0x071
    "KadaryInn_F1",  # 0x072
    "Monsen",  # 0x073
    "MonsenInn",  # 0x074
    "MonsenHouse1",  # 0x075
    "MonsenHouse2",  # 0x076
    "MonsenHouse3",  # 0x077
    "MonsenHouse4",  # 0x078
    "MonsenHouse5",  # 0x079
    "MonsenItemShop",  # 0x07A
    "Termi",  # 0x07B
    "TermiItemShop",  # 0x07C
    "TermiHouse1",  # 0x07D
    "TermiWeaponShop",  # 0x07E
    "TermiInn",  # 0x07F
    "TermiHouse2",  # 0x080
    "Passageway",  # 0x081
    "ZioFort",  # 0x082
    "ZioFort_Part2",  # 0x083
    "ZioFort_F1",  # 0x084
    "ZioFort_F2West",  # 0x085
    "ZioFortWestTunnel",  # 0x086
    "ZioFortJuzaRoom",  # 0x087
    "ZioFortEastTunnel",  # 0x088
    "ZioFort_F2East",  # 0x089
    "ZioFort_F3",  # 0x08A
    "ZioFort_F4",  # 0x08B
    "LadeaTower",  # 0x08C
    "LadeaTower_F1",  # 0x08D
    "LadeaTower_F2",  # 0x08E
    "LadeaTower_F3",  # 0x08F
    "LadeaTower_F4",  # 0x090
    "LadeaTower_F5",  # 0x091
    "IslandCave",  # 0x092
    "IslandCave_F1",  # 0x093
    "IslandCave_F1_Part2",  # 0x094
    "IslandCave_Part2",  # 0x095
    "IslandCave_B1",  # 0x096
    "IslandCave_F2",  # 0x097
    "IslandCave_F3",  # 0x098
    "SoldiersTempleOutside",  # 0x099
    "SoldiersTemple",  # 0x09A
    "ValleyMaze",  # 0x09B
    "ValleyMaze_Part2",  # 0x09C
    "ValleyMaze_Part3",  # 0x09D
    "ValleyMaze_Part4",  # 0x09E
    "ValleyMaze_Part5",  # 0x09F
    "ValleyMaze_Part6",  # 0x0A0
    "ValleyMaze_Part7",  # 0x0A1
    "BioPlant",  # 0x0A2
    "BioPlant_Part2",  # 0x0A3
    "BioPlant_Part3",  # 0x0A4
    "NullA5",  # 0x0A5
    "BioPlant_B1",  # 0x0A6
    "BioPlant_B2",  # 0x0A7
    "BioPlant_B2_Part2",  # 0x0A8
    "BioPlant_B3",  # 0x0A9
    "BioPlant_B3_Part2",  # 0x0AA
    "BioPlant_B4",  # 0x0AB
    "BioPlant_B4_Part2",  # 0x0AC
    "BioPlant_B4_Part3",  # 0x0AD
    "Wreckage",  # 0x0AE
    "Wreckage_Part2",  # 0x0AF
    "Wreckage_Part3",  # 0x0B0
    "Wreckage_F1",  # 0x0B1
    "Wreckage_F1_Part2",  # 0x0B2
    "Wreckage_F2",  # 0x0B3
    "Wreckage_F2_Part2",  # 0x0B4
    "Wreckage_F2_Part3",  # 0x0B5
    "Wreckage_F2_Part4",  # 0x0B6
    "MachineCenter",  # 0x0B7
    "MachineCenter_B1",  # 0x0B8
    "MachineCenter_B1_Part2",  # 0x0B9
    "PlateSystem",  # 0x0BA
    "PlateSystem_F1",  # 0x0BB
    "PlateSystem_F2",  # 0x0BC
    "PlateSystem_F3",  # 0x0BD
    "PlateSystem_F4",  # 0x0BE
    "MotaSpaceport",  # 0x0BF
    "ClimCenter",  # 0x0C0
    "ClimCenter_F1",  # 0x0C1
    "ClimCenter_F2",  # 0x0C2
    "ClimCenter_F3",  # 0x0C3
    "WeaponPlant",  # 0x0C4
    "WeaponPlant_F1",  # 0x0C5
    "WeaponPlant_F2",  # 0x0C6
    "WeaponPlant_F3",  # 0x0C7
    "VahalFort",  # 0x0C8
    "VahalFort_F1",  # 0x0C9
    "VahalFort_F2",  # 0x0CA
    "VahalFort_F3",  # 0x0CB
    "Nurvus_Part2",  # 0x0CC
    "Nurvus_Part3",  # 0x0CD
    "Nurvus_B1",  # 0x0CE
    "Nurvus_B2",  # 0x0CF
    "Nurvus_B3",  # 0x0D0
    "Nurvus_B1Tunnel",  # 0x0D1
    "Nurvus_B4",  # 0x0D2
    "Nurvus_B4_Part2",  # 0x0D3
    "DezoSpaceport",  # 0x0D4
    "Nurvus_B5",  # 0x0D5
    "Nurvus_B3Tunnel",  # 0x0D6
    "Nurvus",  # 0x0D7
    "ValleyMazeOutside",  # 0x0D8
    "ValleyMazeOutside2",  # 0x0D9
    "PassagewayNearAiedo",  # 0x0DA
    "PassagewayNearKadary",  # 0x0DB
    "NullDC",  # 0x0DC
    "NullDD",  # 0x0DD
    "NullDE",  # 0x0DE
    "NullDF",  # 0x0DF
    "Uzo",  # 0x0E0
    "UzoHouse1",  # 0x0E1
    "UzoHouse2",  # 0x0E2
    "UzoInn",  # 0x0E3
    "UzoHouse3",  # 0x0E4
    "UzoItemShop",  # 0x0E5
    "Torinco",  # 0x0E6
    "CulversHouse",  # 0x0E7
    "TorincoHouse1",  # 0x0E8
    "TorincoHouse2",  # 0x0E9
    "TorincoItemShop",  # 0x0EA
    "TorincoInn",  # 0x0EB
    "MonsenCave",  # 0x0EC
    "RappyCave",  # 0x0ED
    "NullEE",  # 0x0EE
    "NullEF",  # 0x0EF
    "LeRoofRoom",  # 0x0F0
    "SilenceTm",  # 0x0F1
    "StrengthTower",  # 0x0F2
    "StrengthTower_F1",  # 0x0F3
    "StrengthTower_F2",  # 0x0F4
    "StrengthTower_F3",  # 0x0F5
    "StrengthTower_F4",  # 0x0F6
    "CourageTower",  # 0x0F7
    "CourageTower_F1",  # 0x0F8
    "CourageTower_F2",  # 0x0F9
    "CourageTower_F3",  # 0x0FA
    "CourageTower_F4",  # 0x0FB
    "AngerTower",  # 0x0FC
    "AngerTower_F1",  # 0x0FD
    "AngerTower_F2",  # 0x0FE
    "NullFF",  # 0x0FF
    "TheEdge",  # 0x100
    "TheEdge_Part2",  # 0x101
    "TheEdge_Part3",  # 0x102
    "TheEdge_Part4",  # 0x103
    "TheEdge_Part5",  # 0x104
    "TheEdge_Part6",  # 0x105
    "TheEdge_Part7",  # 0x106
    "TheEdge_Part8",  # 0x107
    "TheEdge_Part9",  # 0x108
    "Null109",  # 0x109
    "Null10A",  # 0x10A
    "Null10B",  # 0x10B
    "Null10C",  # 0x10C
    "Null10D",  # 0x10D
    "Null10E",  # 0x10E
    "Null10F",  # 0x10F
    "Null110",  # 0x110
    "Null111",  # 0x111
    "Null112",  # 0x112
    "Null113",  # 0x113
    "Null114",  # 0x114
    "Null115",  # 0x115
    "Null116",  # 0x116
    "Null117",  # 0x117
    "Null118",  # 0x118
    "Null119",  # 0x119
    "Null11A",  # 0x11A
    "Null11B",  # 0x11B
    "Null11C",  # 0x11C
    "Null11D",  # 0x11D
    "Null11E",  # 0x11E
    "Null11F",  # 0x11F
    "Tyler",  # 0x120
    "TylerHouse1",  # 0x121
    "TylerWeaponShop",  # 0x122
    "TylerItemShop",  # 0x123
    "TylerHouse2",  # 0x124
    "TylerInn",  # 0x125
    "Zosa",  # 0x126
    "ZosaHouse1",  # 0x127
    "ZosaHouse2",  # 0x128
    "ZosaWeaponShop",  # 0x129
    "ZosaItemShop",  # 0x12A
    "ZosaInn",  # 0x12B
    "ZosaHouse3",  # 0x12C
    "Meese",  # 0x12D
    "MeeseHouse1",  # 0x12E
    "MeeseItemShop2",  # 0x12F
    "MeeseItemShop1",  # 0x130
    "MeeseWeaponShop",  # 0x131
    "MeeseInn",  # 0x132
    "MeeseClinic",  # 0x133
    "MeeseClinic_F1",  # 0x134
    "Null135",  # 0x135
    "Jut",  # 0x136
    "JutHouse1",  # 0x137
    "JutHouse2",  # 0x138
    "JutHouse3",  # 0x139
    "JutHouse4",  # 0x13A
    "JutHouse5",  # 0x13B
    "JutWeaponShop",  # 0x13C
    "JutItemShop",  # 0x13D
    "JutHouse6",  # 0x13E
    "JutHouse6_F1",  # 0x13F
    "JutHouse7",  # 0x140
    "JutHouse8",  # 0x141
    "JutInn",  # 0x142
    "JutChurch",  # 0x143
    "Ryuon",  # 0x144
    "RyuonItemShop",  # 0x145
    "RyuonWeaponShop",  # 0x146
    "RyuonHouse1",  # 0x147
    "RyuonHouse2",  # 0x148
    "RyuonHouse3",  # 0x149
    "RyuonPub",  # 0x14A
    "RyuonInn",  # 0x14B
    "RajaTemple",  # 0x14C
    "Reshel1",  # 0x14D
    "Reshel2",  # 0x14E
    "Reshel3",  # 0x14F
    "Reshel2House",  # 0x150
    "Reshel2WeaponShop",  # 0x151
    "Reshel3House1",  # 0x152
    "Reshel3ItemShop",  # 0x153
    "Reshel3House2",  # 0x154
    "Reshel3WeaponShop",  # 0x155
    "Reshel3Inn",  # 0x156
    "Reshel3House3",  # 0x157
    "MystVale",  # 0x158
    "MystVale_Part2",  # 0x159
    "MystVale_Part3",  # 0x15A
    "MystVale_Part4",  # 0x15B
    "MystVale_Part5",  # 0x15C
    "ElsydeonCave",  # 0x15D
    "ElsydeonCave_B1",  # 0x15E
    "Hangar",  # 0x15F
    "GumbiousEntrance",  # 0x160
    "Gumbious",  # 0x161
    "Gumbious_F1",  # 0x162
    "Gumbious_B1",  # 0x163
    "Gumbious_B2",  # 0x164
    "Gumbious_B2_Part2",  # 0x165
    "EspMansionEntrance",  # 0x166
    "EspMansion",  # 0x167
    "EspMansionWestRoom",  # 0x168
    "EspMansionEastRoom",  # 0x169
    "EspMansionNorth",  # 0x16A
    "EspMansionNorthEastRoom",  # 0x16B
    "EspMansionNorthWestRoom",  # 0x16C
    "EspMansionCourtyard",  # 0x16D
    "InnerSanctuary",  # 0x16E
    "InnerSanctuary_B1",  # 0x16F
    "AirCastle_Part6",  # 0x170
    "AirCastle",  # 0x171
    "AirCastle_Part2",  # 0x172
    "AirCastle_Part3",  # 0x173
    "AirCastle_Part4",  # 0x174
    "AirCastle_Part5",  # 0x175
    "AirCastle_F1_Part9",  # 0x176
    "AirCastle_F1_Part5",  # 0x177
    "AirCastle_F1_Part2",  # 0x178
    "AirCastle_F1_Part10",  # 0x179
    "AirCastleInner",  # 0x17A
    "AirCastle_F1_Part11",  # 0x17B
    "AirCastle_F1_Part12",  # 0x17C
    "AirCastle_F1_Part13",  # 0x17D
    "AirCastle_Part8",  # 0x17E
    "AirCastle_Part7",  # 0x17F
    "AirCastle_F1_Part4",  # 0x180
    "AirCastle_F1",  # 0x181
    "AirCastle_F1_Part3",  # 0x182
    "AirCastle_F2",  # 0x183
    "AirCastleXeAThoulRoom",  # 0x184
    "AirCastleInner_B1",  # 0x185
    "AirCastleInner_B1_Part2",  # 0x186
    "AirCastleInner_B1_Part3",  # 0x187
    "AirCastleInner_B2",  # 0x188
    "AirCastleInner_B3",  # 0x189
    "AirCastleInner_B4",  # 0x18A
    "AirCastleInner_B5",  # 0x18B
    "ZelanSpace",  # 0x18C
    "Zelan",  # 0x18D
    "Zelan_F1",  # 0x18E
    "KuranSpace",  # 0x18F
    "Kuran",  # 0x190
    "Kuran_F1",  # 0x191
    "Kuran_F2",  # 0x192
    "Kuran_F1_Part2",  # 0x193
    "Kuran_F1_Part3",  # 0x194
    "Kuran_F1_Part5",  # 0x195
    "Kuran_F2_Part2",  # 0x196
    "Kuran_F1_Part4",  # 0x197
    "Kuran_F3",  # 0x198
    "GaruberkTower",  # 0x199
    "GaruberkTower_Part2",  # 0x19A
    "GaruberkTower_Part3",  # 0x19B
    "GaruberkTower_Part4",  # 0x19C
    "GaruberkTower_Part5",  # 0x19D
    "GaruberkTower_Part6",  # 0x19E
    "GaruberkTower_Part7",  # 0x19F
    "AirCastleSpace",  # 0x1A0
]

# MusicID_* from ps4.constants.asm, contiguous from $81.
MUSIC_ID_BASE = 0x81
MUSIC_SYMBOLS = """
TonoeDePon Inn MotabiaVillage MotabiaTown OrganicBeat DezorisTown1 NowOnSale
BehindTheCircuit MachineCenter InTheCave Winners FieldMotabia LandMaster
RequiemForLutz MeetThemHeadOn RyucrossField DungeonArrange1 Fal TempleNgangbius
Thray DefeatAtABlow CyberneticCarnival TerribleSight EdgeOfDarkness
DezorisField1 Tower TakeOffLandeel DezorisTown2 DezorisField2 AHappySettlement
Suspicion TheKingOfTerrors TheAgeOfFables Abyss EnemyAppearance HerLastBreath
Pain JijyNoRag DungeonArrange2Cont TheBlackBlood RedAlert Laughter Mystery
EndOfTheMillennium Explosion StaffRoll ThePromisingFuture1 PaoPao
DungeonArrange2 ThePromisingFuture2 DezorisDeDon Ooze
""".split()

# FieldObjectsJmpTbl, 222 `bra.w` entries. An object record's id word is the
# *byte* offset into that table (`andi.w #$7FFC, d0` before the jump), so the
# symbol for id N is entry N/4.
FIELD_OBJECT_SYMBOLS = """
None Chaz Alys Hahn Rune Gryz Rika Demi Wren Raja Kyra Seth ScrollTextArrow
RedCursor NPCType1 NPCType2 NPCType3 NPCType4 NPCType5 NPCType6 NPCType7
NPCType8 NPCType9 NPCType10 NPCType11 NPCType12 NPCAlysPiata
NPCHahnNearBasement NPCRune InvisibleBlock DividingSandOrSnow
LiftingSandOrSnow loc_4D1D2 PlaceFadeIn NPCType13 NPCType14 Statue loc_4AF38
CaveWallPiece Penguin TreasureChest Fire loc_4B0D6 loc_4B16A LandRover
IceDigger Hydrofoil loc_489D6 NPCType16 NPCType17 NPCType18 NPCType19
NPCType20 NPCType21 NPCType22 NPCType23 NPCType24 NPCType25 NPCType26 NPCHahn
NPCGryz NPCType27 NPCType28 NPCType29 NPCType30 NPCType31 NPCType32 NPCType33
NPCType34 Prisoner NPCType35 NPCType36 Elevator loc_4D2D0 loc_48F36 loc_48F96
loc_48FF4 Pana loc_490B8 loc_49128 loc_49502 loc_49192 loc_49212 loc_49542
Dust BigFire FireplaceFire EclipseTorch MileSandWorm loc_4B4B4 Rocky Mouse
Butterfly BigDuck SmallWhiteDuck SmallBrownDuck FaintedPriest Xanafalgue
Igglanova ProfHoltPetrified NPCRuneSequence NPCScriptMove loc_483DC loc_469D4
SayaStars NPCAlysTonoe AngerLines DorinPunched DorinChair Juza Landale
LandaleWings LandaleRearWings LandalePropulsionJets LandaleBeam LandingLandale
LandingLandaleJets WhiteTreasureChest BarrierBeam1 BarrierBeam2 BarrierBeam3
BarrierBeam4 Zio ZioBeam NPCWren Snow ChestBarrier ChestBarrierSplinter
Spaceship PropulsiveJet LandingSpaceship GyLaguiah World NPCRuneFlaeli Flaeli
BlastedRock ExplDust GravestoneHalf MuskCat MuskCatGuardMoved LyingDownMuskCat
MuskCatChiefTopHalf MuskCatChiefBottomHalf MuskCatGuard FellowPenguin
loc_496C6 loc_49746 loc_497A8 loc_4980A NPCKyra loc_4986C Barrier loc_498CA
NPCRika EsperGuard InnerEsperGuards FractOoze DElmLars DarkForce1 DarkForce2
XeAThoulAppearing XeAThoul XeAThoulDisappearing LightCircle LightCircle2
XeAThoulMoving NPCAlysInBed XeAThoulAirCastle DeVars DeVarsFire GiLeFarg
GiLeFargTandil SaLews SaLewsRay Blindheads BlindheadsRay ReFaze ZemaRocks
NPCRajaSpaceport NPCKyraSpaceport NPCGryzSpaceport NPCHahnSpaceport
NPCDemiSpaceport GryzSpaceportWaiting HahnSpaceportWaiting DemiSpaceportWaiting
RajaSpaceportWaiting KyraSpaceportWaiting AlysAngerTower StrayRocky loc_4BC80
Tallas TonoeBasementDoor TrappingRopes DemiTrapped PrisonDoor TallasShoes
loc_4BDF0 loc_4BE38 loc_4BE80 RajaInBed StudentInBed KingRappy
KingRappyFlyingAway Tinkerbell ChazAlisSword loc_4FAE0 Lashiec LutzMirror
Stripper StripperCoat StripClubCustomer loc_49406 loc_49442 SaveSlotCursor
DancingStripper1 DancingStripper2 DancingStripper3 loc_4B30C Pennant
SandWormCarving loc_4FA30
""".split()
