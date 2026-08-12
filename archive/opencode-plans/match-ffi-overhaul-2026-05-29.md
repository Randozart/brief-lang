# Match Expression + FFI Overhaul

**Date:** 2026-05-29
**Author:** Design session with user
**Status:** Active implementation plan

---

## 1. Match Expression

### Problem
`uni` chains are verbose and error-prone. Three statements for what should be one:

```
uni result(Ok(pair)) = { ... };
uni result(Err(e)) = { ... };
term default;  // Easy to forget → silent Void return
```

No exhaustiveness checking means forgotten arms silently produce `Void`.

### Solution
A `match` expression — concise, exhaustive, readable:

```
match result {
    Ok(pair) => { ... },
    Err(e) => { ... },
};
```

### Design
- **Expression or statement** — expression-first. `let x = match val { ... };` works.
- **Arms evaluated in order**, first match wins.
- **Exhaustive** — require `_ =>` or error at compile time (like Rust).
- **Optional guards**: `match val { A(y) if y > 0 => ..., _ => ... }`
- **`uni` stays** — for single-arm unification, `uni` is less ceremony. `match` is for multi-arm.

### AST (Rust, `src/ast.rs`)
```rust
Expr::Match {
    value: Box<Expr>,
    arms: Vec<MatchArm>,
}

struct MatchArm {
    pattern: MatchPattern,  // VariantName(f1, f2) or Wildcard
    guard: Option<Expr>,    // if condition
    body: Expr,             // expression or block
}

enum MatchPattern {
    Wildcard,
    Variant { name: String, fields: Vec<String> },
}
```

### Grammar (add to `parse_primary` / new `parse_match_expr`)
```
match_expr = "match" expr "{" match_arm+ "}"
match_arm  = match_pattern ("if" expr)? "=>" (expr | block) ","
match_pattern = "_" | identifier ("(" identifier ("," identifier)* ")")?
```

### Interpreter (`src/interpreter.rs`)
```
Expr::Match { value, arms }:
    let target = eval_expr(value);
    for arm in arms:
        if arm.pattern is Wildcard → always matches
        if arm.pattern is Variant(name, fields):
            if target is Enum(_, name, enum_fields):
                bind fields from enum_fields
                if guard is None or eval_expr(guard) == true:
                    return eval_expr(body)
    → Error(non-exhaustive match) // should be caught at compile-time eventually
```

### Self-hosted (Briev-in-Briev)
- `token.bv`: Add `KeywordMatch`
- `lexer.bv`: Recognize `match`
- `ast.bv`: Add `ExprMatch(String, List<MatchArm>)`, `MatchArm { pattern: MatchPattern, guard: Expr, body: Expr }`, `MatchPattern { variant: String, fields: List<String> }`
- `parser.bv`: Parse match arms in `parse_expression()`

### Implementation Order
1. Rust AST (`src/ast.rs`)
2. Rust parser (`src/parser.rs`)
3. Rust interpreter (`src/interpreter.rs`)
4. Self-hosted side dependent on successful FFI bootstrap

---

## 2. FFI Overhaul

### Problem
Current FFI is a mess of:
- Hardcoded `ForeignFn` closures in `src/interpreter.rs` (36+ functions)
- Special-case built-ins (`is_ok`, `unwrap`, `clone`, `char_at`, `get`, `list_append`, `empty_map`)
- TOML binding files with inconsistent resolution
- DBVS warnings for malformed schemas
- `metropolitan_hub` subsystem that's disconnected from actual FFI resolution

### Philosophy
0. **No hardcoded frgns in the compiler.** Every FFI function is declared as `frgn name { args } -> Ret;` in a `.bv` file.
1. **TOML is dead.** FFI declarations are inline in `.bv` files, with optional `from "lib.so"` for shared library resolution.
2. **Efficiency is paramount.** Use `dlsym` direct calls for simple types (primitives, strings). Fall through to shared-memory metropolitan protocol only for complex types (lists, enums, structs).
3. **Terribly easy.** The declaration IS the binding. Write `frgn strlen { s: String } -> Int;` and it works — no config, no codegen step, no daemon.

### Architecture

#### Tier 1: Direct Dynamic Linking (default)
```
frgn strlen {
    s: String
} -> Int;
```

At resolution time:
1. Compiler finds the declaration
2. At interpreter startup OR first call, uses `libloading::Library::new("libc.so.6")` + `library.get::<...>("strlen")`
3. Auto-generated wrapper converts Briev args → C values (`String` → `*const c_char`, etc.)
4. Calls function pointer directly (~1-2ns overhead — same as native C call)
5. Converts return value back

**Supported types for direct ABI:**
- `Int` → `i64`
- `Float` → `f64`
- `Bool` → `i32` (0/1)
- `Char` → `u8`
- `String` → `*const c_char` (auto-null-terminated)
- `Void` → no return

#### Tier 2: Metropolitan Protocol (complex types)
```
frgn process_json {
    input: JsonValue  // complex
} -> JsonValue from "libprocessor" via metropolitan;
```

When types don't have a C ABI equivalent, uses shared memory + atomic handshake:
1. Compiler computes byte layout (`compute_layout()` from metropipe)
2. Opens `/dev/shm/metro_<name>`
3. Packs data at computed offsets
4. Atomic CAS handshake: `IDLE → REQ → ACK → RES → IDLE`
5. Unpacks result

