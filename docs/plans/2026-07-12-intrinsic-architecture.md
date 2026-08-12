# Phase 8G: `#` Intrinsic Architecture

**Date:** 2026-07-12
**Replaces:** Phase 10 from `big-rewrite-execution.md`
**Thesis:** `docs/architecture/bits-thesis.md`
**Status:** Execution

---

## The Core Idea

`#` is a first-class lexical character in identifiers. `Sqrt#(x)` is parsed as a
standard `Expr::Call(Expr::Identifier("Sqrt#"), [x])` — no `Intrinsic` enum,
no `Expr::IntrinsicCall` AST node, no special parser rules. The `#` suffix IS
the intrinsic marker.

### The Rules

1. Every intrinsic is a `defn`-like compiler primitive that has **no user-defined
   body**. The interpreter evaluates it via a hardcoded `execute_intrinsic()`
   dispatch table operating on `Value::Bits`.
2. An intrinsic is something **every backend is expected to accelerate natively**.
   If a backend encounters an intrinsic it doesn't handle → compilation error.
3. The compiler's single source of truth for intrinsic types is
   `get_intrinsic_signature(name)` — one match arm per intrinsic.
4. Type-specific rebinding uses the existing `get_operator_intrinsic()` system:
   `bind Complex<Float> { Sqrt# <~ complex_sqrt };` uses the exact same `op`
   binder syntax as `op Add <~ add_i64`.
5. The interpreter **never special-cases** `#` calls — it evaluates them via
   the same `Call` dispatch path, calling `execute_intrinsic()` when the callee
   name ends with `#`.
6. The `.dbvl` semantic archive carries frontend metadata properties for each
   intrinsic call (e.g., `fastmath <~ true`). Backends read these from the
   archive and parameterize their codegen templates.

### The Unification

```
Operator            Intrinsic            Type-specific overload
─────────           ──────────           ─────────────────────
`a + b`             `Sqrt#(x)`           `Sqrt#(x: Complex)`

    │                    │                        │
    v                    v                        v
  Op to fn             Identifier              Identifier
  binding              "Sqrt#"                 "Sqrt#"
  add_i64              (no user defn)          op-rebound to
                                                complex_sqrt
    │                    │                        │
    +────────────+───────┴────────+───────────────+
                 │
                 v
        Expr::Call(name, args)
                 │
     ┌───────────┴───────────┐
     │                       │
     v                       v
  name.ends_with("#")      name is regular function
     │                       │
     v                       v
  execute_intrinsic()     evaluate defn body
  (interpreter)           (interpreter)
  ─────────────           ─────────────
  backend: look up        backend: emit call
  in .dbvl bindings       to function body
  or error
```

---

## Non-Negotiable Rule: Max 2 Nesting Depth

**Every function in every file touched by this phase must have max 2 levels of
indentation.** This is the same rule from the Big Rewrite (Phase 0–7). Any
code that violates it must be rewritten with guard clauses, `?`, and extracted
helpers before the phase is considered complete.

| ✅ Allowed | ❌ Forbidden |
|---|---|
| `let val = expr?;` | `if let Some(v) = opt { if let Ok(x) = v { ... } }` |
| `if !condition { return; }` | `if cond { if other { ... } else { ... } }` |
| `match v { A => a, B => b }` | `match x { A => match y { B => ..., C => ... } }` |
| `for item in items { helper(item)?; }` | `for item in items { if let Some(x) = item { ... } }` |
| Helper functions extracted from match arms | Match arms with >10 lines inline |

Violations from the existing codebase must be fixed during this phase, not
deferred. Every new function must be flat by construction.

---

## What Gets Deleted (~730 lines)

| Item | File | Est. Lines |
|---|---|---|
| `Intrinsic` enum (all variants + impl) | `src/ast.rs` | ~120 |
| `Expr::IntrinsicCall` AST variant | `src/ast.rs` | ~5 |
| `intrinsic_dispatch.rs` (entire module) | `src/interpreter/` | ~500 |
| `inop` keyword + `#` special parsing | `src/lexer.rs`, `src/parser.rs` | ~30 |
| `TopLevel::Inop` / `InopDeclaration` | `src/ast.rs` | ~30 |
| `Expr::IntrinsicCall` match arms (~8 sites) | typechecker, backends, desugarer, etc. | ~40 |
| `Intrinsic::name()` + helpers | `src/ast.rs` | ~10 |
| **Total** | | **~730** |

