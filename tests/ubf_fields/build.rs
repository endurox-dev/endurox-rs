// Generates Rust constants for the test field table with `mkfldhdr -m4`.
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let fd = manifest_dir.join("../ubftab/test.fd");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let mkfldhdr = find_mkfldhdr(&manifest_dir);

    println!("cargo:rerun-if-changed={}", fd.display());
    println!("cargo:rerun-if-env-changed=ENDUROX_MKFLDHDR");

    let status = Command::new(&mkfldhdr)
        .arg("-m4")
        .arg("-d")
        .arg(&out)
        .arg(&fd)
        .status()
        .unwrap_or_else(|e| panic!("failed to execute {}: {e}", mkfldhdr.display()));

    if !status.success() {
        panic!(
            "{} failed to generate Rust UBF constants from {}",
            mkfldhdr.display(),
            fd.display()
        );
    }
}

/// Lookup order: `ENDUROX_MKFLDHDR`, the Enduro/X checkout beside the
/// endurox-rs repository, then `PATH`.
fn find_mkfldhdr(manifest_dir: &Path) -> PathBuf {
    if let Ok(path) = env::var("ENDUROX_MKFLDHDR") {
        return PathBuf::from(path);
    }

    // tests/ubf_fields -> tests -> endurox-rs -> parent holding ../endurox.
    let sibling = manifest_dir
        .ancestors()
        .nth(3)
        .map(|parent| parent.join("endurox/mkfldhdr/mkfldhdr"));
    if let Some(path) = sibling {
        if path.exists() {
            return path;
        }
    }

    PathBuf::from("mkfldhdr")
}
