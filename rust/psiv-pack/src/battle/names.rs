//! The name tables `psiv_tools/core.py` and `psiv_tools/symbols.py` attach to
//! battle records: the extractor's names, the disassembly's symbols, and the
//! byte meanings. They are names for ids, not cartridge content. The
//! cartridge's own display names come from [`display_names`].

use std::collections::HashMap;

use crate::PackError;

/// `CHARACTER_NAMES`.
pub const CHARACTERS: [&str; 11] = [
    "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika", "Demi", "Wren", "Raja", "Kyra", "Seth",
];

/// `TECHNIQUE_NAMES`, by technique id minus one.
pub const TECHNIQUES: [&str; 40] = [
    "Foi", "Gifoi", "Nafoi", "Wat", "Giwat", "Nawat", "Tsu", "Githu", "Nathu", "Zan", "Gizan",
    "Nazan", "Gra", "Gigra", "Nagra", "Megid", "Brose", "Vol", "Savol", "Gelun", "Doran", "Seals",
    "Rimit", "Res", "Gires", "Nares", "Sar", "Gisar", "Nasar", "Shift", "Saner", "Deban", "Feeve",
    "Anti", "Rimpa", "Rever", "Regen", "Arows", "Ryuka", "Hinas",
];

/// `SKILL_NAMES`, by skill id minus one.
pub const SKILLS: [&str; 54] = [
    "Crosscut",
    "Rayblade",
    "DblSlash",
    "Flaeli",
    "Flare",
    "Vortex",
    "Astral",
    "Airslash",
    "Disrupt",
    "Hewn",
    "Tandle",
    "Efess",
    "Legeon",
    "Burstroc",
    "Posibolt",
    "Sweeping",
    "Phonon",
    "StFire",
    "Corrsion",
    "Explode",
    "Eliminat",
    "Diem",
    "Spark",
    "Death",
    "Holyword",
    "Dthspell",
    "Negatis",
    "Illusion",
    "Telele",
    "Shadow",
    "Earth",
    "Hijammer",
    "Moonshad",
    "Crash",
    "StasisBm",
    "Bindwa",
    "Mindblst",
    "Barrier",
    "WarCry",
    "Blessing",
    "Warla",
    "Recover",
    "Medice",
    "Miracle",
    "MedicPw",
    "Ataraxia",
    "Vision",
    "Eliminat2",
    "Flaeli2",
    "Hewn2",
    "Tandle2",
    "Spark2",
    "Barrier2",
    "Recover2",
];

/// `ENEMY_SYMBOLS`, by enemy id; whitespace-separated, as in `symbols.py`.
const ENEMY_SYMBOL_TEXT: &str = "
Helex MonsterFly GunnerBit SensorBit ProtectBit ForcedFly Neowhistle Seeker Sweeper
Xanafalgue ZoranBult Gicefalgue Igglanova Guilgenova Locusta Fanbite Grasshound Slave
Servant Blauzen Silvalt Goldine Tarantella ArthroPod WorkerPod Wiredine LifeDeletr BalDuel
DragerDuel JurafaDuel Crawler CarrionCr Caterpillr Blob ZolSlug JrOoze MetaSlug SnowSlug
FractOoze Tower CRayTube ArmDrone SatMinion StarDrone FloatMine CommndBall VopalSphre
Warren286 Siren386 Browren486 FloatMine2 Loader Debugger Dominator Whistle Tracer SandNewt
Mistralgec FlameNewt StoneHeads CrminHeads BlindHeads AbeFrog GerotLux DarkMaraud
DeathBearr ChaosBrngr Scorpirus Rajago BiterFly ShadowSabr FrostSaber BloodSaber
DimensWorm OuterBeast FlattrPlnt FlyScreamr TechPlant ToadStool Shrieker SandWorm
DesrtLeach Leviathan Ripper BladeRight Piercer HakenLeft TwinArms SoldrFiend Ismounos
Depcen HewGilla Elmelew MiniWorm InfantWorm SnowWorm Centaur KingSaber DarkRider TechUser
TechMaster DarkWitch Speard ZiosGuard Acacia ShadMirage Haunt Spector Phantom Zombie Ghoul
ChaosSorcr Illusionst ImagioMage Juza Greneris Radhin GyLaguiah LwAddmer CulaBellr DeVars
SaLews DElmLars XeAThoul LeFawGan GiLeFarg Ryre ReFaze Lashiec CarnivorousTree DarkForce1
DarkForce2 DarkForce3 ProfoundDarkness1 ProfoundDarkness2 ProfoundDarkness3 SandWorm2
FractOoze2 ChaosSorcr2 Zio Zio2 DezoOwl Skytiara Owltalon SnowMole RedMole HungryMole
Rappy BlueRappy KingRappy InfantWorm2 Prophallus Zio3
";
const ENEMY_COUNT: usize = 153;

