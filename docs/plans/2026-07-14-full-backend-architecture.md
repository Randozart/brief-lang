# Full Backend Architecture — Detachment, Operations, and Type Derivation

## The Three-Layer Architecture

```
Layer 1: BACKEND DETACHMENT
  Compile pipeline dispatches by backend name. Zero hardcoded LlvmBackend.
  └── Layer 2: OPERATIONS VS INTRINSICS
        Syntax maps to config-driven ops. # calls stay as backend builtins.
        └── Layer 3: BACKEND CHOOSES TYPE DERIVATION
              Backend reads primitive, bytes, or other metadata as it chooses.
```

## Layer 1: Backend Detachment

### Problem
`compile_source()` hardcodes `LlvmBackend::new().generate()`. CIRCT and Webstack backends exist but are never called.

### Target
A `--backend` CLI flag selects the backend. The config file maps file extension to default backend. `compile_source()` dispatches by backend name — zero mentions of `LlvmBackend`.

### Config file: `config/targets.toml`

```toml
[.bv]
backend = "llvm"
defaults = ["--budget", "256"]

[.ebv]
backend = "llvm"
defaults = ["--optimize-size", "--budget", "0"]

[.cbv]
backend = "circt"
defaults = []

[.wbv]
backend = "webstack"
defaults = ["--target", "wasm"]
```

Compiled into the binary via `include_str!`. The compiler reads the extension from the input file, looks up the table, resolves the backend name to a `BackendKind` enum.

### CLI flag precedence

`briv build file.ebv --backend llvm` — explicit `--backend` overrides the config file.

### Dispatch in `compile_source()`

```rust
fn compile_source(file_path, source, opts) {
    let (items, universe) = parse_and_check(file_path, source)?;
    let (items, universe) = run_plugin_chain(items, universe, opts)?;
    match opts.backend {
        BackendKind::Llvm => {
            let mut b = LlvmBackend::new().with_type_universe(universe);
            if opts.optimize_budget > 0 { b = b.with_optimize_budget(opts.optimize_budget); }
            let output = b.generate(&items, None)?;
            write_output(file_path, output, ".ll")?;
            if !opts.emit_ir_only { compile_ll_to_binary(output_path)?; }
        }
        BackendKind::Circt => {
            let output = CirctBackend::new().with_type_universe(universe).generate(&items)?;
            write_output(file_path, output, ".mlir")?;
        }
        BackendKind::Webstack => {
            let output = WebstackGenerator::new().generate(&items, &[], "program")?;
            write_output(file_path, output.typescript, ".ts")?;
        }
    }
    Ok(())
}
```

### What changes

| File | Change | Lines |
|------|--------|-------|
| `config/targets.toml` | **New** — extension → backend routing | ~15 |
| `src/target.rs` | **New** — `TargetConfig`, `BackendKind`, `resolve_backend()` | ~60 |
| `src/compile.rs` | Replace `LlvmBackend::new()` with `match opts.backend { ... }` | ~30 |
| `src/main.rs` | Add `--backend` flag, read extension → look up target config | ~15 |
| `src/backend/mod.rs` | Add `Backend` trait (optional, or just dispatch match) | ~20 |
| `src/backend/circt.rs` | Implement `Backend` for `CirctBackend` | ~15 |
| `src/backend/webstack.rs` | Implement `Backend` for `WebstackGenerator` | ~20 |

## Layer 2: Operations vs Intrinsics

### Distinction

| Kind | Defined by | Syntax | Dispatch |
|------|-----------|--------|----------|
| **Operation** | Backend config file (`llvm-ops.toml`) | Briv syntax: `+`, `==`, `++` | Config lookup: `(op, primitive, bytes)` |
| **Intrinsic** | Backend (hardware or builtin) | `Name#(args)` call syntax | Backend chooses: lookup, fallback, error |
| **Override** | Source type definition | `op Add <~ custom_fn(#L, #R)` | Type registry lookup before config |

### Operation dispatch

```
Expr::BinaryOp(Add, lhs, rhs)
  │
  ├─ Type override exists for (lhs.ty, rhs.ty)?
  │    └─ Yes: emit call to override function with #L, #R
  │
  ├─ Same-type override exists on lhs.ty?
  │    └─ Yes: emit call to override function
  │
  ├─ Config lookup: (Add, primitive, bytes)?
  │    └─ Yes: emit LLVM IR template (replace %v, %a, %b)
  │
  └─ Fallback: error — operation not supported for type
```

### Config file: `config/llvm-ops.toml`

