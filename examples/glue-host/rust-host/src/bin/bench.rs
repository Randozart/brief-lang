// Rust native-speed benchmark: Rust → Briev (GLUE, C ABI) vs native Rust.
// 2026-08-03: quantifies the boundary overhead so Briev can be used for
// compiler-internal components without loss of efficiency. The boundary is a
// single C ABI call (zero marshalling); the work is FNV-1a folding over
// `count` features, identical in Briev and native Rust.
//
// Run: BRIEVC=<repo>/target/release/brievc cargo run --release --bin bench

#[path = "../briev_bindings.rs"]
mod briev_bindings;
use briev_bindings::*;
use std::ffi::c_void;
use std::time::Instant;

const COUNT: i64 = 1000;
const ITERS: u64 = 200_000;

/// The exact FNV-1a folding the Briev bridge implements (i64 wrapping).
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
    let briev = unsafe {
        let state: *mut c_void = __briev_init_state();
        move || feature_hash(state, COUNT, seed)
    };
    let native = || feature_hash_native(COUNT, seed);

    // Correctness first: identical output.
    let briev_out = std::hint::black_box(briev());
    let native_out = native();
    assert_eq!(briev_out, native_out, "Briev and native must agree");
    println!("output identical: feature_hash({COUNT}, {seed}) = {briev_out}\n");

    println!("per-call latency over {ITERS} calls (feature_hash count={COUNT}):");
    bench("Rust -> Briev (GLUE)", &briev);
    bench("native Rust", &native);
}