/// `ENEMY_SKILL_SYMBOLS`, by enemy skill id minus one.
const ENEMY_SKILL_SYMBOL_TEXT: &str = "
Nothing FlameBolt RailGun LasrCannon Lightning Fission Fission2 SpiralBld MotrCannon
TwinClaw StasisBall Combine Combine2 MicroMissl FlamLaunch Thread Poison Fusion CellSplit
Warning ChargCnnon Flash Waiting Explosion Detonation CyanicBomb Fission3 FlareShot
Barrier Spark DblSlash PhononMasr FireBreath RayBreath SuperSonic PoisonMist SleepGas
Shift Saner Doran Seals Rimit Needle Airslash Deban Giwat Vol Distortion Gra Gigra
AcidBreath Voice Gizan StrngLight SandStorm Earthquake Maelstrom Combine3 Combine4
BladeShine HakenBolt Gires FlodBreath Wat Nothing2 RaySpear ThrowLancr Foi Res Sar Zan
Gifoi Gisar StarDust ShadowBind EvilEye Corrsion DthSpell Hewn BadSmell MindBlst Tandle
Nightmare BlackWave Legeon ForceFlash Gelun LghtBreath DisruptArm Flaeli Bindwa ThndrBlast
Tandil Megid ThndHalbrt Posession AnothrGate Reinforce Burstroc ShdwBreath LightShowr
DestrocRay Nothing3 Nothing4 Canceling WindStorm MagBarrir BlackWave2 RoundEyes LovelEyes
Casting BlackWave3
";
const ENEMY_SKILL_COUNT: usize = 112;

fn split(text: &'static str, count: usize, what: &str) -> Vec<&'static str> {
    let symbols: Vec<&str> = text.split_whitespace().collect();
    assert_eq!(symbols.len(), count, "{what}");
    symbols
}

/// `ENEMY_SYMBOLS`.
pub fn enemy_symbols() -> Vec<&'static str> {
    split(ENEMY_SYMBOL_TEXT, ENEMY_COUNT, "ENEMY_SYMBOLS")
}

/// `ENEMY_SKILL_SYMBOLS`.
pub fn enemy_skill_symbols() -> Vec<&'static str> {
    split(
        ENEMY_SKILL_SYMBOL_TEXT,
        ENEMY_SKILL_COUNT,
        "ENEMY_SKILL_SYMBOLS",
    )
}

/// `PROFESSIONS`, by profession id.
pub const PROFESSIONS: [&str; 8] = [
    "Hunter", "Scholar", "Wizard", "Motavian", "Numan", "Android", "Priest", "Esper",
];

/// `ITEM_TYPES`, by type byte.
pub const ITEM_TYPES: [&str; 11] = [
    "none_or_unused",
    "one_handed_single_target_weapon",
    "one_handed_multi_target_weapon",
    "two_handed_single_target_weapon",
    "two_handed_multi_target_weapon",
    "shield",
    "headwear_or_ring",
    "body_equipment",
    "disposable_item",
    "plot_item",
    "field_only_item",
];

