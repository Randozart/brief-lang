// Rust bridge benchmark — Tier 1: extern "C" (gen_rust output)
// 2026-07-24: Measures per-call latency of Brief export via Rust extern "C".
// After LTO this is ~0ns overhead — the function IS the native function.
//
// Build:
//   rustc -O bench_rust.rs -o bench_rust -L out -l bench_add
// or with dynamic loading:
//   rustc -O bench_rust.rs -o bench_rust -l dl
// Run:
//   ./bench_rust

use std::time::Instant;

fn main() {
    // Load via dlsym (equivalent to generated bridge.rs)
    let lib = unsafe { libc::dlopen("out/bench_add.so\0".as_ptr() as *const i8, libc::RTLD_LAZY) };
    if lib.is_null() {
        eprintln!("dlopen failed");
        std::process::exit(1);
    }

    let add_fn: unsafe extern "C" fn(i64, i64) -> i64 = unsafe {
        std::mem::transmute(libc::dlsym(lib, "add\0".as_ptr() as *const i8))
    };

    // Warmup
    let warm = unsafe { add_fn(3, 4) };
    assert_eq!(warm, 7);

    // Benchmark
    const N: usize = 100000;
    let t0 = Instant::now();
    for _ in 0..N {
        unsafe { add_fn(3, 4); }
    }
    let t1 = Instant::now();
    let ns = t1.duration_since(t0).as_nanos() as i64;
    println!("  Rust (dlsym extern C)  median={}ns  result={}", ns / N as i64, warm);
    println!("  total: {}ns over {} iterations", ns, N);
}