---

## Execution Order

### Step 1: Lexer (`src/lexer.rs`)

- Allow trailing `#` in identifier character class:
  ```
  Identifier ::= [a-zA-Z][a-zA-Z0-9_]*#?
  ```
- Remove `#` as a standalone token variant.
- Remove `inop` keyword token.
- Remove `Hash` from the `Token` enum if it exists.

### Step 2: AST (`src/ast.rs`)

- Delete the entire `Intrinsic` enum definition + all helper impls (`name()`,
  `from_str()`, `Display`, any `#[cfg(test)]` builders, etc.).
- Delete `Expr::IntrinsicCall { intrinsic: Box<Intrinsic>, args: Vec<Expr> }`
  (or `IntrinsicCall(Intrinsic, Vec<Expr>)`) from the `Expr` enum.
- Delete `TopLevel::Inop(InopDeclaration)` if it exists.
- Delete `InopDeclaration` struct if it exists.
- Delete `Statement::InopDef` or similar.
- Fix all `#[cfg(test)]` arms in the same file that reference deleted types.

### Step 3: Parser (`src/parser.rs`)

- Remove `inop` keyword token from the token matching.
- Remove `#` suffix as a special grammar rule — `Sqrt#(x)` now parses
  naturally as `Call(Identifier("Sqrt#"), [x])` because `#` is part of the
  identifier.
- Remove all `Intrinsic` token references.
- Remove any regex-based `#` splitting logic.
- If there's a `parse_intrinsic_call()` function, delete it.
- Update the `parse_call()` or equivalent function to handle the `#`-ending
  identifier as a normal callee name.
- Fix all tests in the same file that construct `Expr::IntrinsicCall`.

### Step 4: Intrinsic Signature Registry (new: `src/intrinsic_signatures.rs`)

One flat match function. Every arm is `name ⇒ Signature { params, ret }`.
No nested logic — each arm is a one-liner.

```rust
/// Compiler's built-in signature registry for `#` intrinsics.
/// One arm per intrinsic. Single source of truth for type-checking.
pub fn get_intrinsic_signature(name: &str) -> Option<Signature> {
    match name {
        "Sqrt#"    => Some(math_unary(Type::float(), Type::float())),
        "Sin#"     => Some(math_unary(Type::float(), Type::float())),
        "Cos#"     => Some(math_unary(Type::float(), Type::float())),
        "SHA256#"  => Some(hash_sig()),
        "Malloc#"  => Some(alloc_sig()),
        "Free#"    => Some(free_sig()),
        // ... one arm per intrinsic
        _ => None,
    }
}

// Helper constructors (flat, 2 lines each)
fn math_unary(p: Type, r: Type) -> Signature {
    Signature { params: vec![p], ret: r, ... }
}
fn hash_sig() -> Signature {
    Signature { params: vec![Type::string()], ret: Type::string(), ... }
}
```

This file is added to `src/lib.rs` (or `src/interpreter/mod.rs` if it's
interpreter-only — but the type-checker also needs it, so it should be at the
crate level or in `type_universe/`).

### Step 5: Type-checker (`src/typechecker.rs`)

In `infer_expression()` → `Expr::Call(name_expr, args)` arm (around
line ~3700 or wherever the Call match is):

```
1. Extract callee name from the identifier.
2. If name.ends_with("#"):
   a. Infer the type of the first argument (to support type-specific rebinding).
   b. Check get_operator_intrinsic(arg_type, name) for type-specific override.
   c. If found → use the rebound function's signature.
   d. If not found → check get_intrinsic_signature(name).
   e. If neither → compilation error: "unknown intrinsic '{name}'".
   f. Validate arg types against the signature. Return the return type.
