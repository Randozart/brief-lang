// ── Compiler-in-Brief: build the needs_state pass library ──────────────
// 2026-08-04 (plan 2026-08-04-compiler-in-brief-dogfood-ffi, P3): produce
// `target/compiler-in-brief/needs_state.so` (the Brief pass compiled by
// briefc) so the crate can dlopen it at runtime. The .so is EMBEDDED via
// cargo:rustc-env (BRIEF_COMPILER_IN_BRIEF_SO) and loaded on first use by
// src/glue/brief_pass.rs — the same way a host language loads a Brief bridge.
//
// Bootstrap ordering: `briefc` IS this crate's binary, so the FIRST cargo
// build has no briefc yet and skips the pass (the runtime falls back to the
// Rust reference, and the transition test still guards correctness). Every
// build after that finds target/{debug,release}/briefc and rebuilds the .so.
// `cargo:rerun-if-changed` keeps it fresh when the pass source changes.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let out_root = Path::new(&manifest).join("target").join("compiler-in-brief");
    println!("cargo:rerun-if-changed=lib/compiler/needs_state.bv");

    // A prebuilt briefc from a previous build (or BRIEFC_BIN override).
    let briefc = std::env::var("BRIEFC_BIN").ok().map(PathBuf::from).or_else(|| {
        let dbg = Path::new(&manifest).join("target").join("debug").join("briefc");
        let rel = Path::new(&manifest).join("target").join("release").join("briefc");
        if dbg.exists() { Some(dbg) } else if rel.exists() { Some(rel) } else { None }
    });

    let Some(briefc) = briefc else {
        println!("cargo:warning=compiler-in-Brief: no prebuilt briefc found on first build — pass library skipped (runtime falls back to the Rust reference)");
        println!("cargo:rustc-env=BRIEF_COMPILER_IN_BRIEF_SO=");
        return;
    };

    let ok = Command::new(&briefc)
        .args(["build", "lib/compiler/needs_state.bv", "--library", "--out"])
        .arg(&out_root)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        println!("cargo:warning=compiler-in-Brief: briefc build failed — pass library skipped (runtime falls back to the Rust reference)");
        println!("cargo:rustc-env=BRIEF_COMPILER_IN_BRIEF_SO=");
        return;
    }

    // The output .so is `needs_state.so` inside the --out directory.
    let so = out_root.join("needs_state.so");
    if so.exists() {
        println!("cargo:rustc-env=BRIEF_COMPILER_IN_BRIEF_SO={}", so.display());
        println!("cargo:warning=compiler-in-Brief: pass library ready at {}", so.display());
    } else {
        println!("cargo:rustc-env=BRIEF_COMPILER_IN_BRIEF_SO=");
    }
}
