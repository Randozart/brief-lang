// ── Phase 8: Pretty-Printer Bridge Round-Trip Test ─────────────────────
// 2026-07-22: Compiles pp-types.bv to a shared library, loads it at
// runtime via libloading, and calls exported functions. Verifies that
// Brief pretty-printer output matches expected strings.
//
// Pipeline: .bv → brief build → .ll → llc → .o → cc → .so → FFI call

use libloading::{Library, Symbol};
use std::ffi::{c_void, CStr, CString};
use std::path::Path;
use std::process::Command;

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn compiler_path() -> String {
    format!("{}/target/debug/brief-compiler", PROJECT_ROOT)
}

fn build_bridge_so() -> String {
    let out_dir = std::env::temp_dir().join("brief_pp_test");
    let _ = std::fs::create_dir_all(&out_dir);

    let bv_path = format!("{}/pp-types.bv", PROJECT_ROOT);
    let ll_path = out_dir.join("pp-types.ll");
    let o_path = out_dir.join("pp-types.o");
    let rt_o_path = out_dir.join("brief_rt.o");
    let so_path = out_dir.join("libpp_types.so");

    let build = Command::new(compiler_path())
        .args(&["build", &bv_path, "--llvm", "--out", &out_dir.to_string_lossy()])
        .output().expect("failed brief-compiler build");
    assert!(build.status.success(), "brief build failed: {}", String::from_utf8_lossy(&build.stderr));

    let llc_out = Command::new("llc")
        .args(&["-filetype=obj", "-relocation-model=pic", "-o", &o_path.to_string_lossy(), &ll_path.to_string_lossy()])
        .output().expect("failed llc");
    assert!(llc_out.status.success(), "llc failed: {}", String::from_utf8_lossy(&llc_out.stderr));

    let rt_c = format!("{}/lib/runtime/brief_rt.c", PROJECT_ROOT);
    let cc_rt = Command::new("cc")
        .args(&["-c", "-fPIC", "-o", &rt_o_path.to_string_lossy(), &rt_c])
        .output().expect("failed cc brief_rt.c");
    assert!(cc_rt.status.success(), "cc brief_rt.c failed: {}", String::from_utf8_lossy(&cc_rt.stderr));

    let cc_so = Command::new("cc")
        .args(&["-shared", "-o", &so_path.to_string_lossy(), &o_path.to_string_lossy(), &rt_o_path.to_string_lossy()])
        .output().expect("failed cc .so");
    assert!(cc_so.status.success(), "cc .so failed: {}", String::from_utf8_lossy(&cc_so.stderr));

    so_path.to_string_lossy().to_string()
}

fn load_bridge() -> (Library, *mut c_void) {
    let so_path = build_bridge_so();
    let lib = unsafe { Library::new(&so_path).expect("failed to load bridge .so") };
    // Allocate a state buffer (32 bytes = 4 * i64 fields) and initialize via init_state()
    let state = unsafe { std::alloc::alloc_zeroed(std::alloc::Layout::from_size_align(32, 8).unwrap()) as *mut c_void };
    unsafe {
        let init: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"init_state").expect("init_state not found");
        init(state);
    }
    (lib, state)
}

// ── IR validation ─────────────────────────────────────────────────────

#[test]
fn test_bridge_compiles_to_valid_llvm_ir() {
    let out_dir = std::env::temp_dir().join("brief_pp_test_ir");
    let _ = std::fs::create_dir_all(&out_dir);
    let bv_path = format!("{}/pp-types.bv", PROJECT_ROOT);
    let ll_path = out_dir.join("pp-types.ll");

    let build = Command::new(compiler_path())
        .args(&["build", &bv_path, "--llvm", "--out", &out_dir.to_string_lossy()])
        .output().expect("failed brief-compiler build");
    assert!(build.status.success(), "brief build failed: {}", String::from_utf8_lossy(&build.stderr));

    let ir = std::fs::read_to_string(&ll_path).expect("failed to read LLVM IR");

    assert!(ir.contains("define i64 @brief_test_type_bits"), "missing brief_test_type_bits");
    assert!(ir.contains("define i64 @brief_test_type_void"), "missing brief_test_type_void");
    assert!(ir.contains("define i64 @brief_test_cstr_roundtrip"), "missing brief_test_cstr_roundtrip");
    assert!(ir.contains("ptr %arg0") || ir.contains("i64 %arg0"), "missing typed args");
}

#[test]
fn test_bridge_compiles_to_shared_library() {
    let so_path = build_bridge_so();
    assert!(Path::new(&so_path).exists(), "shared library missing");
    let meta = std::fs::metadata(&so_path).expect("failed metadata");
    assert!(meta.len() > 1000, ".so too small: {} bytes", meta.len());
}

#[test]
fn test_bridge_loads_and_resolves() {
    let (_lib, state) = load_bridge();
    assert!(!state.is_null(), "state should be non-null");
}

// ── Round-trip FFI tests ──────────────────────────────────────────────

#[test]
fn test_pp_void_via_ffi() {
    let (ref lib, state) = load_bridge();
    unsafe {
        let func: Symbol<unsafe extern "C" fn(*mut c_void) -> i64> =
            lib.get(b"brief_test_type_void").expect("func not found");
        let ptr = func(state);
        eprintln!("void test: ptr={:p}", ptr as *const u8);
        assert_ne!(ptr, 0, "brief_test_type_void returned null");
        let s = CStr::from_ptr(ptr as *const i8).to_str().unwrap();
        eprintln!("void test: result={:?}", s);
        assert_eq!(s, "void");
    }
}

#[test]
fn test_cstr_roundtrip_via_ffi() {
    let (ref lib, state) = load_bridge();
    unsafe {
        let func: Symbol<unsafe extern "C" fn(*mut c_void, i64) -> i64> =
            lib.get(b"brief_test_cstr_roundtrip").expect("func not found");
        let input = CString::new("42").unwrap();
        let ptr = func(state, input.as_ptr() as i64);
        let s = CStr::from_ptr(ptr as *const i8).to_str().unwrap();
        assert_eq!(s, "42");
    }
}

/// Tests pp_type_custom(s) which returns the input as-is (no concatenation).
#[test]
fn test_custom_echo_via_ffi() {
    let (ref lib, state) = load_bridge();
    unsafe {
        let func: Symbol<unsafe extern "C" fn(*mut c_void, i64) -> i64> =
            lib.get(b"brief_test_custom_echo").expect("func not found");
        let input = CString::new("hello").unwrap();
        let ptr = func(state, input.as_ptr() as i64);
        let s = CStr::from_ptr(ptr as *const i8).to_str().unwrap();
        assert_eq!(s, "hello");
    }
}

#[test]
fn test_bits_static_via_ffi() {
    let (ref lib, state) = load_bridge();
    unsafe {
        let func: Symbol<unsafe extern "C" fn(*mut c_void) -> i64> =
            lib.get(b"brief_test_bits_static").expect("func not found");
        let ptr = func(state);
        let s = CStr::from_ptr(ptr as *const i8).to_str().unwrap();
        assert_eq!(s, "Bits(42): test");
    }
}

#[test]
#[ignore]
fn test_pp_bits_via_ffi() {
    let (ref lib, state) = load_bridge();
    unsafe {
        let func: Symbol<unsafe extern "C" fn(*mut c_void, i64) -> i64> =
            lib.get(b"brief_test_type_bits").expect("func not found");
        let input = CString::new("42").unwrap();
        let ptr = func(state, input.as_ptr() as i64);
        let s = CStr::from_ptr(ptr as *const i8).to_str().unwrap();
        assert_eq!(s, "Bits(42)");
    }
}