3. Otherwise: existing function call logic (TypeUniverse lookup, etc.).
```

Remove the old `Expr::IntrinsicCall` match arm entirely.

**Flat code requirement:** Each of steps 2a–2f is a flat guard clause or
function call. No nested `if let` chains.

### Step 6: Interpreter — eval.rs (`src/interpreter/eval.rs`)

In `eval_expr()` → `Expr::Call(name_expr, args)` arm:

```
1. Evaluate callee expression → Value (expected to be an identifier string).
2. Extract name from the evaluated callee.
3. Evaluate all argument expressions → Vec<Value>.
4. If name.ends_with("#"):
   a. Call execute_intrinsic(name, &evaluated_args).
   b. Return the result.
5. Otherwise: existing function call evaluation (look up defn, check
   oracle fuel, evaluate body, etc.).
```

Remove the existing `Expr::IntrinsicCall { intrinsic, args }` match arm.

**Flat code requirement:** Each step is a guard clause or single function call.
No match-inside-match-inside-match.

### Step 7: Interpreter — intrinsics.rs (`src/interpreter/intrinsics.rs`)

The existing `execute_intrinsic(name, &[Value])` function currently handles
only `__add_i64`, `__sub_i64`, `__mul_i64`, `__eq_i64`, `__fadd_f64`.
Extend it to cover ALL `#` intrinsics.

Structure:
```rust
pub fn execute_intrinsic(name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
    match name {
        "Sqrt#"   => eval_math_unary_float(args, |f| f.sqrt(), "Sqrt#"),
        "Sin#"    => eval_math_unary_float(args, |f| f.sin(), "Sin#"),
        "Cos#"    => eval_math_unary_float(args, |f| f.cos(), "Cos#"),
        "Add#"    => eval_math_binary_int(args, |a, b| a.wrapping_add(b), "Add#"),
        // ... one arm per intrinsic
        _ => Err(RuntimeError::UnknownIntrinsic(name.into())),
    }
}
```

Move the helper functions from `intrinsic_dispatch.rs` into this file:
- `eval_unary_float(values, op, name)`
- `eval_unary_int(values, op, name)`
- `eval_string_*` helpers
- `eval_sha256`, `eval_base64_encode`, etc.

Each helper is max 2 levels: guard clause → compute → return.

**Delete `intrinsic_dispatch.rs` entirely.** Its `eval_intrinsic(&mut self,
&Intrinsic, &[Expr])` function is replaced by `execute_intrinsic(&str,
&[Value])` — the `&mut self` is gone because `execute_intrinsic` doesn't need
interpreter state (it operates on already-evaluated `Value`s). If any intrinsic
DOES need interpreter state (e.g., `Alloc#` needs `self.virtual_heap`), pass
the needed state explicitly as a parameter rather than `&mut self`.

**Also in this file:** `bits_to_i64`, `i64_to_bits`, `value_as_i64`,
`value_as_bool`, `value_as_f64`, `bits_to_f64`, `f64_to_bits` — these stay
here unchanged. They are the byte-conversion primitives used by all intrinsics.

### Step 8: Type-specific op rebinding (`src/type_universe/operators.rs`)

Extend `get_operator_intrinsic(type_name, op_name)` to handle `#` names.

Currently:
```rust
get_operator_intrinsic("Int", "Add") => Some("__add_i64")
```

Now also:
```rust
get_operator_intrinsic("Complex<Float>", "Sqrt#") => Some("complex_sqrt")
```

No new infrastructure. The same `op` binder that maps symbols to function names
also maps `#` names to function names. The Briev syntax is:

```briev
bind Complex<Float> {
    Sqrt# <~ complex_sqrt;
};
```

The parser already handles `op Add <~ add_i64` — this is the same mechanism
with a `#`-ending name instead of a symbol.

**Resolution order** (used by type-checker, interpreter, and backends):

```
Call("Sqrt#", arg: T)
  1. get_operator_intrinsic(T, "Sqrt#")
     → found: use rebound function (name, signature, body)
     → not found: fall through
  2. get_intrinsic_signature("Sqrt#")
     → found: use global signature, interpreter dispatch
     → not found: compilation error "unknown intrinsic 'Sqrt#'"
```

### Step 9: Backend bindings (`.dbvl` files, one per backend)

