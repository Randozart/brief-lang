// ── Compiler-in-Briv: build the needs_state pass library ──────────────
// 2026-08-04 (plan 2026-08-04-compiler-in-briv-dogfood-ffi, P3): produce
// `target/compiler-in-briv/needs_state.so` (the Briv pass compiled by
// briefc) so the crate can dlopen it at runtime. The .so is EMBEDDED via
// cargo:rustc-env (BRIV_COMPILER_IN_BRIV_SO) and loaded on first use by
// src/glue/briv_pass.rs — the same way a host language loads a Briv bridge.
//
// Bootstrap ordering: `briefc` IS this crate's binary, so the FIRST cargo
// build has no briefc yet and skips the pass (the runtime falls back to the
// Rust reference, and the transition test still guards correctness). Every
// build after that finds target/{debug,release}/briefc and rebuilds the .so.
// `cargo:rerun-if-changed` keeps it fresh when the pass source changes.

use std::path::{Path, PathBuf};
use std::process::Command;

fn build_pass(briefc: &Path, bv: &str, out_root: &Path) -> Option<PathBuf> {
    let ok = Command::new(briefc)
        .args(["build", bv, "--library", "--out"])
        .arg(out_root)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        return None;
    }
    // The output .so is `<stem>.so` inside the --out directory.
    let stem = Path::new(bv).file_stem()?.to_string_lossy().to_string();
    let so = out_root.join(format!("{stem}.so"));
    if so.exists() { Some(so) } else { None }
}

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let out_root = Path::new(&manifest).join("target").join("compiler-in-briv");
    println!("cargo:rerun-if-changed=lib/compiler/needs_state.bv");
    println!("cargo:rerun-if-changed=lib/compiler/soa_reorder.bv");
    println!("cargo:rerun-if-changed=lib/compiler/reader.bv");

    // A prebuilt briefc from a previous build (or BRIEFC_BIN override).
    let briefc = std::env::var("BRIEFC_BIN").ok().map(PathBuf::from).or_else(|| {
        let dbg = Path::new(&manifest).join("target").join("debug").join("briefc");
        let rel = Path::new(&manifest).join("target").join("release").join("briefc");
        if dbg.exists() { Some(dbg) } else if rel.exists() { Some(rel) } else { None }
    });

    let Some(briefc) = briefc else {
        println!("cargo:warning=compiler-in-Briv: no prebuilt briefc found on first build — pass libraries skipped (runtime falls back to the Rust references)");
        println!("cargo:rustc-env=BRIV_COMPILER_IN_BRIV_SO=");
        println!("cargo:rustc-env=BRIV_COMPILER_IN_BRIV_SOA_SO=");
        return;
    };

    // Each pass is an independent .so, dlopen'd by src/glue/briv_pass.rs.
    match build_pass(&briefc, "lib/compiler/needs_state.bv", &out_root) {
        Some(so) => {
            println!("cargo:rustc-env=BRIV_COMPILER_IN_BRIV_SO={}", so.display());
            println!("cargo:warning=compiler-in-Briv: needs_state pass ready at {}", so.display());
        }
        None => {
            println!("cargo:warning=compiler-in-Briv: needs_state pass build failed — runtime falls back to the Rust reference");
            println!("cargo:rustc-env=BRIV_COMPILER_IN_BRIV_SO=");
        }
    }
    match build_pass(&briefc, "lib/compiler/soa_reorder.bv", &out_root) {
        Some(so) => {
            println!("cargo:rustc-env=BRIV_COMPILER_IN_BRIV_SOA_SO={}", so.display());
            println!("cargo:warning=compiler-in-Briv: soa_reorder pass ready at {}", so.display());
        }
        None => {
            println!("cargo:warning=compiler-in-Briv: soa_reorder pass build failed — runtime falls back to the Rust reference");
            println!("cargo:rustc-env=BRIV_COMPILER_IN_BRIV_SOA_SO=");
        }
    }
}
