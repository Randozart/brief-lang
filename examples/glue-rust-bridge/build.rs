/// build.rs — GLUE Rust bridge compilation
///
/// Reads bridge-exports.dbvl for metadata about exported functions and
/// their type signatures, then compiles bridge.ll → bridge.o via llc.
/// The resulting object file is linked into the final binary.
///
/// Prerequisites:
///   - LLVM toolchain (llc) in PATH
///   - bridge.ll from `brief build --library bridge.bv --out .`
///   - bridge-exports.dbvl from `brief export bridge.bv rust --out .`

use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    // Find bridge.ll
    let bridge_ll = manifest_dir.join("bridge.ll");
    let bridge_ll_alt = manifest_dir.join("bridge-bridge").join("bridge.ll");
    let bridge_ll = if bridge_ll.exists() {
        bridge_ll
    } else if bridge_ll_alt.exists() {
        bridge_ll_alt
    } else {
        println!("cargo:warning=bridge.ll not found. Run these commands first:");
        println!("cargo:warning=  cd examples/glue-rust-bridge");
        println!("cargo:warning=  brief build --library bridge.bv --out .");
        println!("cargo:warning=  brief export bridge.bv rust --out .");
        return;
    };

    // Find bridge-exports.dbvl (in crate root or bridge-bridge/ subdirectory)
    let exports_dirs = [
        manifest_dir.join("bridge-exports.dbvl"),
        manifest_dir.join("bridge-bridge").join("bridge-exports.dbvl"),
    ];
    let exports_path = exports_dirs.iter().find(|p| p.exists()).cloned();

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let obj_path = PathBuf::from(&out_dir).join("bridge.o");

    // Parse bridge-exports.dbvl for metadata (informational only — type checking
    // at the Rust side is manual until we generate a typed header)
    if let Some(ref ep) = exports_path {
        let content = std::fs::read_to_string(ep).unwrap_or_default();
        let mut export_count = 0;
        let mut meld_count = 0;
        for line in content.lines() {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.is_empty() {
                continue;
            }
            match parts[0] {
                "export" if parts.len() >= 4 => {
                    println!("cargo:warning=  export {}: {}({}) -> {}", parts[1], parts[1], parts[2], parts[3]);
                    export_count += 1;
                }
                "meld" if parts.len() >= 4 => {
                    println!("cargo:warning=  meld {} <:> {}: {}", parts[1], parts[2], parts[3]);
                    meld_count += 1;
                }
                "ctype" if parts.len() >= 3 => {
                    println!("cargo:warning=  ctype {} = {}", parts[1], parts[2]);
                }
                _ => {}
            }
        }
        println!("cargo:warning=Bridge: {} exports, {} meld declarations", export_count, meld_count);
    } else {
        println!("cargo:warning=bridge-exports.dbvl not found (optional — linking proceeds without metadata)");
    }

    // Compile bridge.ll → bridge.o via llc
    let llc_result = std::process::Command::new("llc")
        .args(["-filetype=obj", "-O2", "--relocation-model=pic"])
        .arg("-o")
        .arg(&obj_path)
        .arg(&bridge_ll)
        .status();

    match llc_result {
        Ok(status) if status.success() => {
            println!("cargo:warning=Bridge object: {}", obj_path.display());
            // Tell cargo to link the object
            println!("cargo:rustc-link-search=native={}", out_dir);
            println!("cargo:rustc-link-lib=static=bridge");
        }
        Ok(_) => {
            println!("cargo:warning=llc failed with non-zero exit. Install LLVM tools:");
            println!("cargo:warning=  apt install llvm clang");
            println!("cargo:warning=Or compile manually:");
            println!("cargo:warning=  llc bridge.ll -filetype=obj -O2 -o {}", obj_path.display());
        }
        Err(e) => {
            println!("cargo:warning=llc not found: {}. Install LLVM tools and try again.", e);
        }
    }
}