Each backend maintains a `.dbvl` file mapping `#` names to codegen templates.

**`src/backend/llvm/bindings.dbvl`:**
```
"Sqrt#" | "call double @llvm.sqrt.f64(double %0)"
"Sin#"  | "call double @llvm.sin.f64(double %1)"
"Add#"  | "add i64 %0, %1"
```

In `emit_call(name, args, metadata)`:
```
1. If name.ends_with("#"):
   a. Read frontend metadata from the semantic .dbvl archive
      (fastmath, precision, rounding mode, etc. — these are properties
       attached to the call site, not to the intrinsic definition).
   b. Look up name in backend's .dbvl bindings file.
   c. Found → instantiate template with metadata parameters.
      Example: fastmath=true → "call @llvm.sqrt.f64"
               fastmath=false → "fcmp + select + precise sequence"
   d. Not found → compilation error (every backend must accelerate
      all intrinsics).
2. Otherwise: existing function call codegen.
```

### Step 10: SMT proof engine (`src/proof_engine.rs`)

Map `#` intrinsics to Z3's native theories:

```rust
fn intrinsic_to_z3(name: &str, args: &[Z3Expr]) -> Option<Z3Expr> {
    match name {
        "Sqrt#"   => Some(z3_fp_sqrt(args[0])),
        "F64ToI64#" => Some(z3_fp_to_sbv(args[0], 64)),
        "I64ToF64#" => Some(z3_sbv_to_fp(args[0])),
        "Add#"    => Some(z3_bvadd(args[0], args[1])),
        // ...
        _ => None,
    }
}
```

One arm per intrinsic. Each arm is a one-liner. Flat.

### Step 11: Remove dead code across all remaining files

Build the compiler. Every `Intrinsic::Foo`, `Expr::IntrinsicCall`, and `inop`
reference that survives produces a compile error. Fix each by:

| Pattern | Replace with |
|---|---|
| `Intrinsic::Sqrt` in match | Remove match arm (unless logic depends on it — then use `execute_intrinsic("Sqrt#", ...)` instead) |
| `Expr::IntrinsicCall { intrinsic, args }` | Remove match arm or replace with `Expr::Call(ident, args)` |
| `TopLevel::Inop(..)` | Remove match arm |
| `inop` in any doc comment | Replace with `defn` or remove |
| `Intrinsic::name()` | Use string literal (the name IS the discriminant now) |

Files known to have match sites:
- `src/desugarer.rs` — desugars `IntrinsicCall` → check if still needed
- `src/backend/llvm/mod.rs` — intrinsic codegen
- `src/backend/webstack.rs` — intrinsic codegen
- `src/backend/circt.rs` — intrinsic codegen
- `src/normalize_types.rs` — normalizes intrinsic expressions
- `src/features/traits.rs` — `ExprDispatch`/`ExprEval` impls
- `src/analysis/equality_saturation.rs` — expression rewriting
- `src/interpreter/mod.rs` — any remaining `Intrinsic::Foo` references outside
  the submodules
- `src/interpreter/cells.rs` — `Intrinsic::TtyReadKey` etc. references
- `src/interpreter/ffi.rs` — any `Intrinsic` references that slipped through

**Each fix must produce flat code (max 2 nesting).** If removing a match arm
reveals a surrounding function with >2 levels of nesting, extract the function's
body into a named helper.

### Step 12: Clean up the `.dbvl` semantic pipeline

Ensure the compiler outputs a `.dbvl` file containing:
- Every function call node with callee name and argument types.
- For `#` calls: the call site metadata properties (`fastmath`, etc.).
- The type universe (for backend type lookups).

This likely already works (the compiler outputs `.dbvl`). Verify that the
metadata properties for `#` calls survive the compilation pipeline
(parser → type-checker → normalization → emission).

---

## Verification

```bash
cargo build                                          # 0 errors, 0 warnings
cargo test --lib                                     # same count as pre-change
```

