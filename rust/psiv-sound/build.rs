fn main() {
    let manifest_dir = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo provides the manifest path"),
    );
    let oracle_dir = manifest_dir.join("../../oracle/gpgx-src/core/sound");
    let nuked_source = oracle_dir.join("ym3438.c");
    let bridge = manifest_dir.join("src/nuked_bridge.c");

    println!("cargo:rerun-if-changed={}", nuked_source.display());
    println!(
        "cargo:rerun-if-changed={}",
        oracle_dir.join("ym3438.h").display()
    );
    println!("cargo:rerun-if-changed={}", bridge.display());

    cc::Build::new()
        .define("HAVE_YM3438_CORE", None)
        .include(&oracle_dir)
        .file(nuked_source)
        .file(bridge)
        .warnings(false)
        .compile("psiv_nuked_opn2");
}
