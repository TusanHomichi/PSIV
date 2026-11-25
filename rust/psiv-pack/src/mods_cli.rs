//! `psiv-pack mod <init|diff|check|schema>` subcommands (#118, #789, #790).

use std::path::{Path, PathBuf};

use psiv_data::mods::{
    DiffOptions, ModManifest, apply_mods, audit_mod_shareability, check_mod_schemas,
    diff_packs_with_options, init_mod_dir, load_mod, write_mod_schemas,
};
use psiv_pack::PackError;

pub const MOD_USAGE: &str = "\
usage:
  psiv-pack mod init <out-dir> --id <id> [--name <name>] [--version <ver>]
  psiv-pack mod diff <base-pack> <edited-pack> <out-dir> [--id <id>] [--name <name>] [--version <ver>] [--allow-derived]
  psiv-pack mod check <base-pack> <mod-dir>...
  psiv-pack mod schema <out-dir> [--check]";

pub fn run_mod(args: &[String]) -> Result<(), PackError> {
    let Some((sub, rest)) = args.split_first() else {
        return Err(PackError(MOD_USAGE.into()));
    };
    match sub.as_str() {
        "init" => run_init(rest),
        "diff" => run_diff(rest),
        "check" => run_check(rest),
        "schema" => run_schema(rest),
        _ => Err(PackError(MOD_USAGE.into())),
    }
}

struct ManifestFlags {
    id: Option<String>,
    name: Option<String>,
    version: String,
    author: Option<String>,
    description: Option<String>,
    order: i32,
    allow_derived: bool,
}

fn parse_manifest_flags(
    args: &[String],
    accept_allow_derived: bool,
) -> Result<(Vec<String>, ManifestFlags), PackError> {
    let mut positional = Vec::new();
    let mut flags = ManifestFlags {
        id: None,
        name: None,
        version: "1.0.0".to_owned(),
        author: None,
        description: None,
        order: 100,
        allow_derived: false,
    };
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if let Some(flag) = arg.strip_prefix("--") {
            if flag == "allow-derived" {
                if !accept_allow_derived {
                    return Err(PackError(format!("unknown flag --{flag}\n{MOD_USAGE}")));
                }
                flags.allow_derived = true;
                i += 1;
                continue;
            }
            if !matches!(
                flag,
                "id" | "name" | "version" | "author" | "description" | "order"
            ) {
                return Err(PackError(format!("unknown flag --{flag}\n{MOD_USAGE}")));
            }
            let Some(val) = args.get(i + 1) else {
                return Err(PackError(format!("missing value for --{flag}")));
            };
            match flag {
                "id" => flags.id = Some(val.clone()),
                "name" => flags.name = Some(val.clone()),
                "version" => flags.version = val.clone(),
                "author" => flags.author = Some(val.clone()),
                "description" => flags.description = Some(val.clone()),
                "order" => {
                    flags.order = val
                        .parse::<i32>()
                        .map_err(|e| PackError(format!("invalid --order {val:?}: {e}")))?;
                }
                _ => unreachable!(),
            }
            i += 2;
        } else {
            positional.push(arg.to_owned());
            i += 1;
        }
    }
    Ok((positional, flags))
}

fn build_manifest(flags: ManifestFlags, default_id: &Path) -> Result<ModManifest, PackError> {
    let id = match flags.id {
        Some(id) => id,
        None => default_id
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| PackError("pass --id <mod-id>".into()))?,
    };
    let name = flags.name.unwrap_or_else(|| id.clone());
    let mut manifest = ModManifest::new(id, name, flags.version);
    manifest.author = flags.author;
    manifest.description = flags.description;
    manifest.order = flags.order;
    Ok(manifest)
}

fn run_init(args: &[String]) -> Result<(), PackError> {
    let (pos, flags) = parse_manifest_flags(args, false)?;
    let [out_dir] = pos.as_slice() else {
        return Err(PackError(MOD_USAGE.into()));
    };
    let out_path = Path::new(out_dir);
    let manifest = build_manifest(flags, out_path)?;
    init_mod_dir(out_path, &manifest).map_err(|e| PackError(e.to_string()))?;
    println!("initialized mod `{}` in {out_dir}", manifest.id);
    Ok(())
}

fn run_diff(args: &[String]) -> Result<(), PackError> {
    let (pos, flags) = parse_manifest_flags(args, true)?;
    let [base, edited, out_dir] = pos.as_slice() else {
        return Err(PackError(MOD_USAGE.into()));
    };
    let allow_derived = flags.allow_derived;
    let out_path = Path::new(out_dir);
    let manifest = build_manifest(flags, out_path)?;
    let summary = diff_packs_with_options(
        Path::new(base),
        Path::new(edited),
        out_path,
        &manifest,
        DiffOptions { allow_derived },
    )
    .map_err(|e| PackError(e.to_string()))?;
    if !summary.derived_items.is_empty() {
        eprintln!(
            "warning: `--allow-derived` exported {} non-delta base item(s) (keep private unless replaced with original work):",
            summary.derived_items.len()
        );
        for item in &summary.derived_items {
            eprintln!("  - {item}");
        }
    }
    println!(
        "wrote mod `{}` to {out_dir} ({} patch file(s), {} asset(s))",
        manifest.id,
        summary.patches.len(),
        summary.assets.len()
    );
    Ok(())
}

fn run_check(args: &[String]) -> Result<(), PackError> {
    if args.len() < 2 {
        return Err(PackError(MOD_USAGE.into()));
    }
    let base = Path::new(&args[0]);
    let mut mods = Vec::with_capacity(args.len() - 1);
    for mod_arg in &args[1..] {
        let loaded = load_mod(Path::new(mod_arg)).map_err(|e| PackError(e.to_string()))?;
        mods.push(loaded);
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let staging: PathBuf = std::env::temp_dir().join(format!(
        "psiv-pack-mod-check-{}-{stamp}",
        std::process::id()
    ));
    let result = apply_mods(base, &mods, &staging);
    let _ = std::fs::remove_dir_all(&staging);
    let applied = result.map_err(|e| PackError(e.to_string()))?;
    for m in &mods {
        let notices = audit_mod_shareability(base, m).map_err(|e| PackError(e.to_string()))?;
        for notice in notices {
            let level = if notice.is_warning() {
                "warning"
            } else {
                "note"
            };
            eprintln!("{level}: mod `{}`: {notice}", m.manifest.id);
        }
    }
    println!(
        "ok: {} mod(s), {} patched file(s), {} replaced asset(s)",
        applied.active.len(),
        applied.patched_files.len(),
        applied.replaced_assets.len()
    );
    Ok(())
}

fn run_schema(args: &[String]) -> Result<(), PackError> {
    let mut check_only = false;
    let mut positional = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--check" => check_only = true,
            flag if flag.starts_with("--") => {
                return Err(PackError(format!("unknown flag {flag}\n{MOD_USAGE}")));
            }
            other => positional.push(other),
        }
    }
    let [out_dir] = positional.as_slice() else {
        return Err(PackError(MOD_USAGE.into()));
    };
    let dir = Path::new(out_dir);
    if check_only {
        check_mod_schemas(dir).map_err(|problems| PackError(problems.join("\n")))?;
        println!("ok: schemas in {out_dir} match psiv-data");
    } else {
        let written = write_mod_schemas(dir).map_err(|e| PackError(e.to_string()))?;
        println!("wrote {} schema file(s) to {out_dir}", written.len());
    }
    Ok(())
}
