// Rust bridge benchmark — Tier 1: extern "C" (direct link)
// 2026-07-25: Links directly to bench_add.so at compile time.
// After LTO this is ~0ns overhead — the function IS the native function.
//
// Build:
//   rustc -O bench_rust.rs -o bench_rust -L out -l bench_add
// Run:
//   LD_LIBRARY_PATH=out ./bench_rust

use std::time::Instant;

extern "C" {
    fn add(a: i64, b: i64) -> i64;
}

fn main() {
    // Warmup + verify correctness
    let warm = unsafe { add(3, 4) };
    assert_eq!(warm, 7);

    // Benchmark: 100000 iterations, measure total time
    const N: usize = 100000;
    let t0 = Instant::now();
    for _ in 0..N {
        unsafe { add(3, 4); }
    }
    let t1 = Instant::now();
    let ns = t1.duration_since(t0).as_nanos() as i64;
    println!("  Rust (direct extern C)  median={}ns  result={}", ns / N as i64, warm);
    println!("  total: {}ns over {} iterations", ns, N);
}
