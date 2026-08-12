# Target-Configurable `#Int` Protocol Width & Narrowing Fix

**Date:** 2026-07-25
**Author:** Agent
**Status:** Implementation plan

## 1. Motivation

### 1.1 The Problem

The `#Int` protocol category currently hardcodes to `i64` in the LLVM backend
regardless of target. This causes two issues:

1. **WASM emits i64 → BigInt in JavaScript** (~50ns overhead). Pure JS `number`
   addition runs at ~68ns, but our WASM `add` runs at 122ns because every call
   converts through `BigInt`. Native speed requires i32.

2. **The narrowing pass (which shrinks Int to i8/i16/i32 when contracts prove
   safety) is incomplete** — only `Add` uses the narrowed type. `Sub`, `Mul`,
   `Div`, `Mod` hardcode `i64` operations, which would produce invalid LLVM IR
   (e.g., `sub i64` on i32 registers) if narrowing fired.

### 1.2 The Architecture

`#Int` is a protocol category, not a concrete type. The compiler should resolve
it to the target's native integer width:

```
#Int protocol → target.int_bits (configurable, default 64)
             → narrowing pass proves narrower (overrides, optional)
             → LLVM type i{width}
```

Priority chain for determining `Int`'s LLVM type:
1. **Narrowing evidence** (contract `[a < 1000]`) — strongest proof, overrides all
2. **`--int-bits` config** (target hint, e.g. 32 for WASM) — default when no proof
3. **Primordial/universe fallback** (hardcoded `i64`) — only if neither applies

## 2. Changes

### 2.1 Fix Sub/Mul/Div/Mod — use `binop_int_type()` like Add

**File:** `src/backend/llvm/emit_expr.rs`

**What:** Replace hardcoded `i64` in arithmetic operations with the same
`binop_int_type()` helper that `Add` already uses. This function returns the
narrowed width when available, `"i64"` otherwise — so no-contract case is
unchanged.

| Line | Op | Current | Fixed |
|------|-----|---------|-------|
| 1837 | `Sub` | `sub nsw i64` | `sub nsw {}` |
| 1854 | `Mul` | `mul nsw i64` | `mul nsw {}` |
| 1870 | `Div` | `sdiv i64` | `sdiv {}` |
| 1878 | `Mod` | `srem i64` | `srem {}` |

Each becomes a two-line change:
```rust
let op_bits = self.binop_int_type();
writeln!(out, "{}{} = sub nsw {} {}, {}", indent, v, op_bits, l.name, r.name).ok();
```

`Mod` also needs `ret_ty` fixed — it currently hardcodes `Type::int()` instead
of using the incoming operand type. Changed to use `ret_ty.clone()` like the
other ops.

**Rationale comment** at each changed arm:
```rust
// 2026-07-25: Use binop_int_type() so narrowing pass controls width.
// Was hardcoded i64 — worked for non-narrowed code but broke WASM i32
// by emitting sub nsw i64 on i32 registers.
```

**Files changed:** 1

### 2.2 Add `--int-bits` CLI flag

**File:** `src/main.rs`

Add parsing for `--int-bits <N>` CLI flag:
```rust
let mut int_bits = 64u64;
// ...in the argument loop...
} else if arg == "--int-bits" {
    let val = args.get(i + 1).ok_or("--int-bits requires a number argument")?;
    int_bits = val.parse()
        .map_err(|_| format!("invalid --int-bits value: '{}'", val))?;
    i += 2;
```

Validate that `int_bits` is a positive power of two (8, 16, 32, 64):
```rust
if int_bits != 8 && int_bits != 16 && int_bits != 32 && int_bits != 64 {
    return Err(format!("--int-bits must be 8, 16, 32, or 64, got: {}", int_bits));
}
```

Update `BuildOptions` construction to include `int_bits`.

**File:** `src/compile.rs` — add field to `BuildOptions`:
```rust
/// 2026-07-25: Native integer width for #Int protocol (default 64).
/// WASM targets should set this to 32 to avoid BigInt in JavaScript.
pub int_bits: u64,
```

Initialize in default/fallback constructors: `int_bits: 64`.

**Rationale comments:**
```rust
// 2026-07-25: Target #Int protocol width. WASM uses 32 to emit i32
// instead of i64, eliminating BigInt overhead in JavaScript.
```

**Files changed:** 2

### 2.3 Thread `int_bits` to `CompilerContext`

**File:** `src/backend/llvm/mod.rs` — add builder method:
```rust
/// 2026-07-25: Set the native integer width for #Int protocol.
pub fn with_int_bits(mut self, bits: u64) -> Self {
    self.ctx.int_bits = bits;
    self
}
```

**File:** `src/backend/llvm/context.rs` — add field:
```rust
/// 2026-07-25: Native integer width for #Int protocol (default 64).
/// Controls i32 vs i64 emission for Int/UInt types.
pub int_bits: u64,
```

Initialize in `CompilerContext::new()`:
```rust
int_bits: 64,
```

