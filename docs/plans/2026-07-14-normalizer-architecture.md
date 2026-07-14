# Normalizer Architecture — Backend-Specific AST Annotation

## The Problem

The backend currently reads config files and matches on `primitive` + `bytes` at every expression node. This is repeated work: every `Add#` call, every `Sqrt#` call, every type reference walks the same config-driven decision tree. CIRCT doesn't use `primitive` at all but still sees it on every node. LLVM re-derives `"double"` from `primitive=Float, bytes=8` hundreds of times per compilation.

## The Solution

A **normalizer** pass runs between the plugin chain and the codegen backend. It walks the entire AST once, reads the backend's config files, and **attaches annotations** to every node that would otherwise need config lookup. The backend never reads config files and never matches on `primitive` — it only reads pre-attached annotations.

The backend still owns all structural transformation (phi nodes for LLVM, register chains for CIRCT, reactive loop wiring). The normalizer only pre-bakes the config-driven decisions.

## The Pipeline

```
Source
  │
  ▼ Frontend (parse, resolve, first-pass optimizations)
  │  - Strips contracts
  │  - Emits bounded-ness metadata
  │  - Wires reactive components
  │  - Output: rich Brief AST with all metadata attached
  │
  ▼ [PLUGIN CHAIN (optional)]
  │  - User-supplied transforms
  │  - Can add/modify metadata, rewrite sections
  │  - Output: modified AST
  │
  ▼ NORMALIZER (per-backend, configured in targets.toml)
  │  Reads: config/<backend>-primitives.toml, config/<backend>-ops.toml
  │  Walks the AST once:
  │    - Attaches resolved LLVM type to every type reference
  │    - Resolves generic operations to concrete backend lowerings
  │    - Resolves intrinsics where obvious (Sqrt# + Float → llvm.sqrt.f64)
  │    - Strips metadata the backend doesn't use
  │  Output: annotated AST (same structure, richer nodes)
  │
  ▼ BACKEND (codegen)
  │  - Reads pre-resolved annotations from AST nodes
  │  - Performs its own structural transformations
  │    (phi nodes for LLVM, registers for CIRCT, reactive wiring)
  │  - NEVER reads config files
  │  - NEVER matches on primitive/bytes
  │  Output: LLVM IR / MLIR / TypeScript
```

## What the Normalizer Attaches

### On type references

Every `Type` value in the AST gets an additional annotation:

```rust
// Before normalizer:
Type::Custom("Float64")  →  ResolvedType { bytes: 8, primitive: Some("Float"), properties: {...} }

// After LLVM normalizer:
Type::Custom("Float64")  →  ResolvedType {
    bytes: 8,
    primitive: Some("Float"),
    properties: {
        "llvm_type" => PropertyValue::String("double"),  // NEW — attached by normalizer
        "primitive" => PropertyValue::Identifier("Float") // kept — backend still uses it
    }
}
```

For CIRCT:

```rust
// After CIRCT normalizer:
ResolvedType {
    bytes: 8,
    primitive: Some("Float"),  // kept but CIRCT ignores it
    properties: {
        "bit_width" => PropertyValue::Int(64),  // NEW — attached by CIRCT normalizer
        "dialect" => PropertyValue::Identifier("hw")  // NEW — CIRCT dialect hint
    }
}
```

### On operation calls

Every `Expr::Call("Add#", ...)` that can be resolved gets an annotation:

```rust
// Before normalizer:
Expr::Call("Add#", [lhs, rhs])
// The backend would need to: look at arg types, derive primitive+bytes, look up config

// After normalizer:
Expr::Call("Add#", [lhs, rhs])
// The Call node itself has no annotation, but the arg types carry llvm_type.
// The backend reads: lhs.ty.properties["llvm_type"] = "double" → emit "fadd double"
```

Wait — the normalizer shouldn't replace the operation. It should attach enough info that the backend doesn't need config lookups. The backend still writes the instruction — it just reads `llvm_type` from the operands instead of deriving it from `primitive + bytes + config`.

### On every expression node

Every `Expr` variant that produces a value gets a `TypedRegister`-like annotation:

```rust
Expr::BinaryOp(BinaryOpKind::Add, lhs, rhs)
// The normalizer attaches inferred type info:
//   lhs inferred as: { llvm_type: "double" }
//   rhs inferred as: { llvm_type: "double" }
//   result inferred as: { llvm_type: "double" }
```

The backend just reads these annotations. It never matches on `Type::Custom("Float")`.

## Normalizer Trait

```rust
pub trait BackendNormalizer: std::fmt::Debug {
    fn name(&self) -> &str;

    /// Walk and annotate the AST. Called after plugin chain, before codegen.
    /// The AST structure is preserved — only annotations are added.
    fn normalize(
        &self,
        items: &mut Vec<TopLevel>,
        universe: &mut TypeUniverse,
    ) -> Result<(), String>;
}
```

