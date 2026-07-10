// GLUE Rust Bridge — dogfooding test
//
// Calls Brief-exported functions through the GLUE protocol.
// The bridge object is compiled by build.rs and linked statically.
//
// Each Brief export compiles to an LLVM function with signature:
//   i64 @<name>(ptr %state, i64 %arg0, i64 %arg1, ...)
//
// The first argument is the Brief state pointer (opaque).
// Call __brief_init_state() once before any export to initialize.

extern "C" {
    fn __brief_init_state() -> *mut std::ffi::c_void;
    fn add(state: *mut std::ffi::c_void, a: i64, b: i64) -> i64;
    fn multiply(state: *mut std::ffi::c_void, a: i64, b: i64) -> i64;
}

fn main() {
    println!("═══ GLUE Rust Bridge Demo ═══");

    let state = unsafe { __brief_init_state() };
    println!("  Brief runtime initialized (state=0x{:x})", state as usize);

    let sum = unsafe { add(state, 40, 2) };
    println!("  add(40, 2) = {}", sum);
    assert_eq!(sum, 42);

    let product = unsafe { multiply(state, 6, 7) };
    println!("  multiply(6, 7) = {}", product);
    assert_eq!(product, 42);

    println!("═══ All bridge calls passed ═══");
}