```toml
[op.Add.Int]
1 = "add nsw i8 %a, %b"
2 = "add nsw i16 %a, %b"
4 = "add nsw i32 %a, %b"
8 = "add nsw i64 %a, %b"

[op.Add.Float]
4 = "fadd float %a, %b"
8 = "fadd double %a, %b"

[op.Eq.Int]
1 = "icmp eq i8 %a, %b"
2 = "icmp eq i16 %a, %b"
4 = "icmp eq i32 %a, %b"
8 = "icmp eq i64 %a, %b"

[op.Eq.Float]
4 = "fcmp oeq float %a, %b"
8 = "fcmp oeq double %a, %b"

[op.Eq.Bool]
1 = "icmp eq i8 %a, %b"

[op.Eq.Char]
4 = "icmp eq i32 %a, %b"

[op.Sqrt.Float]
4 = "call float @llvm.sqrt.f32(float %a)"
8 = "call double @llvm.sqrt.f64(double %a)"

[op.Concat.String]
8 = "call ptr @string_concat(ptr %a, ptr %b)"

[op.Print.Int]
8 = "call i32 (ptr, ...) @printf(ptr @.fmt_int, i64 %a)"

[op.Print.Float]
8 = "call i32 (ptr, ...) @printf(ptr @.fmt_float, double %a)"

[op.Print.String]
8 = "call i32 (ptr, ...) @printf(ptr @.fmt_str, ptr %a)"

[op.Malloc]
8 = "call ptr @malloc(i64 %a)"

[op.Free]
8 = "call void @free(ptr %a)"

[op.Memcpy]
8 = "call ptr @memcpy(ptr %a, ptr %b, i64 %c)"

[op.Memset]
8 = "call ptr @memset(ptr %a, i64 %b, i64 %c)"
```

Template variables: `%v` = result register, `%a` = first operand, `%b` = second, `%c` = third.

### Source-level override syntax

Inside a type body (same-type override):

```briv
type Int : Bits {
    bytes <~ 8;
    primitive <~ Int;
    op Add <~ add_int_int(#L, #R);
}
```

Top-level (cross-type override):

```briv
op Add(Int, String) <~ add_int_string(#L, #R);
op Add(String, Int) <~ add_string_int(#L, #R);
```

These register into the TypeUniverse as operation overrides keyed by `(op_name, lhs_type_name, rhs_type_name)`.

### Intrinsic collapse

Replace 38 type-specific intrinsics with 15 generic operations.

**Removed:**
AddI64#, SubI64#, MulI64#, DivI64#, RemI64#, EqI64#, NeI64#, LtI64#, GtI64#, LeI64#, GeI64#,
FAddF64#, FSubF64#, FMulF64#, FDivF64#, FEqF64#, FLtF64#, FGtF64#, FLeF64#, FGeF64#,
EqI1#, EqI32#, FloatToInt#, IntToFloat#, IntToString#, FloatToString#, CharToInt#, IntToChar#,
StringConcat#, StringLength#, StringEq#, ListInsert#, ListGet#, Len#,
PrintInt#, PrintFloat#, PrintString#, GetEnvInt#, GetEnvString#

**Added (generic):**
Add#, Sub#, Mul#, Div#, Rem#, Eq#, Ne#, Lt#, Gt#, Le#, Ge#, Neg#, Abs#,
Sqrt#, Sin#, Cos#, Fabs#, Ceil#, Floor#, Pow#,
ToInt#, ToFloat#, ToString#, Concat#, Length#, Get#, Insert#,
Print#, Malloc#, Free#, Memcpy#, Memset#, GetEnv#,
GetGlobalId#, GetGlobalSize#, GetLocalId#

Each generic operation has `observable: true/false` and no fixed type signature — the type is inferred from arguments.

### Interpreter dispatch

```rust
fn execute_intrinsic(name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
    let ty = infer_type_from_args(args);
    match (name, ty) {
        ("Add#", Type::Int) => {
            let a = args[0].as_i64()?;
            let b = args[1].as_i64()?;
            Ok(Value::from(a.wrapping_add(b)))
        }
        ("Add#", Type::Float) => {
            let a = args[0].as_f64()?;
            let b = args[1].as_f64()?;
            Ok(Value::from(a + b))
        }
        ("Eq#", Type::Bool) => {
            let a = args[0].as_i64()? != 0;
            let b = args[1].as_i64()? != 0;
            Ok(Value::from(a == b))
        }
        _ => Err(RuntimeError::UnsupportedOperation(...)),
    }
}
```

### What changes