## Builtin Normalizers

Each backend has its own normalizer compiled into the binary. Configured in `config/targets.toml`:

```toml
[".bv"]
backend = "llvm"
normalizer = "llvm"
defaults = ["--budget", "256"]

[".cbv"]
backend = "circt"
normalizer = "circt"
defaults = []
```

If `normalizer` is absent, it defaults to the same value as `backend`.

An external normalizer executable can be specified as a path:

```toml
[".bv"]
backend = "llvm"
normalizer = "/usr/lib/brief/my-normalizer"
```

External normalizers use the same stdin/stdout BVIR contract as plugins.

## Backend Normalizer: LLVM

```rust
// src/backend/llvm/normalizer.rs

impl BackendNormalizer for LlvmNormalizer {
    fn normalize(&self, items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String> {
        // 1. Attach llvm_type to every ResolvedType
        for rt in universe.types.values_mut() {
            let prim = rt.primitive();
            let llvm_ty = derive_llvm_type(prim, rt.bytes, &self.prim_config);
            rt.properties.insert("llvm_type".into(), PropertyValue::String(llvm_ty));
        }

        // 2. Walk all expressions, attach inferred llvm_type
        for item in items.iter_mut() {
            self.annotate_item(item, universe)?;
        }

        // 3. Strip metadata LLVM doesn't use
        for rt in universe.types.values_mut() {
            rt.properties.retain(|k, _| matches!(k.as_str(), "primitive" | "llvm_type" | "encoding"));
        }

        Ok(())
    }
}
```

What LLVM normalizer strips:

