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
    fn factorial(state: *mut std::ffi::c_void, n: i64) -> i64;
}

fn main() {
    println!("═══ GLUE Rust Bridge Demo ═══");

    // Initialize Brief runtime — must be called once before any export.
    // Returns an opaque state pointer that is threaded through all calls.
    let state = unsafe { __brief_init_state() };
    println!("  Brief runtime initialized (state=0x{:x})", state as usize);

    // Call Brief exports
    let sum = unsafe { add(state, 40, 2) };
    println!("  add(40, 2) = {}", sum);
    assert_eq!(sum, 42, "add should compute 40 + 2 = 42");

    let product = unsafe { multiply(state, 6, 7) };
    println!("  multiply(6, 7) = {}", product);
    assert_eq!(product, 42, "multiply should compute 6 * 7 = 42");

    let fact = unsafe { factorial(state, 10) };
    println!("  factorial(10) = {}", fact);
    assert_eq!(fact, 3628800, "factorial(10) should be 3628800");

    println!("═══ All bridge calls passed ═══");
}