**File:** `src/compile.rs` — wire into `LlvmBackend` construction in both
`BackendKind::Llvm` and `BackendKind::Gpu` paths:
```rust
.with_int_bits(opts.int_bits)
```

**Rationale comments:**
```rust
// 2026-07-25: Target-specific #Int protocol width. WASM = 32 for
// i32 emission (no BigInt), native = 64 for register-width matching.
```

**Files changed:** 3

### 2.4 Use `int_bits` in `llvm_type()` as fallback

**File:** `src/backend/llvm/emit_toplevel.rs`

In `llvm_type()`, after the narrowing check (line ~370) and before the universe
fallback, add a check for `Int`/`UInt` that returns the configured `int_bits`:

```rust
// 2026-07-25: #Int protocol default width from --int-bits.
// Narrowing evidence (contracts) has priority — if we reach here,
// no narrowing was proven. Fall back to target-configured default.
if let Type::Custom(name) = ty {
    if name == "Int" || name == "UInt" {
        return format!("i{}", self.ctx.int_bits);
    }
}
```

This returns early for Int/UInt before the universe lookup, which would return
`"i64"` from the primordial table. On WASM with `--int-bits 32`, this emits
`i32` for all Int parameters, returns, and operations.

No change needed to `fallback_llvm_type()` — it's only reached for non-Int types
(e.g. `Bits`, `Float`, etc.) after this early return.

**Rationale comment:**
```rust
// 2026-07-25: #Int protocol resolved to target's native width.
// Narrowing evidence (from contracts) would have returned above.
// This fallback emits i{int_bits} for Int/UInt, overriding the
// primordial table's hardcoded i64. On WASM with int_bits=32,
// this eliminates BigInt without requiring contract annotations.
```

**Files changed:** 1

### 2.5 Update WASM Makefile

**File:** `benchmarks/metropolitan/Makefile`

Add `--int-bits 32` to the LLVM IR generation target:
```makefile
$(OUT_DIR)/bench_add.ll: $(BV_SRC) $(BRIEVC) | $(OUT_DIR)
	cd $(PROJECT_ROOT) && $(BRIEVC) build $(BV_SRC) --llvm --out $(OUT_DIR) --int-bits 32 2>&1
```

Also update the `.so` build for consistency (though the .so is native x86_64
so it stays at i64 — the flag only matters for WASM targets):
```makefile
$(OUT_DIR)/bench_add.so: $(BV_SRC) $(BRIEVC) | $(OUT_DIR)
	cd $(PROJECT_ROOT) && $(BRIEVC) build $(BV_SRC) --shared --out $(OUT_DIR) 2>&1
	ln -sf bench_add.so $(OUT_DIR)/libbench_add.so
```
(No change needed for .so — it naturally stays i64.)

**Files changed:** 1

### 2.6 Update WASM benchmark contract and bridge

**File:** `benchmarks/metropolitan/bench_add.bv`

Add contract precondition so the narrowing pass can prove i32 safety:
```briev
export defn add(a: Int, b: Int [a < 1000 && b < 1000]) -> Int {
    term a + b;
};

export defn mul(a: Int, b: Int [a < 1000 && b < 1000]) -> Int {
    term a * b;
};
```

**Rationale:** On WASM with `--int-bits 32`, `add` gets `i32` by default.
But the narrowing pass can independently prove ≤32 bits from the contract,
working on any target. This demonstrates both mechanisms.

**File:** `benchmarks/metropolitan/bench_wasm.mjs`

Drop `BigInt()` wrapping. With i32, WASM exports return plain numbers:
```javascript
// Before:
let warm = add(3n, 4n);
// After:
let warm = add(3, 4);
```

Remove the `(a, b) => add(BigInt(a), BigInt(b))` wrapper:
```javascript
run("wasm add", add, 50000, 3, 4);
```

**Files changed:** 2

### 2.7 Update `gen_wasm.bv` (conditional BigInt)

**File:** `lib/ffi/gen_wasm.bv`

The generated bridge currently wraps all calls in `BigInt()` unconditionally.
With `--int-bits 32`, this wrapping is unnecessary and adds overhead.

The simplest approach: emit two versions of the bridge based on int width.
But since the generator doesn't receive the int_bits flag directly, leave the
bridge generation unchanged for now — the benchmark calls WASM directly via
`instance.exports.add`, not through the generated bridge.

**Rationale comment added to gen_wasm.bv:**
```briev
// 2026-07-25: When --int-bits 32 is used, WASM exports i32 functions
// that return plain JS numbers (no BigInt). The generated bridge
// currently wraps in BigInt() — a future update can detect the int
// width and emit BigInt conditionally.
```

**Files changed:** 1 (comment only)

### 2.8 Run full benchmark suite

After all changes, run:
```bash
cd benchmarks/metropolitan && make clean && make run
```

Expected results:

