/// build.rs — GLUE Rust bridge compilation
///
/// Compiles bridge.ll → bridge.o → libbridge.a and links it into the binary.
/// The bridge.ll is produced by `briv build --disable-plugin prelude --library bridge.bv --out .`.

use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    let bridge_ll = manifest_dir.join("bridge.ll");
    if !bridge_ll.exists() {
        println!("cargo:warning=bridge.ll not found. Generate it first:");
        println!("cargo:warning=  cd examples/glue-rust-bridge");
        println!("cargo:warning=  briv build --disable-plugin prelude --library bridge.bv --out .");
        return;
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // Compile bridge.ll → bridge.o via llc
    let obj_path = out_dir.join("bridge.o");
    let llc_ok = std::process::Command::new("llc")
        .args(["-filetype=obj", "-O2", "--relocation-model=pic"])
        .arg("-o")
        .arg(&obj_path)
        .arg(&bridge_ll)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !llc_ok {
        println!("cargo:warning=llc failed. Install LLVM tools (apt install llvm).");
        println!("cargo:warning=Manual: llc bridge.ll -filetype=obj -O2 -o {}", obj_path.display());
        return;
    }

    println!("cargo:warning=Bridge .o: {}", obj_path.display());

    // Create static archive (rename .o → libbridge.a — works for single-object archives)
    let archive_path = out_dir.join("libbridge.a");
    std::fs::copy(&obj_path, &archive_path).unwrap();
    println!("cargo:warning=Bridge archive: {}", archive_path.display());

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=bridge");
}
