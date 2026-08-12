// build.rs — builds the Briev library and links it into this crate.
// 2026-08-03: `brievc build rank.bv --library` produces librank.a (real ELF
// objects — gcc/rustc-linkable) + rank.so; `brievc bindings rank.bv rust`
// produces src/briev_bindings.rs. The boundary is a plain C ABI call with no
// marshalling — the Rust LTO host path.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let brievc = std::env::var("BRIEVC").unwrap_or_else(|_| {
        let rel = manifest.join("../../../target/release/brievc");
        if rel.exists() { rel.to_string_lossy().into_owned() } else { "brievc".into() }
    });
    // Canonicalize: the resolver derives its search root from the FILE's
    // parent, and `..` components break Path::parent()-based walks.
    let rank = std::fs::canonicalize(manifest.join("../rank.bv")).unwrap();
    let repo_root = std::fs::canonicalize(manifest.join("../../..")).unwrap();
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("briev");
    std::fs::create_dir_all(&out).unwrap();

    let output = Command::new(&brievc)
        .current_dir(&repo_root)
        .args(["build", rank.to_str().unwrap(), "--library", "--out", out.to_str().unwrap()])
        .output()
        .expect("failed to run brievc build --library");
    if !output.status.success() {
        eprintln!("brievc stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("brievc build --library failed");
    }

    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-lib=rank");
    println!("cargo:rerun-if-changed={}", rank.display());
}