| Tier | Target | Before | After | Improvement |
|------|--------|--------|-------|-------------|
| WASM | WASM (BigInt) | 122ns | ~70ns | 1.7× |
| WASM | Pure JS (native) | 68ns | 68ns | — |
| Native | C (dlsym) | 2ns | 2ns | unchanged |
| Native | Rust (direct) | 1ns | 1ns | unchanged |
| Tier 2 | Python C ext | 132ns | 132ns | unchanged |
| Tier 2 | Node koffi | 151ns | 151ns | unchanged |

## 3. Files Changed Summary

| # | File | Change | Risk |
|---|------|--------|------|
| 1 | `src/backend/llvm/emit_expr.rs` | Fix Sub/Mul/Div/Mod → `binop_int_type()` | Low — same pattern as Add |
| 2 | `src/main.rs` | Parse `--int-bits` CLI flag | Low — boilerplate flag |
| 3 | `src/compile.rs` | Add `int_bits` to `BuildOptions`, wire to backend | Low — new field |
| 4 | `src/backend/llvm/mod.rs` | Add `with_int_bits()` builder | Low — builder pattern |
| 5 | `src/backend/llvm/context.rs` | Add `int_bits` field, init to 64 | Low — new field |
| 6 | `src/backend/llvm/emit_toplevel.rs` | Use `int_bits` as fallback for Int/UInt | Medium — type dispatch |
| 7 | `benchmarks/metropolitan/Makefile` | Add `--int-bits 32` for WASM target | Low — flag only |
| 8 | `benchmarks/metropolitan/bench_add.bv` | Add contracts for narrowing demo | Low — comment |
| 9 | `benchmarks/metropolitan/bench_wasm.mjs` | Drop BigInt wrapping | Low — JS change |
| 10 | `lib/ffi/gen_wasm.bv` | Add rationale comment | None — comment |

## 4. Documentation

### New doc comments:
- `CompilerContext.int_bits` — `/// 2026-07-25: Native integer width for #Int protocol (default 64). Controls i32 vs i64 emission for Int/UInt types.`
- `LlvmBackend::with_int_bits()` — `/// 2026-07-25: Set the native integer width for #Int protocol.`
- `BuildOptions.int_bits` — `/// 2026-07-25: Native integer width for #Int protocol (default 64). WASM targets should set this to 32 to avoid BigInt in JavaScript.`

### Rationale comments at each modified code site:
- `emit_expr.rs` Sub/Mul/Div/Mod: `// 2026-07-25: Use binop_int_type() so narrowing pass controls width.`
- `emit_toplevel.rs` llvm_type(): `// 2026-07-25: #Int protocol resolved to target's native width.`
- `main.rs` flag parsing: `// 2026-07-25: Target #Int protocol width.`

### Architecture docs:
- `docs/architecture/hash-words.md` — update `#Int` protocol description to note
  configurable width via `--int-bits`.
- `docs/architecture/backend-type-dispatch.md` — add `int_bits` to the type
  resolution priority chain.

## 5. Testing

### Existing tests
All 989 existing tests must pass before and after changes.
```bash
cargo test --lib
```

### Behavioral verification
1. **No `--int-bits` flag → native x86_64**: Default `int_bits=64`. Int → i64.
   Confirm by building `bench_add.bv` and checking `bench_add.ll` for `i64` params.
2. **`--int-bits 32` → WASM target**: All Int → i32. Confirm by building
   `bench_add.bv --int-bits 32 --llvm` and checking `.ll` for `i32` params.
3. **Narrowing + `--int-bits`**: Contract `[a < 1000]` overrides `--int-bits 64`.
   Confirm `i32` appears even with `--int-bits 64` when contract is present.
4. **Contract without `--int-bits`**: Narrows to i32 regardless of target.
5. **WASM benchmark runs**: `make bench_wasm` completes with correct result (7).

### Regression guard
- All existing Sub/Mul/Div/Mod operations without narrowing produce `i64`
  (via `binop_int_type()` fallback) — identical to current behavior.
- The `--int-bits` flag is optional; omitting it leaves everything unchanged.
- New code is additive (new match arms / early returns), no existing paths removed.

## 6. Implementation Order

```
Step B (fix Sub/Mul/Div/Mod)    ─┐
                                 ├── independent, do first
Step C (--int-bits plumbing)    ─┘
         │
         ▼
Step D (llvm_type fallback)     ─── depends on C
         │
         ▼
Step E (WASM benchmark updates) ─── depends on D (to verify)
         │
         ▼
Step F (gen_wasm.bv comment)    ─── minimal change
         │
         ▼
Step G (benchmark suite)        ─── verify everything
```

Steps B and C are independent and can proceed in parallel. Step D depends on C.
Steps E/F/G depend on D.

## 7. Rolling Back

Each step is a separate commit. If the `--int-bits` approach causes issues:
- `git revert <commit>` for the specific step
- Steps B and C can be reverted independently
- Default behavior (no `--int-bits` flag) is identical to pre-change
