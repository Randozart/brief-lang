# GLUE Rust Bridge Example

Call Briev-exported functions from Rust via the GLUE protocol.

## Quick Start

```bash
cd examples/glue-rust-bridge
briev build --disable-plugin prelude --library bridge.bv --out .
cargo build
cargo run
```

Expected output:
```
═══ GLUE Rust Bridge Demo ═══
  Briev runtime initialized (state=0x...)
  add(40, 2) = 42
  multiply(6, 7) = 42
═══ All bridge calls passed ═══
```

## How It Works

1. `briev build --disable-plugin prelude --library bridge.bv --out .` compiles to `bridge.ll`
   — a reusable LLVM IR module with `__briev_init_state()` + exports, no `main()`
2. `build.rs` runs `llc bridge.ll -filetype=obj -O2 -o bridge.o`, packs into `libbridge.a`
3. Rust binary links the archive statically
4. `main.rs` declares `extern "C"` functions matching the LLVM signatures,
   calls `__briev_init_state()` once, then calls exports directly

## C ABI Convention

```rust
// LLVM:  define i64 @add(ptr %state, i64 %a, i64 %b)
extern "C" {
    fn __briev_init_state() -> *mut c_void;  // returns state ptr
    fn add(state: *mut c_void, a: i64, b: i64) -> i64;
}
```

## Prerequisites

- LLVM toolchain (`llc`) — `apt install llvm` or `brew install llvm`
- Briev compiler from this repo