```bash
# Audit: no deleted constructs remain in production code
grep -rn "Intrinsic::" src/ --include="*.rs" | grep -v archive | grep -v "// " | grep -v "^Binary" || echo "CLEAN"
grep -rn "Expr::IntrinsicCall" src/ --include="*.rs" | grep -v archive | grep -v "// " || echo "CLEAN"
grep -rn "mod intrinsic_dispatch" src/ --include="*.rs" || echo "CLEAN"
grep -rn "\binop\b" src/ --include="*.rs" | grep -v archive | grep -v "// " | grep -v "binary_op\|UnaryOp\|BinOp\|Inop" || echo "CLEAN"
```

```bash
# Audit: max 2 nesting depth on new/changed files
praetor --max-depth 2 src/interpreter/eval.rs src/interpreter/intrinsics.rs src/intrinsic_signatures.rs \
  src/backend/llvm/mod.rs src/typechecker.rs
```

```bash
# Full benchmark suite — no regressions
bash benchmarks/build_and_bench.sh --correctness
```

---

## Summary of File Changes

| File | Change |
|---|---|
| `src/lexer.rs` | `#` → identifier char; remove `inop` token |
| `src/ast.rs` | Delete `Intrinsic` enum, `Expr::IntrinsicCall`, `InopDeclaration` |
| `src/parser.rs` | Remove `inop` parsing, `#` special rules |
| `src/intrinsic_signatures.rs` | **NEW** — signature registry (one match arm per intrinsic) |
| `src/typechecker.rs` | `Call` arm checks `#` suffix, uses signature registry |
| `src/interpreter/eval.rs` | `Call` arm dispatches to `execute_intrinsic` for `#` names |
| `src/interpreter/intrinsics.rs` | Expand `execute_intrinsic` to cover all `#` intrinsics |
| `src/interpreter/intrinsic_dispatch.rs` | **DELETE** — replace all uses |
| `src/interpreter/cells.rs` | Remove `Intrinsic::TtyReadKey` etc. → direct `ffi::` calls |
| `src/interpreter/ffi.rs` | Remove any `Intrinsic` references |
| `src/type_universe/operators.rs` | `get_operator_intrinsic` handles `#` names |
| `src/backend/llvm/mod.rs` | `#` calls → `.dbvl` bindings lookup or error |
| `src/backend/llvm/bindings.dbvl` | **NEW** — LLVM codegen templates per intrinsic |
| `src/backend/webstack.rs` | Same pattern as LLVM |
| `src/backend/circt.rs` | Same pattern as LLVM |
| `src/proof_engine.rs` | Map `#` intrinsics to Z3 theories |
| `src/desugarer.rs` | Remove `IntrinsicCall` arm |
| `src/normalize_types.rs` | Remove `IntrinsicCall` arm |
| `src/features/traits.rs` | Remove `IntrinsicCall` arm if present |
| `src/analysis/equality_saturation.rs` | Remove `IntrinsicCall` arm if present |
| `docs/plans/2026-07-11-big-rewrite-execution.md` | Replace Phase 10 section with reference to this doc |
| `AGENTS.md` | Remove `intrinsic_op <~` from convention list; add `#` convention |
| `docs/architecture/features/metadata-dispatch.md` | Add `#` suffix rule to identifier-vs-string section |

---

## The Final State

After Phase 8G:

- **No `Intrinsic` enum.** The compiler never matches on a variant name like
  `Intrinsic::Sqrt`. It matches on the string `"Sqrt#"`.
- **No `Expr::IntrinsicCall`.** `Sqrt#(x)` is `Call("Sqrt#", [x])`.
- **No `inop`.** All opaque bodies are gone. If a function needs no body
  (intrinsic), it is NOT declared in source — the compiler knows it via the
  signature registry.
- **No `#` special parsing.** `#` is just a character in identifiers.
- **No hardcoded backend strings in the standard library.** Backend codegen
  lives in backend-local `.dbvl` bindings files.
- **The `.dbvl` semantic archive is the pipeline.** Compiler output = `.dbvl`.
  Backend input = `.dbvl`. Bindings = `.dbvl`. One format everywhere.
- **Operators, intrinsics, and type-specific overloads are the same mechanism.**
  The `op` binder maps symbols OR `#` names to function names. The resolution
  chain is identical.
- **Every function is flat (max 2 nesting).** This phase produces no arrow code.
