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