The `via metropolitan` clause is explicit — Tier 1 (direct) is the default.

#### The Bootstrap Set
Exactly 4 built-in functions live in the interpreter binary — the minimum needed to load source files:

| Signature | Purpose |
|-----------|---------|
| `__read_file(path: String) -> Option<String>` | Load source code |
| `__write_file(path: String, data: String) -> Bool` | Write output |
| `__print(msg: String) -> Void` | Debug output |
| `__exit(code: Int) -> Void` | Termination |

Everything else (lexer `is_digit`, `is_alpha`, char operations, file I/O beyond read/write, math functions) lives in `lib/std/` as `frgn` declarations.

### Migration Plan

#### Phase A: Dynamic Linker Infrastructure
1. Add `libloading` to `Cargo.toml`
2. Create `src/ffi/dynamic.rs` — Dynamically resolves `frgn` calls at runtime
   - `resolve_function(name: &str, lib: &str) -> Result<RawFn, Error>`
   - `auto_wrap(params: &[Type], ret: &Type, fn_ptr: RawFn, args: &[Value]) -> Result<Value, Error>`
3. Wire into `Expr::Call` in interpreter: if definitions lookup fails and `frgn` declaration exists, try dynamic linking

#### Phase B: Frgn Declaration Resolution
1. Parse `frgn name { params } -> Ret [from "lib"];` declarations
2. Store in interpreter state
3. On call: look up frgn, resolve symbol, call with auto-wrap
4. Remove TOML binding loading from `load_program`

#### Phase C: Remove Special-Cases
1. Move `is_digit`, `is_alpha`, `is_alphanumeric`, `is_upper`, `is_lower`, `is_space`, `char_to_string` → `lib/std/char.bv` as `frgn` declarations
2. Move `clone`, `char_at`, `get`, `list_append` → `lib/std/` as definitions or frgn
3. Move `is_ok`, `is_err`, `unwrap`, `unwrap_err` → `lib/std/result.bv` as proper `frgn` declarations (they already work — just need to remove the interpreter special-case priority)
4. Delete the special-case code paths in `Expr::Call`

#### Phase D: Bootstrap
1. Remove all hardcoded `ForeignFn` closures from `load_ffi_functions()`
2. Keep only `__read_file`, `__write_file`, `__print`, `__exit`
3. Verify selfhost pipeline still runs — it should, since the compiler `.bv` files declare their own FFI needs

#### Phase E: Metropolitan Protocol (post-bootstrap)
1. Add `via metropolitan` clause parsing to `frgn` declarations
2. Integrate (or re-implement minimally) metropipe's channel protocol
3. Memory layout computation for complex types

### Files to Create / Modify

| File | Action |
|------|--------|
| `Cargo.toml` | Add `libloading` dependency |
| `src/ffi/dynamic.rs` | **NEW** — Dynamic linker resolution |
| `src/ffi/mod.rs` | Re-export `dynamic` module |
| `src/ast.rs` | Parse `ForeignBinding` with `from "lib"` |
| `src/parser.rs` | Parse `frgn name { params } -> Ret [from "lib"];` |
| `src/interpreter.rs` | Remove special-cases; wire dynamic linking |
| `src/main.rs` | Update `run_selfhost` for new FFI |
| `lib/std/char.bv` | Add `frgn is_digit(c: Char) -> Bool;` etc. |
| `lib/std/result.bv` | Add proper `frgn is_ok / unwrap` declarations |
| `std/bindings/*.toml` | **DELETE** — no longer needed |
| `std/bindings/*.dbvs` | **DELETE** — no longer needed |

### Precedent / Inspiration
- **Metropipe** — `compute_layout()` for byte-offset ABI, atomic handshake protocol
- **Rust `libloading`** — safe `dlsym` wrapper, already well-tested
- **Python ctypes** — same "declare signature, call dynamically" pattern
- **JNI / CNI** — same auto-wrapping concept, simpler because we control both sides

---

## Implementation Order

1. ~~SESSION-ALREADY-COMPLETED: Pipeline fixes (Result priority, list_append, Bool guard)~~ ✅
2. **Match expression** (Rust AST + parser + interpreter) — self-contained, immediately testable
3. **Dynamic linker** (libloading + auto-wrap) — core FFI infrastructure
4. **Frgn declaration resolution** — parse + wire into interpreter
5. **Remove special-cases** — migrate built-ins to `lib/std/`
6. **Bootstrap reduction** — remove ForeignFn closures, keep only 4
7. **Self-hosted parity** — update token.bv, lexer.bv, parser.bv, ast.bv

---

## Anti-Patterns (DO NOT DO)

1. **Do not add more hardcoded built-ins.** Every new FFI function must be a `frgn` declaration in a `.bv` file.
2. **Do not use TOML for anything new.** TOML was a pragmatic bridge. The future is inline `frgn` declarations + optional `from "lib.so"`.
3. **Do not pre-populate interpreter state** with enum constructors (`None`, `Some`, `Ok`, `Err`) or function definitions. The standard library handles this.
4. **Do not weaken contracts** to match lazy code. Fix the code, not the contract.
5. **Do not add Rust string-match "built-in" functions** for things the standard library provides.
