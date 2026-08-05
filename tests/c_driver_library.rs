// ── C-Driver Library Test ─────────────────────────────────────────────
// 2026-08-03: the `--library` acceptance criterion — compile a Briv bridge
// to a static library (.a) + generated C header, then a plain C program
// includes the header, calls __briv_init_state() and exported functions,
// links the archive, and gets correct results.
//
// Pipeline: briefc build pp-types.bv --library → libpp-types.a + pp-types.so
//           briefc bindings pp-types.bv c → briv_types.h
//           cc driver.c -L. -lpp-types → driver (toolchain-guarded)

use std::path::Path;
use std::process::Command;

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn has(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok()
}

fn cc_guard() -> Option<()> {
    for tool in ["cc", "ar", "llc", "clang"] {
        if !has(tool) {
            eprintln!("SKIP: {} not available", tool);
            return None;
        }
    }
    Some(())
}

#[test]
fn c_driver_calls_briv_library() {
    let Some(()) = cc_guard() else { return };
    let briefc = env!("CARGO_BIN_EXE_briefc");
    let out_dir = std::env::temp_dir().join("briv_c_driver_test");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).unwrap();

    let bv = format!("{}/pp-types.bv", PROJECT_ROOT);

    // 1. Build the static library + .so.
    let build = Command::new(briefc)
        .args(["build", &bv, "--library", "--out", &out_dir.to_string_lossy()])
        .output().expect("failed briefc build --library");
    assert!(build.status.success(), "build --library failed: {}", String::from_utf8_lossy(&build.stderr));

    // 2. Generate the C header.
    let bindings = Command::new(briefc)
        .args(["bindings", &bv, "c", "--out", &out_dir.to_string_lossy()])
        .output().expect("failed briefc bindings");
    assert!(bindings.status.success(), "bindings failed: {}", String::from_utf8_lossy(&bindings.stderr));
    let header = out_dir.join("pp-types-bindings").join("briv_types.h");
    assert!(header.exists(), "briv_types.h not generated");

    // 3. Write a C driver that includes the header and calls exports.
    let driver_c = out_dir.join("driver.c");
    std::fs::write(&driver_c, r#"
#include "briv_types.h"
#include <stdio.h>
#include <string.h>

static char buf[256];
static char* read_cstr(int64_t p) {
    if (!p) return "<null>";
    memcpy(buf, (void*)(uintptr_t)p, 256);
    buf[255] = 0;
    return buf;
}

int main(void) {
    BrivState* st = __briv_init_state();
    printf("bits:%s\n", read_cstr(briv_test_type_bits(st, (int64_t)"42")));
    printf("void:%s\n", read_cstr(briv_test_type_void(st)));
    printf("static:%s\n", read_cstr(briv_test_bits_static()));
    __glue_release(st);
    return 0;
}
"#).unwrap();

    // 4. Compile + link against the .a (plain cc — the archive is real ELF).
    //    .a lives in out_dir directly; the header in <name>-bindings/.
    let header_dir = out_dir.join("pp-types-bindings");
    let driver = out_dir.join("driver");
    let cc = Command::new("cc")
        .current_dir(&out_dir)
        .args(["-o", driver.to_str().unwrap(), driver_c.to_str().unwrap()])
        .arg(format!("-I{}", header_dir.display()))
        .arg(format!("-L{}", out_dir.display()))
        .arg("-lpp-types")
        .output().expect("failed cc");
    assert!(cc.status.success(), "cc failed: {}", String::from_utf8_lossy(&cc.stderr));

    // 5. Run and assert.
    let run = Command::new(&driver).output().expect("failed to run driver");
    assert!(run.status.success(), "driver failed: {}", String::from_utf8_lossy(&run.stderr));
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("bits:Bits(42)"), "type_bits output: {}", stdout);
    assert!(stdout.contains("void:void"), "void output: {}", stdout);
    assert!(stdout.contains("static:Bits(42): test"), "static output: {}", stdout);

    let _ = std::fs::remove_dir_all(&out_dir);
}