| File | Change | Lines |
|------|--------|-------|
| `config/llvm-ops.toml` | **New** — operation → LLVM IR template | ~80 |
| `config/circt-ops.toml` | **New** — CIRCT operation lowering | ~80 |
| `config/webstack-ops.toml` | **New** — Webstack operation lowering | ~80 |
| `src/config.rs` | **Extend** — add `OpConfig`, `lookup(op, prim, bytes)` | ~40 |
| `src/intrinsic_signatures.rs` | **Rewrite** — 38→15 generic ops | ~30 |
| `src/backend/llvm/intrinsics.rs` | **Rewrite** — config-driven dispatch | ~80 |
| `src/backend/llvm/emit_expr.rs` | **Extend** — call `emit_op_call` for `BinaryOp` | ~20 |
| `src/interpreter/intrinsics.rs` | **Rewrite** — generic op dispatch by inferred type | ~100 |
| `src/interpreter/mod.rs` | **Extend** — `infer_type_from_args()` | ~20 |
| `src/parser/definitions.rs` | **Extend** — `op Add <~ fn(#L, #R)` syntax | ~20 |
| `src/type_universe/mod.rs` | **Extend** — `lookup_op(lhs, rhs, op)` | ~20 |
| All `.bv` files | **Rename** — mechanical `AddI64#` → `Add#` etc. | ~200 files |

## Layer 3: Backend Chooses Type Derivation

### Principle
The compiler provides ALL metadata. The backend selects what it needs.

### Backend type derivation approaches

| Approach | What the backend reads | When it's used |
|----------|----------------------|----------------|
| **`primitive` + `bytes`** | `ResolvedType.properties["primitive"]` + `ResolvedType.bytes` | LLVM default — standard type dispatch |
| **Raw `bytes` only** | `ResolvedType.bytes`, ignores `properties` entirely | CIRCT, GPU — hardware doesn't need semantic types |
| **`primitive` required** | Errors if `properties["primitive"]` is missing | Embedded/strict mode (`--strict-metadata`) |
| **Other metadata** | `encoding`, `alignment`, custom properties | Webstack, framework plugins, cross-language interop |

### How the backend reads type info

```rust
// Backend receives ResolvedType { bytes, properties, ... }
// It chooses what to read:

fn resolve_type_info(rt: &ResolvedType) -> TypeInfo {
    // Option A: primitive + bytes (LLVM)
    if let Some(prim) = rt.primitive() {
        if let Some(llvm) = config.lookup(prim, rt.bytes) {
            return TypeInfo { llvm: llvm.to_string(), bytes: rt.bytes };
        }
    }
    // Option B: bytes only (CIRCT, GPU — always works)
    TypeInfo { llvm: format!("i{}", rt.bytes * 8), bytes: rt.bytes }
}
```

### The `primitive` property

Declared in source:
```briv
type Int : Bits { bytes <~ 8; primitive <~ Int; }
```

Stored in `ResolvedType.properties["primitive"]` as `PropertyValue::Identifier("Int")`.

Read by the backend via `rt.primitive()` which returns `Option<&str>`.

If absent, the backend falls back to bytes-only resolution. This is always correct.

### What changes

| File | Change | Lines |
|------|--------|-------|
| `docs/architecture/backend-type-dispatch.md` | Already documents this — update with operation dispatch | ~30 |

## Coding Standards (Every Function)

1. **Max 2 nesting levels deep.** Extract helper functions. Use guard clauses.
2. **Doc comments** on every `fn`, `struct`, `enum`, `mod`.
3. **`// 2026-07-14:` comments** explaining why each change exists at every modification site.
4. **No `else-if` chains deeper than 1.** Use early returns:
   ```rust
   if condition { return A; }
   if condition { return B; }
   C
   ```
5. **HashMap determinism.** Sort before iterating for LLVM IR emission.
6. **Never weaken existing optimization paths.** Additional match arms only.

## Execution Order

| Step | Description | Est. time |
|------|-------------|-----------|
| 1 | Create `config/targets.toml` + `src/target.rs` | 15 min |
| 2 | Add `--backend` flag to main.rs + BuildOptions | 10 min |
| 3 | Create `config/llvm-ops.toml` with all operation entries | 20 min |
| 4 | Add `OpConfig` to `src/config.rs` | 10 min |
| 5 | Collapse `intrinsic_signatures.rs`: 38 → 15 generic ops | 10 min |
| 6 | Mechanical rename across all `.bv` files | 15 min |
| 7 | Replace `emit_intrinsic_call` with config-driven dispatch | 30 min |
| 8 | Update `execute_intrinsic()` for generic typed dispatch | 20 min |
| 9 | Add `op Add <~ fn(#L, #R)` parser + universe support | 20 min |
| 10 | Wire type-level overrides into operation dispatch | 15 min |
| 11 | Dispatch match in `compile_source()` | 15 min |
| 12 | CIRCT + Webstack Backend impl | 20 min |
| 13 | Create CIRCT + Webstack ops config files | 15 min |
| 14 | Create `config/circt-ops.toml` and `config/webstack-ops.toml` | 15 min |
| 15 | Update architecture doc | 10 min |
| 16 | Final test: all tests pass | 15 min |

Total estimated time: ~4 hours.
