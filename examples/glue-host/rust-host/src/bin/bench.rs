// Rust native-speed benchmark: Rust → Brief (GLUE, C ABI) vs native Rust.
// 2026-08-03: quantifies the boundary overhead so Brief can be used for
// compiler-internal components without loss of efficiency. The boundary is a
// single C ABI call (zero marshalling); the work is FNV-1a folding over
// `count` features, identical in Brief and native Rust.
//
// Run: BRIEFC=<repo>/target/release/briefc cargo run --release --bin bench

#[path = "../brief_bindings.rs"]
mod brief_bindings;
use brief_bindings::*;
use std::ffi::c_void;
use std::time::Instant;

const COUNT: i64 = 1000;
const ITERS: u64 = 200_000;

/// The exact FNV-1a folding the Brief bridge implements (i64 wrapping).
fn feature_hash_native(count: i64, seed: i64) -> i64 {
    let mut h = seed;
    for i in 0..count {
        h = (h ^ i.wrapping_mul(2654435761)).wrapping_mul(1099511628211);
    }
    h
}

fn bench(name: &str, mut f: impl FnMut() -> i64) {
    // Warm-up.
    for _ in 0..1000 {
        std::hint::black_box(f());
    }
    let t0 = Instant::now();
    for _ in 0..ITERS {
        std::hint::black_box(f());
    }
    let dt = t0.elapsed();
    let per_call_ns = dt.as_nanos() as f64 / ITERS as f64;
    println!("{name:>22}: {per_call_ns:8.1} ns/call  ({:?} total for {ITERS})", dt);
}

fn main() {
    let seed = 42i64;
    let brief = unsafe {
        let state: *mut c_void = __brief_init_state();
        move || feature_hash(state, COUNT, seed)
    };
    let native = || feature_hash_native(COUNT, seed);

    // Correctness first: identical output.
    let brief_out = std::hint::black_box(brief());
    let native_out = native();
    assert_eq!(brief_out, native_out, "Brief and native must agree");
    println!("output identical: feature_hash({COUNT}, {seed}) = {brief_out}\n");

    println!("per-call latency over {ITERS} calls (feature_hash count={COUNT}):");
    bench("Rust -> Brief (GLUE)", &brief);
    bench("native Rust", &native);
}