/// `PROPERTY_NAMES`: the fourteen property slots, in record order.
pub const PROPERTIES: [&str; 14] = [
    "physical",
    "energy",
    "fire",
    "gravity",
    "water",
    "anti_evil",
    "electric",
    "holyword",
    "brose",
    "biological",
    "psychic",
    "mechanical",
    "efess",
    "destroy",
];

/// `PROPERTY_LEVELS`.
pub fn property_level(value: u8) -> Option<&'static str> {
    ["immune", "resistant", "normal", "weak", "very_weak"]
        .get(usize::from(value))
        .copied()
}

/// `BASIC_ATTACK_STATUS`.
pub fn basic_attack_status(value: u8) -> Option<&'static str> {
    match value {
        0x00 => Some("none"),
        0x1B => Some("poison"),
        0x1C => Some("paralyze"),
        _ => None,
    }
}

/// `TARGETS`.
pub fn target(value: u8) -> Option<&'static str> {
    [
        "single_enemy",
        "all_enemies",
        "self",
        "single_human",
        "all_humans",
        "single_android",
        "all_androids",
        "single_character",
        "all_characters",
    ]
    .get(usize::from(value).wrapping_sub(1))
    .copied()
}

/// `USABILITY`.
pub fn usability(value: u8) -> Option<&'static str> {
    ["battle_only", "field_only", "battle_and_field"]
        .get(usize::from(value).wrapping_sub(1))
        .copied()
}

/// `STATS`.
pub fn stat(value: u8) -> Option<&'static str> {
    [
        "none",
        "strength",
        "mental",
        "agility",
        "dexterity",
        "attack",
        "defense",
        "magic_defense",
    ]
    .get(usize::from(value))
    .copied()
}

/// `ELEMENTS`.
pub fn element(value: u8) -> Option<&'static str> {
    [
        "none",
        "physical",
        "energy",
        "fire",
        "gravity",
        "water_ice",
        "anti_evil",
        "electric",
        "holyword",
        "brose",
        "biological",
        "psychic",
        "mechanical",
        "efess",
        "destroy",
    ]
    .get(usize::from(value))
    .copied()
}

/// `{"id": value, "name": lookup(value)}`, the shape every byte meaning takes.
pub fn named_byte(value: u8, lookup: fn(u8) -> Option<&'static str>) -> psiv_data::NamedId {
    psiv_data::NamedId {
        id: u16::from(value),
        name: lookup(value).map(str::to_owned),
    }
}

/// One of `_display_names`' maps: id to the cartridge's own name. A repeated
/// id keeps its last name, as the Python dict does.
pub type Display = HashMap<u64, Option<String>>;

/// `_display_names`: each record table's display names, keyed as
/// `DISPLAY_NAME_TABLES` keys them.
pub fn display_names(rom: &[u8]) -> Result<HashMap<&'static str, Display>, PackError> {
    let names = serde_json::Value::from(&crate::text::extract_names(rom)?);
    let tables = [
        ("enemies", "enemy_names"),
        ("items", "item_names"),
        ("enemy_skills", "enemy_skill_names"),
        ("techniques", "technique_names"),
        ("skills", "skill_names"),
        ("characters", "character_names"),
        ("professions", "profession_names"),
    ];
    let mut out = HashMap::new();
    for (key, table) in tables {
        let entries = names[table]
            .as_array()
            .ok_or_else(|| PackError(format!("the name tables have no {table}")))?;
        let mut map = Display::new();
        for entry in entries {
            let id = entry["id"]
                .as_u64()
                .ok_or_else(|| PackError(format!("a {table} entry has no id")))?;
            map.insert(id, entry["name"].as_str().map(str::to_owned));
        }
        out.insert(key, map);
    }
    Ok(out)
}

/// `display.get(id)`: the name, if the table has one for `id`.
pub fn display(map: &Display, id: u16) -> Option<String> {
    map.get(&u64::from(id)).cloned().flatten()
}
