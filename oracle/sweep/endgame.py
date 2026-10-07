"""The endgame fixtures: forced captures of the abilities after the Air Castle.

    python3 -m oracle.sweep.endgame --list
    python3 -m oracle.sweep.endgame --capture --extract --work build/a6-captures
    python3 -m oracle.sweep.endgame --capture --extract --only lightning

The recipe for `docs/battle/ENEMY_ABILITIES_ENDGAME.md` (lane A6). The set of
abilities the cases exist for is derived, not listed: `python3 -m
oracle.sweep.route_abilities --stretch air-castle-ending`. The machinery is
`oracle.sweep.forced_cases`; `<work>/scout.json` is the attack-policy scout and
`<work>/script-scout.json` the `--prepare-script` one.
"""
from __future__ import annotations

from .forced_cases import FIXTURE_ROOT, ROOT, Case, defend, run_cases

FIXTURES = FIXTURE_ROOT / "endgame"

#: A party that outlasts the bosses and acts after them (agility 1).
_ENDURING = {"hp": 999, "max_hp": 999, "agility": 1, "defence": 999,
             "mental_defence": 999}


def _chaz(target: int, attack: int, rounds: int = 1, then_defend: bool = True) -> dict:
    """Chaz (slot 2) swings at `target` with `attack` for `rounds` rounds while
    Alys and Hahn defend; then all three defend. Dexterity 120 lands the swing."""
    swing = {"2": {"command": "attack", "target": target}, **defend(1, 3)}
    return {"defaults": {"hp": 999, "max_hp": 999, "agility": 1},
            "party": {"2": {"attack": attack, "dexterity": 120}},
            "rounds": [swing] * rounds + ([defend(1, 2, 3)] if then_defend else [])}


def _first_strike(target: int) -> dict:
    """Chaz (slot 2, attack 540, agility 120 - below the `$80` a signed byte
    comparison reads as negative) fells `target` before the enemy side acts;
    a second swing would retarget onto the survivor. Then everyone defends."""
    return {"defaults": {"hp": 999, "max_hp": 999, "agility": 120},
            "party": {"2": {"attack": 540, "dexterity": 120}},
            "rounds": [{"2": {"command": "attack", "target": target}, **defend(1, 3)},
                       defend(1, 2, 3)]}


def _foi(target: int, rounds: int = 1) -> dict:
    """Alys (slot 1) casts FOI (technique 1) at `target`; the others defend."""
    return {"defaults": {"hp": 999, "max_hp": 999, "agility": 1},
            "rounds": [{"1": {"command": "technique", "id": 1, "target": target},
                        **defend(2, 3)}] * rounds + [defend(1, 2, 3)]}