| Metadata key | LLVM keeps? | Why |
|-------------|-------------|-----|
| `primitive` | Yes | Used for operation dispatch (Add# on Float vs Int) |
| `llvm_type` | Yes | Attached by normalizer — pre-resolved LLVM type string |
| `encoding` | Yes | Used for string operations (utf-8 vs utf-16) |
| `hardware` | No | LLVM doesn't target hardware |
| `jira_ticket` | No | Plugin-only metadata, irrelevant to codegen |
| `rest_route` | No | Plugin-only metadata, irrelevant to codegen |

## Backend Normalizer: CIRCT

```rust
// src/backend/circt/normalizer.rs

impl BackendNormalizer for CirctNormalizer {
    fn normalize(&self, items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String> {
        // 1. Attach bit_width to every ResolvedType (CIRCT uses bits, not bytes)
        for rt in universe.types.values_mut() {
            let bit_width = rt.bytes * 8;
            rt.properties.insert("bit_width".into(), PropertyValue::Int(bit_width as i64));
        }

        // 2. Reject unsupported intrinsics
        let errors = validate_intrinsics(items, universe, &self.supported_ops);
        if !errors.is_empty() {
            return Err(format!("CIRCT normalizer: unsupported intrinsics:\n  {}", errors.join("\n  ")));
        }

        // 3. Strip everything except what CIRCT needs
        for rt in universe.types.values_mut() {
            rt.properties.retain(|k, _| matches!(k.as_str(), "bit_width"));
        }

        Ok(())
    }
}
```

What CIRCT normalizer strips:

| Metadata key | CIRCT keeps? | Why |
|-------------|-------------|------|
| `primitive` | No | CIRCT doesn't distinguish Int from Float — just bits |
| `bit_width` | Yes | Attached by normalizer — pre-resolved from bytes*8 |
| `hardware` | Yes | CIRCT-specific: bram, register, wire |
| `encoding` | No | Hardware doesn't need string encoding info |
| `llvm_type` | No | CIRCT doesn't emit LLVM IR |
| `jira_ticket` | No | Plugin-only metadata |

## Backend Normalizer: GPU

```rust
// src/backend/gpu/normalizer.rs

impl BackendNormalizer for GpuNormalizer {
    fn normalize(&self, items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String> {
        // 1. Attach llvm_type (GPU backend uses LLVM under the hood)
        for rt in universe.types.values_mut() {
            let prim = rt.primitive();
            let llvm_ty = derive_llvm_type(prim, rt.bytes, &self.prim_config);
            rt.properties.insert("llvm_type".into(), PropertyValue::String(llvm_ty));
        }

        // 2. Reject intrinsics that require CPU runtime
        let errors = validate_intrinsics(items, universe, &self.supported_ops);
        // ...

        // 3. Mark kernel entry points
        for item in items.iter_mut() {
            if let TopLevel::Transaction(t) = item {
                if t.metadata.contains_key("kernel") {
                    t.metadata.insert("gpu_kernel".into(), PropertyValue::Bool(true));
                }
            }
        }

        Ok(())
    }
}
```

## Backend Normalizer: Webstack

```rust
// src/backend/webstack/normalizer.rs

impl BackendNormalizer for WebstackNormalizer {
    fn normalize(&self, items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String> {
        // 1. Attach JS type annotations
        for rt in universe.types.values_mut() {
            let js_type = match rt.primitive() {
                Some("Int") => "number",
                Some("Float") => "number",
                Some("Bool") => "boolean",
                Some("String") => "string",
                Some("Char") => "number",
                _ => "object",
            };
            rt.properties.insert("js_type".into(), PropertyValue::String(js_type.into()));
        }

        // 2. Reject GPU and hardware intrinsics
        let errors = validate_intrinsics(items, universe, &self.supported_ops);
        // ...

        Ok(())
    }
}
```

## Shared Normalizer Helpers

```rust
// src/backend/normalizer.rs

/// Attach llvm_type to every ResolvedType based on primitive + bytes + config.
pub fn attach_llvm_types(universe: &mut TypeUniverse, config: &TypeConfig) {
    for rt in universe.types.values_mut() {
        let prim = rt.primitive();
        let llvm_ty = derive_llvm_type(prim, rt.bytes, config);
        rt.properties.insert("llvm_type".into(), PropertyValue::String(llvm_ty));
    }
}

/// Validate that every intrinsic call in the program is supported.
pub fn validate_intrinsics(
    items: &[TopLevel],
    universe: &TypeUniverse,
    supported: &HashSet<String>,
) -> Vec<String> {
    let mut errors = Vec::new();
    for call in collect_intrinsic_calls(items) {
        if !supported.contains(&call.name) {
            errors.push(format!("intrinsic '{}' is not supported", call.name));
        }
    }
    errors
}

/// Walk the AST and collect all Expr::Call where name ends with '#'.
pub fn collect_intrinsic_calls(items: &[TopLevel]) -> Vec<IntrinsicCall> {
    let mut calls = Vec::new();
    for item in items {
        walk_statements(item, &mut |stmt| {
            if let Statement::Expression(Expr::Call(name, _)) = stmt {
                if name.ends_with('#') {
                    calls.push(IntrinsicCall { name: name.clone() });
                }
            }
        });
    }
    calls
}
```

## Config Files

### `config/targets.toml` (updated)

```toml
[".bv"]
backend = "llvm"
normalizer = "llvm"
defaults = ["--budget", "256"]

[".ebv"]
backend = "llvm"
normalizer = "llvm"
defaults = ["--optimize-size", "--budget", "0"]

[".cbv"]
backend = "circt"
normalizer = "circt"
defaults = []

[".rbv"]
backend = "webstack"
normalizer = "webstack"
defaults = ["--target", "wasm"]

[".abv"]
backend = "gpu"
normalizer = "gpu"
defaults = ["--gpu-offload"]
```

### `config/llvm-ops.toml`

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

## Files to Create

| File | Purpose |
|------|---------|
| `config/llvm-ops.toml` | Operation → LLVM IR template mapping |
| `config/circt-ops.toml` | Operation → CIRCT MLIR template mapping |
| `config/webstack-ops.toml` | Operation → JS/TS template mapping |
| `src/backend/normalizer.rs` | Shared helpers: `attach_llvm_types()`, `validate_intrinsics()`, `collect_intrinsic_calls()` |
| `src/backend/llvm/normalizer.rs` | LlvmNormalizer — attaches llvm_type, keeps primitive, strips irrelevant |
| `src/backend/circt/normalizer.rs` | CirctNormalizer — attaches bit_width, rejects Print#/Malloc# |
| `src/backend/webstack/normalizer.rs` | WebstackNormalizer — attaches js_type |
| `src/backend/gpu/normalizer.rs` | GpuNormalizer — attaches llvm_type, rejects non-GPU ops |

## Files to Modify

| File | Change |
|------|--------|
| `config/targets.toml` | Add `normalizer` field per entry |
| `src/target.rs` | Add `normalizer` to `TargetEntry`, add `BackendNormalizerKind` enum |
| `src/compile.rs` | Wire normalizer between plugin chain and codegen dispatch |
| `src/config.rs` | No change — `OpConfig` already planned |
| `src/lib.rs` | Add `pub mod normalizer` if shared helpers are at crate root |
| `docs/architecture/overview.md` | Add normalizer stage to pipeline diagrams |
| `docs/architecture/backend-type-dispatch.md` | Add normalizer annotation model |

## Coding Standards

Every function in this plan must follow:
- **Max 2 nesting levels deep** — extract helpers, use guard clauses, early returns
- **`///` doc comments** on every `fn`, `struct`, `enum`, `mod`
- **`// 2026-07-14:` comments** explaining why each change exists at every modification site
- **No `else-if` chains deeper than 1** — early returns instead
- **HashMap determinism** — sort before iterating for LLVM IR emission
