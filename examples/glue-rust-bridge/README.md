# GLUE Rust Bridge Example

This example demonstrates the **GLUE protocol** (General Language Unification Engine)
— calling Brief-exported functions from Rust with zero-copy FFI.

## Prerequisites

- LLVM toolchain (`llc`) — `apt install llvm` or `brew install llvm`
- Brief compiler (`brief`) — `cargo build --release` from repo root

## Workflow

```bash
# Step 1: Build bridge.ll with --library mode (no main, has __brief_init_state)
cd examples/glue-rust-bridge
brief build --library bridge.bv --out .

# Step 2: Export bridge metadata
brief export bridge.bv rust --out .

# Step 3: Build Rust binary
cargo build

# Step 4: Run
cargo run
# ═══ GLUE Rust Bridge Demo ═══
#   Brief runtime initialized (state=0x...)
#   add(40, 2) = 42
#   multiply(6, 7) = 42
# ═══ All bridge calls passed ═══
```

## How It Works

### Brief Side (`bridge.bv`)
- `#export` functions are compiled to externally-visible LLVM functions
- `brief build --library` emits `.ll` with `__brief_init_state()` and exports, but no `main()`
- `brief export` writes `bridge-exports.dbvl` with tagged entries

### Rust Side (`build.rs` + `main.rs`)
- `build.rs` reads `bridge-exports.dbvl` for metadata and runs `llc` on the `.ll`
- The bridge object is linked statically into the binary
- `main.rs` declares `extern "C"` functions matching the LLVM signatures
- `__brief_init_state()` initializes the Brief runtime
- Each export takes the state pointer as its first argument, plus typed params

## LLVM Function Signatures

Brief `i64` exports become:
```llvm
define i64 @add(ptr %state, i64 %arg0, i64 %arg1) { ... }
```

Rust `extern "C"` declarations match:
```rust
extern "C" {
    fn add(state: *mut c_void, a: i64, b: i64) -> i64;
}
```