CASES: tuple[Case, ...] = (
    Case("lightning", "LIGHTNING: two Sweeper (Weapon Plant)", 0x05, 5,
         formation=0x151, also=(0x04,)),
    Case("motrcannon", "MOTRCANNON: three Servant (Weapon Plant)", 0x09, 6,
         formation=0x154),
    Case("micromissl_lifedeletr", "MICROMISSL: two LifeDeletr (Vahal Fort)", 0x0E, 6,
         formation=0x1A6),
    Case("micromissl_drager", "MICROMISSL: two DragerDuel (Climate Center)", 0x0E, 6,
         formation=0x13F),
    Case("flamlaunch", "FLAMLAUNCH and MICROMISSL: two JurafaDuel (Vahal Fort)", 0x0F, 6,
         formation=0x1A8, also=(0x0E,)),
    Case("flash", "FLASH: three StarDrone (Weapon Plant)", 0x16, 6, formation=0x158),
    Case("poisonmist_newt", "POISONMIST: three FlameNewt (Island Cave)", 0x24, 6,
         formation=0x199),
    Case("strnglight", "STRNGLIGHT: three Shrieker (Island Cave)", 0x36, 6,
         formation=0x19C),
    Case("twinarms", "BLADESHINE and HAKEN BOLT: two TwinArms", 0x3C, 6,
         formation=0x1C9, also=(0x3D,)),
    Case("soldrfiend", "BLADESHINE and HAKEN BOLT: two SoldrFiend (The Edge)", 0x3C, 6,
         formation=0x1E2, also=(0x3D,)),
    Case("kingsaber", "RAY-SPEAR: three KingSaber", 0x42, 6, formation=0x183),
    Case("darkrider", "RAY-SPEAR and THROWLANCR: three DarkRider (The Edge)", 0x43, 6,
         formation=0x1E5, also=(0x42,)),
    Case("darkwitch", "GIFOI: three DarkWitch", 0x48, 6, formation=0x1CC),
    Case("ghoul", "BAD SMELL: four Ghoul (Garuberk Tower)", 0x50, 6, formation=0x186),
    Case("chaossorcr", "TANDLE: two ChaosSorcr", 0x52, 6, formation=0x171),
    Case("illusionst", "MINDBLST and TANDLE: three Illusionst", 0x51, 6,
         formation=0x1BE, also=(0x52,)),
    Case("imagiomage", "LEGEON: two ImagioMage (The Edge)", 0x55, 6, formation=0x1EA),
    Case("radhin", "SHIFT, SANER, SEALS and DEBAN: four Radhin", 0x26, 6,
         formation=0x189, also=(0x27, 0x29, 0x2D)),
    Case("culabellr", "LGHTBREATH: two CulaBellr (Rykros)", 0x58, 6, formation=0x1C0),
    Case("lefawgan", "TANDIL: three LeFawGan", 0x5D, 6, formation=0x1C3),
    Case("lefawgan_gifoi", "GIFOI: three LeFawGan", 0x48, 8, formation=0x1C3, delay=1,
         script={"defaults": _ENDURING, "rounds": [defend(1, 2, 3)]}),
    Case("gilefarg", "TANDIL: three GiLeFarg (The Edge)", 0x5D, 8, formation=0x1EE),
    Case("railgun", "RAIL-GUN: two GunnerBit (Plate System)", 0x03, 4, formation=0xE8),
    Case("cyanicbomb", "CYANICBOMB: two VopalSphre after the party's swings", 0x1A, 6,
         formation=0x1A9),
    Case("twin_claw_silvalt", "TWIN CLAW: Silvalt at half HP", 0x0A, 6, formation=0x155,
         script=_chaz(6, 150, rounds=6, then_defend=False)),
    Case("twin_claw_goldine", "TWIN CLAW: Goldine at half HP", 0x0A, 6, formation=0x1A3,
         script=_chaz(6, 170, rounds=6, then_defend=False)),
    Case("fission3", "FISSION: FractOoze at half HP becomes four JR.OOZE", 0x1B, 6,
         formation=0x197, script=_chaz(6, 450, rounds=3)),
    Case("supersonic_alone", "SUPERSONIC: the BlindHeads left alone", 0x23, 4,
         formation=0x1DA, script=_first_strike(7)),
    Case("darkmaraud_shift", "DORAN on its ambush, SHIFT at half HP: DarkMaraud", 0x26, 5,
         formation=0x17C, also=(0x28,), script=_chaz(6, 270, rounds=2)),
    # An enemy ambush sets the Ambush reaction bit (loc_B62A, ps4.asm:17456-17463):
    # the delay is the one that opens these battles with it.
    Case("deathbearr", "SEALS after a FOI, then SHIFT alone: DeathBearr", 0x29, 4,
         formation=0x1B8, also=(0x26,), script=_foi(6)),
    Case("chaosbrngr", "RIMIT on its ambush, SEALS after a FOI, SHIFT at half HP: ChaosBrngr",
         0x29, 5, formation=0x1DC, also=(0x26, 0x2A), delay=1, script={
             "defaults": {"hp": 999, "max_hp": 999, "agility": 1},
             "party": {"2": {"attack": 350, "dexterity": 120}},
             "rounds": [{"1": {"command": "technique", "id": 1, "target": 6},
                         "2": {"command": "attack", "target": 6}, **defend(3)},
                        defend(1, 2, 3)]}),
    Case("bloodsaber_shift", "SHIFT: the BloodSaber left alone", 0x26, 4,
         formation=0x1C6, script=_first_strike(7)),
    Case("soldrfiend_gires", "GIRES: SoldrFiend heals itself at half HP", 0x3E, 6,
         formation=0x1E1, script=_chaz(6, 220, rounds=2)),
    # The arm clears `$24(a4)` (ps4.asm:21770), so the log never holds `$41`:
    # the case is checked by the SandWorm the reload seats.
    Case("infantworm", "NOTHING: the InfantWorm left alone becomes a SandWorm", 0x41, 4,
         formation=0x3C, seated=80, script=_chaz(7, 150)),
    Case("darkwitch_gisar", "GISAR: two DarkWitch, one at half HP", 0x49, 5,
         formation=0x1CB, script=_chaz(6, 120, rounds=2)),
    Case("techmaster_sar", "SAR: two TechMaster, one at half HP", 0x46, 5,
         formation=0x110, script=_chaz(6, 70, rounds=2)),
    Case("radhin_gisar", "GISAR: two Radhin, one at half HP", 0x49, 5,
         formation=0x187, script=_chaz(6, 150, rounds=2)),
    Case("dthspell_illusionst", "DTHSPELL: one Illusionst", 0x4E, 8, formation=0x1BC,
         script={"defaults": _ENDURING, "rounds": [defend(1, 2, 3)]}),
    Case("dthspell_imagiomage", "DTHSPELL: one ImagioMage", 0x4E, 8, formation=0x1E9, delay=1,
         script={"defaults": _ENDURING, "rounds": [defend(1, 2, 3)]}),
    Case("spark", "SPARK: Browren486 after a FOI, Wren the one android", 0x1E, 4,
         formation=0x1AC, script={**_foi(6, rounds=3), "characters": [1, 7, 0]}),
    Case("spark_two", "SPARK: Browren486 after a FOI, Wren and Demi", 0x1E, 4,
         formation=0x1AC, script={**_foi(6, rounds=3), "characters": [1, 7, 6]}),
    Case("combine_workerpods", "COMBINE: the ArthroPod and Wiredine left alone", 0x0C, 4,
         formation=0x149, script=_chaz(8, 400)),
    Case("combine_wiredine", "COMBINE: the Wiredine left beside one ArthroPod", 0x0D, 4,
         formation=0x149, script=_chaz(6, 400)),
    # DeathBearr's SANER arm swings (its word compare, ps4.asm:22137-22139); the
    # log files the turn as an attack, so the case is checked by the replay.
    Case("deathbearr_saner", "SANER: DeathBearr at half HP swings instead", None, 6,
         formation=0x1B8, script=_chaz(6, 330, rounds=2)),
    Case("trees", "The carnivorous trees: the latch's first action", None, 3, event=10),
    Case("ryre", "Ryre, the Anger Tower's guardian: its ability-0 swing", None, 3, event=24,
         script={"defaults": _ENDURING, "rounds": [defend(1, 2, 3)]}),
    Case("dark_force_3", "Dark Force 3: SHADOWBIND and MINDBLST", 0x4B, 20, event=18,
         also=(0x51,), script={"defaults": _ENDURING, "rounds": [defend(1, 2, 3)]}),
    Case("de_vars", "De Vars: DISRUPTARM after a FOI", 0x59, 4, event=22,
         script={**_foi(6, rounds=4), "defaults": _ENDURING}),
    Case("sa_lews_tandle", "Sa Lews: TANDLE", 0x52, 10, event=23,
         script={"defaults": _ENDURING, "rounds": [defend(1, 2, 3)]}),
    Case("sa_lews", "Sa Lews: LEGEON after a swing", 0x55, 6, event=23, script={
             "defaults": _ENDURING, "party": {"2": {"dexterity": 120}},
             "rounds": [{"2": {"command": "attack", "target": 6}, **defend(1, 3)}]}),
    Case("re_faze", "Re Faze: MEGID", 0x5E, 3, event=25,
         script={"defaults": _ENDURING, "rounds": [defend(1, 2, 3)]}),
    Case("profound_darkness_slow", "Profound Darkness, slowly: ANOTHRGATE and CANCELING",
         0x61, 60, event=26, also=(0x69,), repeats=6000, script={
             "defaults": {**_ENDURING, "attack": 120, "dexterity": 120},
             "rounds": [{"1": {"command": "attack", "target": -1},
                         "2": {"command": "attack", "target": 6},
                         "3": {"command": "attack", "target": 6}}]}),
    Case("profound_darkness_long", "Profound Darkness: RAY BREATH",
         0x22, 40, event=26, repeats=2400, script={
             "defaults": {**_ENDURING, "attack": 200, "dexterity": 120},
             "rounds": [{"1": {"command": "attack", "target": -1},
                         "2": {"command": "attack", "target": 6},
                         "3": {"command": "attack", "target": 6}}]}),
    Case("profound_darkness", "Profound Darkness: the rise, both form changes, MEGID first",
         0x5E, 16, event=26, also=(0x21, 0x64, 0x65, 0x30, 0x4C), script={
             "defaults": {**_ENDURING, "attack": 500, "dexterity": 120},
             "rounds": [{"1": {"command": "attack", "target": -1},
                         "2": {"command": "attack", "target": 6},
                         "3": {"command": "attack", "target": 6}}]}),
)


def main(argv: list[str] | None = None) -> int:
    return run_cases(CASES, FIXTURES, ROOT / "build" / "a6-captures",
                     __doc__.splitlines()[0], argv)


if __name__ == "__main__":
    raise SystemExit(main())
