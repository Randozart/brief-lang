# Plan: frgn / export / GLUE Pipeline

**2026-07-22:** Full implementation plan for cross-language FFI architecture.
See `docs/architecture/frgn-export-glue-architecture.md` for the design.

**Status:** Phases 0–4 complete, Phase 5 complete (CLI wired, `#export` modifier removed from language, export is now a straight keyword). Phases 6–7 pending implementation.

---

## Table of Contents

- [Phase 0: AST + Parser Changes (DONE)](#phase-0-ast--parser-changes)
- [Phase 1: TOML Registry + Type Files (DONE)](#phase-1-toml-registry--type-files)
- [Phase 2: ResolvedFrgn Dispatch in Compilation Pass (DONE)](#phase-2-resolvedfrgn-dispatch-in-compilation-pass)
- [Phase 3: GLUE Bridge Codegen (DONE)](#phase-3-glue-bridge-codegen)
- [Phase 4: Fallback Codegen (DONE)](#phase-4-fallback-codegen)
- [Phase 5: Export Unification + CLI Subcommands (DONE)](#phase-5-export-unification--cli-subcommands)
- [Phase 6: Layout Optimizer Pass](#phase-6-layout-optimizer-pass)
- [Phase 7: Tests + Examples](#phase-7-tests--examples)
- [Phase 8: Ship of Theseus — AST Pretty-Printer Stress Test](#phase-8-ship-of-theseus--ast-pretty-printer-stress-test)
- [Documentation](#documentation)
- [Edge Cases](#edge-cases)
- [Regression Guard](#regression-guard)

---

## Phase 0: AST + Parser Changes (DONE)

**Goal:** Add `as_name` and `Fallback` to `ForeignBinding`, parse them.

### 0.1 — Add `Fallback` enum and `as_name` field to `ForeignBinding`

**File:** `src/ast/top.rs`

Add after line 556 (before `ForeignBinding` struct):

```rust
/// 2026-07-22: Fallback strategy for when a frgn call's return violates
/// its contract or the foreign function cannot be reached.
/// The program must always produce a valid result — this is the safety net.
pub enum Fallback {
    /// Return a static expression (literal, constructor call, etc.)
    Static(Expr),
    /// Call a Brief function with the frgn's parameters
    FnCall(String, Vec<Expr>),
    /// Void-return frgn — just skip the call
    Implicit,
    /// No fallback declared (codegen uses zero-value of return type)
    None,
}
```

Add field to `ForeignBinding` struct (around line 560):

```rust
pub struct ForeignBinding {
    pub name: String,
    /// 2026-07-22: The foreign symbol name when it differs from `name`.
    /// `None` means the foreign symbol equals `name`.
    pub as_name: Option<String>,
    ...
    /// 2026-07-22: Fallback strategy when the foreign call fails.
    pub fallback: Fallback,
}
```

Update `ForeignBinding::new()` (around line 575) to accept `as_name` and
`fallback` parameters.

**Tests to add in `src/ast/top.rs`:**
- `test_fallback_static_creation`
- `test_fallback_fn_call_creation`
- `test_foreign_binding_with_as_name`

### 0.2 — Parse `as <ident>` and `fallback <expr>` in frgn declarations

**File:** `src/parser/definitions.rs`

Modify `parse_frgn_decl()` (around line 70):

```rust
/// 2026-07-22: Extended parsing for `as <foreign_symbol>` and
/// `fallback <expr>` / `fallback <fn_name>(<args>)`.
fn parse_frgn_decl(&mut self) -> Result<ForeignBinding, SyntaxError> {
    let frgn_bang = self.eat(&Token::FrgnBang);
    let name = self.expect_identifier()?;

    // Parse optional `as <foreign_symbol>` — symbol rename, NOT a protocol hint
    let as_name = if self.eat_identifier("as") {
        Some(self.expect_identifier()?)  // TODO: expect_identifier_or_string?
    } else {
        None
    };

    // Parse parameters
    self.expect(Token::LParen)?;
    let inputs = self.parse_fn_params()?;
    self.expect(Token::RParen)?;

    // Parse optional return type
    let success_output = if self.eat(&Token::Arrow) {
        Some(self.parse_type()?)
    } else {
        None
    };

    // Parse mandatory `from`
    self.expect_identifier("from")?;
    let from = self.parse_from_spec()?;

    // Parse optional `fallback`
    let fallback = if self.eat_identifier("fallback") {
        // Peek: if next token is an identifier followed by LParen, it's FnCall
        // Otherwise it's a literal expression
        if self.check(&Token::Identifier) && self.peek(1) == Some(&Token::LParen) {
            let fn_name = self.expect_identifier()?;
            self.expect(Token::LParen)?;
            let args = self.parse_call_args()?;
            self.expect(Token::RParen)?;
            Fallback::FnCall(fn_name, args)
        } else {
            let expr = self.parse_expression()?;
            Fallback::Static(expr)
        }
    } else {
        Fallback::None
    };

    self.expect(Token::Semicolon)?;

    Ok(ForeignBinding { name, as_name, inputs, success_output, from, fallback, ... })
}
```

Note: the `...` above means retain existing fields (`target`, `error_type`,
`error_fields`, `input_layout`, `output_layout`, `precondition`,
`postcondition`, `buffer_mode`, `default_watchdog`, `wasm_impl`,
`wasm_setup`, `span`).

**Tests to add in `src/parser/definitions.rs`:**
- `test_parse_frgn_with_as` — `frgn __func(x: Int) -> Int as func from "lib.so";`
- `test_parse_frgn_with_fallback_static` — `frgn get() -> Int as get from "mod.py" fallback 0;`
- `test_parse_frgn_with_fallback_fn` — `frgn get() -> Int as get from "mod.py" fallback default();`
- `test_parse_frgn_as_and_fallback` — both together
- `test_parse_frgn_rejects_missing_from` — parser error when `from` is absent

### 0.3 — Update `ForeignSignature` struct if needed

**File:** `src/ast/top.rs`

Check if `ForeignSignature` (the call-site type, around line 529) needs
`as_name` and `fallback`. It probably doesn't — it's the resolved signature
at call sites, not the declaration. But verify that the declaration-to-signature
conversion in the LLVM backend (`mod.rs:1751-1761`) copies `as_name` and
`fallback` if needed.

### 0.4 — Update display for `Fallback`

**File:** `src/ast/display.rs`

Add display for `Fallback`:
```rust
impl Display for Fallback {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Fallback::Static(expr) => write!(f, "fallback {}", expr),
            Fallback::FnCall(name, args) => {
                let args_str: Vec<String> = args.iter().map(|a| format!("{}", a)).collect();
                write!(f, "fallback {}({})", name, args_str.join(", "))
            }
            Fallback::Implicit => write!(f, "fallback;"),
            Fallback::None => Ok(()),
        }
    }
}
```

Also update `ForeignBinding` display to include `as_name` and `fallback`.

---

## Phase 1: TOML Registry + Type Files (DONE)

**Goal:** Replace the old `glue.dbvl`/`glue.dbvs` registry with `lib/glue.toml`,
create the per-language type `.bv` files.

### 1.1 — Create `lib/glue.toml`

**New file:** `lib/glue.toml`

```toml
# lib/glue.toml — GLUE Adapter Registry
# 2026-07-22: Replaces lib/glue.dbvl. Shipped with the compiler, project-level
# override via --glue-config <path>.
#
# Each section declares:
#   types_module    — .bv file declaring foreign type representations
#   extension       — native source file extension
#   bridge_kind     — how to call into this language from compiled Brief
#   calling_convention — ABI at the boundary
#   c_type_map      — Brief type → C ABI type for C ABI boundary

[python]
types_module = "glue/python/types.bv"
extension = "py"
bridge_kind = "native_module"
calling_convention = "c_abi"

[python.c_type_map]
Int = "int64_t"
Float = "double"
Bool = "bool"
String = "cstring"

[node]
types_module = "glue/node/types.bv"
extension = "mjs"
bridge_kind = "esm_module"
calling_convention = "c_abi"

[node.c_type_map]
Int = "int64_t"
Float = "double"
Bool = "bool"
String = "cstring"

[rust]
types_module = "glue/rust/types.bv"
extension = "rs"
bridge_kind = "extern_c_crate"
calling_convention = "lto"
```

### 1.2 — Create `lib/glue/python/types.bv`

**New directory/file:** `lib/glue/python/types.bv`

```brief
// 2026-07-22: Python type declarations for Brief's type universe.
// Types are defined against protocol haswords (#String<UTF8>, #Int, etc.)
// so GLUE can find CastTo/CastFrom paths.

type PyBytes <: Bits {
    bytes <~ 8;
    alignment <~ 8;
    op CastTo(#Bits) = identity(#L);
    op CastFrom(#Bits) = identity(#L);
};

type PyString <: Bits {
    bytes <~ 16;
    alignment <~ 8;
    op CastTo(#String<UTF8>) = ucs4_to_UTF8(#L);
    op CastFrom(#String<UTF8>) = UTF8_to_ucs4(#L);
    op CastTo(#String<ASCII>) = ucs4_to_ASCII(#L);
    op CastFrom(#String<ASCII>) = ASCII_to_ucs4(#L);
};

type PyInt <: Bits {
    bytes <~ 8;
    alignment <~ 8;
    op CastTo(#Int) = pylong_to_i64(#L);
    op CastFrom(#Int) = i64_to_pylong(#L);
};

// Zero-copy melds: if Brief CBuffer has same layout as PyBytes
meld PyBytes -> CBuffer {
    ptr -> ptr;
    len -> len;
};
```

### 1.3 — Create `lib/glue/node/types.bv`

**New directory/file:** `lib/glue/node/types.bv`

```brief
// 2026-07-22: Node.js type declarations for Brief's type universe.

type JsBuffer <: Bits {
    bytes <~ 8;
    alignment <~ 8;
    op CastTo(#Bits) = identity(#L);
    op CastFrom(#Bits) = identity(#L);
};

type JsString <: Bits {
    bytes <~ 8;
    alignment <~ 8;
    op CastTo(#String<UTF8>) = jsstring_to_UTF8(#L);
    op CastFrom(#String<UTF8>) = UTF8_to_jsstring(#L);
};

type JsNumber <: Bits {
    bytes <~ 8;
    alignment <~ 8;
    op CastTo(#Float<IEEE754>) = jsnumber_to_f64(#L);
    op CastFrom(#Float<IEEE754>) = f64_to_jsnumber(#L);
    op CastTo(#Int) = jsnumber_to_i64(#L);
    op CastFrom(#Int) = i64_to_jsnumber(#L);
};
```

### 1.4 — Create `lib/glue/rust/types.bv`

**New directory/file:** `lib/glue/rust/types.bv`

```brief
// 2026-07-22: Rust type declarations for Brief's type universe.
// Rust uses LLVM LTO for interop — no C ABI boundary for scalars.
// Types are defined for documentation and meld compatibility.

type RstI64 <: Bits {
    bytes <~ 8;
    alignment <~ 8;
    op CastTo(#Int) = identity(#L);
    op CastFrom(#Int) = identity(#L);
};

type RstF64 <: Bits {
    bytes <~ 8;
    alignment <~ 8;
    op CastTo(#Float<IEEE754>) = identity(#L);
    op CastFrom(#Float<IEEE754>) = identity(#L);
};
```

### 1.5 — TOML parser for the GLUE config

**New file:** `src/glue/config.rs`

```rust
/// ── GLUE Configuration (TOML) ─────────────────────────────────────────
/// 2026-07-22: Reads lib/glue.toml to resolve language targets for frgn
/// dispatch and export generation. Replaces the old dbvl-based registry.
///
/// Why TOML over dbvl: TOML is a mature, widely-supported format with
/// existing Rust ecosystem (toml crate). The dbvl format remains as the
/// output format for bridge-exports metadata (machine consumption), but
/// the compiler's own registry is TOML for maintainability.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A language target from the GLUE registry.
pub struct GlueTarget {
    pub language: String,
    pub types_module: PathBuf,
    pub extension: String,
    pub bridge_kind: String,
    pub calling_convention: String,
    pub c_type_map: HashMap<String, String>,
}

/// Load the GLUE registry from a TOML file.
///
/// Searches: built-in path (compiler-shipped lib/glue.toml), then
/// project-level override via --glue-config.
pub fn load_glue_config(path: Option<&Path>) -> Result<HashMap<String, GlueTarget>, String> {
    // Default to compiler-shipped path
    let config_path = match path {
        Some(p) => p.to_path_buf(),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("lib").join("glue.toml"),
    };
    let source = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read GLUE config '{}': {}", config_path.display(), e))?;
    let parsed: toml::Value = source.parse()
        .map_err(|e| format!("Failed to parse GLUE config '{}': {}", config_path.display(), e))?;
    // ... extract each language section into GlueTarget ...
}

/// Resolve the dispatch strategy for a frgn declaration based on extension.
pub fn resolve_frgn_dispatch(
    glue_targets: &HashMap<String, GlueTarget>,
    extension: &str,
) -> FrgnDispatch {
    // Match extension against each language's known patterns
    // e.g., ".py" → "python", ".js" → "node"
    // Then check if the target is serviced by this backend
}
```

**Test file:** `src/glue/config.rs` (inline `#[cfg(test)] mod tests`)
- `test_load_glue_config_default_path`
- `test_load_glue_config_custom_path`
- `test_resolve_frgn_dispatch_python`
- `test_resolve_frgn_dispatch_rust`
- `test_resolve_frgn_dispatch_unknown`

### 1.6 — Wire config into `src/glue/mod.rs`

**File:** `src/glue/mod.rs`

Add `pub mod config;` and re-export key types.

---

## Phase 2: ResolvedFrgn Dispatch in Compilation Pass (DONE)

**Goal:** The main compilation pass resolves each frgn declaration to a
`ResolvedFrgn` (Inline, Bridge, Unsupported) before the backend runs.

### 2.1 — Define `ResolvedFrgn` and `ProtocolStep`

**New file:** `src/analysis/frgn_dispatch.rs`

```rust
/// ── Frgn Dispatch Resolution ──────────────────────────────────────────
/// 2026-07-22: Resolved during the main compilation pass (before codegen),
/// not inside the backend. The backend receives a ResolvedFrgn and emits
/// the appropriate IR without re-implementing dispatch logic.
///
/// Why resolve pre-backend:
///   1. The dispatch decision depends on the protocol graph (type_universe),
///      the GLUE registry, and the backend's capabilities — all available
///      at compile time.
///   2. Backends should not reimplement extension matching or BFS.
///   3. A single error point for "no bridge available" is cleaner than
///      per-backend error messages.
///
/// Why NOT resolve in the backend:
///   The backend already knows its own capabilities. The ResolvedFrgn is
///   the intersection of "what the type system says" and "what the backend
///   can do." The backend still validates that it can handle the result.

use crate::ast::top::Fallback;
use crate::ast::Type;
use crate::glue::config::GlueTarget;

/// The dispatch strategy for a single frgn declaration.
pub enum ResolvedFrgn {
    /// Backend inlines directly (compile/link the source, call the symbol)
    Inline {
        /// The foreign symbol name (from `as` or brief_name)
        symbol: String,
        /// If true, the backend should compile this source to .o first
        compile_source: bool,
    },
    /// Route through the GLUE bridge
    Bridge {
        /// Language identifier
        language: String,
        /// Protocol transform chain for each parameter
        param_paths: Vec<ProtocolStep>,
        /// Protocol transform chain for the return value
        return_path: Option<ProtocolStep>,
        /// Fallback strategy
        fallback: Fallback,
    },
    /// Not supported by this backend
    Unsupported(String),
}

/// A single step in a protocol transform chain.
/// Describes how to go from one type representation to another.
pub struct ProtocolStep {
    /// The source type in the chain
    pub source: Type,
    /// The target type in the chain
    pub target: Type,
    /// The kind of transform needed
    pub kind: TransformKind,
}

/// Cost category for the transform.
pub enum TransformKind {
    /// No transform needed — types are structurally identical
    Identity,
    /// Meld shuffle — bit permutation, field reordering
    MeldShuffle,
    /// Protocol transform — CastTo/CastFrom with actual encoding work
    ProtocolTransform(String),  // the hashword category, e.g., "#String<UTF8>"
    /// Raw bitcast — implicit Cast(#Bits)
    Bitcast,
}
```

### 2.2 — Protocol path computation

Add a function to `src/analysis/frgn_dispatch.rs`:

```rust
/// Compute the protocol path between two types for a frgn boundary.
///
/// 2026-07-22: Uses the existing BFS in find_cast_path() + meld lookup
/// to determine how to transform a Brief type to/from a foreign type.
/// Returns the shortest path by cost.
pub fn compute_protocol_path(
    universe: &TypeUniverse,
    brief_type: &Type,
    foreign_type: &Type,
    operator_defs: &HashMap<String, Vec<OperatorDef>>,
) -> Result<Vec<ProtocolStep>, String> {
    // 1. Check for direct meld
    // 2. If no meld, find CastTo/CastFrom protocol path via BFS
    // 3. Fall back to implicit Cast(#Bits)
    // 4. Return error if no path exists
}
```

### 2.3 — Extension → language mapping

Add to `src/analysis/frgn_dispatch.rs`:

```rust
/// Map a file extension to a language identifier.
/// Baked per-backend but exposed via TOML for debugging.
pub fn extension_to_language(ext: &str, backend: BackendKind) -> Option<&'static str> {
    match backend {
        BackendKind::Llvm => match ext {
            "py" | "pyc" => Some("python"),
            "js" | "ts" | "mjs" => Some("node"),
            _ => None,
        },
        BackendKind::Webstack => match ext {
            "c" => Some("c"),
            "py" => Some("python"),
            "rs" => Some("rust"),
            _ => None,
        },
        BackendKind::Circt => None,  // All frgn rejected
        BackendKind::Spirv => match ext {
            "c" => Some("c"),
            "py" => Some("python"),
            _ => None,
        },
    }
}
```

### 2.4 — Wire into `src/compile.rs`

**File:** `src/compile.rs`

After the normalizer + protocol_verify (around line 188), add:

```rust
// ── Frgn dispatch resolution ───────────────────────────────────────────
let glue_config = brief_compiler::glue::config::load_glue_config(
    opts.glue_config.as_deref().map(|p| Path::new(p))
)?;

// For each ForeignBinding in items, resolve its dispatch strategy
let mut resolved_frgns: HashMap<String, ResolvedFrgn> = HashMap::new();
for item in &items {
    let brief_compiler::ast::TopLevel::ForeignBinding(fb) = item else { continue; };
    let ext = fb.from.extension().unwrap_or("");
    let dispatch = resolve_single_frgn(fb, ext, &glue_config, &universe, &operator_defs, opts.backend)?;
    resolved_frgns.insert(fb.name.clone(), dispatch);
}
```

The `resolve_single_frgn()` function:
1. Gets the extension from `fb.from`
2. Checks if extension is inlineable (per-backend list)
3. If yes → `ResolvedFrgn::Inline`
4. If not, checks if extension is bridgeable
5. If yes → compute protocol paths, return `ResolvedFrgn::Bridge`
6. If neither → `ResolvedFrgn::Unsupported`

### 2.5 — Pass `resolved_frgns` to the backend

Add `with_resolved_frgns()` method to `LlvmBackend` (and other backends):

```rust
// In LlvmBackend:
resolved_frgns: HashMap<String, ResolvedFrgn>,

pub fn with_resolved_frgns(mut self, map: HashMap<String, ResolvedFrgn>) -> Self {
    self.resolved_frgns = map;
    self
}
```

### 2.6 — Update `BuildOptions` if needed

**File:** `src/compile.rs`

Add `glue_config: Option<String>` to `BuildOptions` to allow project-level
override of the GLUE registry.

---

## Phase 3: GLUE Bridge Codegen (DONE)

**Goal:** Generate bridge calls for `ResolvedFrgn::Bridge` paths across all
backends.

### 3.1 — Shared bridge logic in `src/glue/bridge.rs`

**New file:** `src/glue/bridge.rs`

```rust
/// ── GLUE Bridge — Protocol-Mediated Foreign Calls ─────────────────────
/// 2026-07-22: Shared bridge generation logic used by all backends.
///
/// The bridge wraps a foreign function call with:
///   1. Protocol transforms (CastTo/CastFrom) for each parameter
///   2. The foreign call itself (via the appropriate mechanism)
///   3. Protocol transforms for the return value
///   4. Contract verification + fallback dispatch
///
/// The specific mechanism for making the foreign call (dlopen, Python
/// embedding, JS glue, etc.) is backend-specific, but the transform
/// chain and fallback logic is shared.

use crate::analysis::frgn_dispatch::{ProtocolStep, ResolvedFrgn};
use crate::ast::Type;

/// Emit the protocol transform chain for a single value.
/// Returns the IR string for the transformed value.
/// This is called by backends during codegen.
pub fn emit_protocol_chain(
    value_reg: &str,
    path: &[ProtocolStep],
    backend: &mut LlvmBackend,  // or generic Backend trait
) -> Result<String, String> {
    let mut current_reg = value_reg.to_string();
    for step in path {
        match step.kind {
            TransformKind::Identity => {
                // No transformation needed
            }
            TransformKind::MeldShuffle => {
                // Emit meld shuffle (lshr/and/shl/or sequence)
                // Reuses emit_meld_shuffle() in llvm/intrinsics.rs
            }
            TransformKind::ProtocolTransform(ref category) => {
                // Emit CastTo or CastFrom inline
                // Reuses emit_intrinsic_cast() path
            }
            TransformKind::Bitcast => {
                // Emit bitcast to iN
            }
        }
    }
    Ok(current_reg)
}

/// Emit a contract check + fallback dispatch.
/// Returns the final value register (phi of call result and fallback).
pub fn emit_fallback_wrapper(
    call_result_reg: &str,
    fallback: &Fallback,
    ret_type: &Type,
    backend: &mut LlvmBackend,
) -> Result<String, String> {
    match fallback {
        Fallback::Static(expr) => {
            // Emit the fallback expression, compute phi
        }
        Fallback::FnCall(name, args) => {
            // Emit a call to @name(args)
        }
        Fallback::Implicit | Fallback::None => {
            // Use zero-value for the return type
        }
    }
}
```

### 3.2 — LLVM backend bridge emission

**File:** `src/backend/llvm/emit_expr.rs`

Modify `emit_frgn_call()` (around line 1053) to handle `ResolvedFrgn::Bridge`:

```rust
/// 2026-07-22: Extended to handle both Inline and Bridge dispatch paths.
fn emit_frgn_call(&mut self, out: &mut String, v: &Expr, args: &[Expr], indent: usize)
    -> Result<BTypedRegister, String>
{
    // Get function name from the call expression
    let name = match v { Expr::Call(n, ..) => n, _ => return Err(...) };

    // Look up the resolved dispatch from pre-computed map
    let dispatch = self.resolved_frgns.get(name)
        .ok_or_else(|| format!("frgn '{}' not resolved", name))?;

    match dispatch {
        ResolvedFrgn::Inline { symbol, compile_source } => {
            // Existing inline call path
            self.emit_direct_frgn_call(out, symbol, args, indent)
        }
        ResolvedFrgn::Bridge { language, param_paths, return_path, fallback } => {
            // Bridge path: transform args, call foreign, transform result, apply fallback
            self.emit_bridge_frgn_call(out, name, args, param_paths, return_path, fallback, indent)
        }
        ResolvedFrgn::Unsupported(msg) => {
            Err(format!("frgn '{}': {}", name, msg))
        }
    }
}
```

### 3.3 — Webstack backend bridge emission

**File:** `src/backend/webstack.rs`

Modify `gen_ffi_call()` (around line 363) to accept the resolved dispatch
and emit the appropriate bridge. Webstack can directly call `.js`/`.ts`
functions, so the bridge for non-JS extensions would emit stub warnings.

### 3.4 — CIRCT backend

**File:** `src/backend/circt.rs`

Add frgn handling that always returns `ResolvedFrgn::Unsupported` (B5002).
Already caught by the hardware validator, but explicit is better.

**Tests:**
- `test_llvm_emit_bridge_call_with_protocol_chain`
- `test_llvm_emit_bridge_call_fallback_static`
- `test_webstack_bridge_call_stub`

---

## Phase 4: Fallback Codegen (DONE)

**Goal:** Implement fallback IR emission for all three forms.

### 4.1 — Unified fallback emission

**File:** `src/glue/bridge.rs` (or `src/backend/llvm/emit_expr.rs`)

```rust
/// Emit LLVM IR for fallback dispatch.
///
/// Structure:
///   %result = call @try_call(args...)
///   %ok = call @verify_postcondition(%result)
///   br i1 %ok, label %use_result, label %use_fallback
///
/// use_result:
///   br label %merge
///
/// use_fallback:
///   %fb = ... (fallback value)
///   br label %merge
///
/// merge:
///   %final = phi [%result, %use_result], [%fb, %use_fallback]
pub fn emit_fallback_llvm(
    out: &mut String,
    builder: &mut LlvmBuilder,
    call_reg: &str,
    ret_llvm_ty: &str,
    fallback: &Fallback,
    state_ptr: &str,
) -> Result<String, String> {
    let result_label = builder.gen_label("use_result");
    let fallback_label = builder.gen_label("use_fallback");
    let merge_label = builder.gen_label("merge");

    // Contract check: for now, a simple null/non-null check.
    // Future: full postcondition verification via the contract system.
    let ret_llvm_ptr_ty = format!("ptr");
    let null_check = if ret_llvm_ty == "void" {
        // Void return — no contract to check
        format!("br label %{}", merge_label)
    } else {
        format!(
            "  %ok = icmp ne {} %{}, {} zeroinitializer\n  br i1 %ok, label %%{}, label %%%{}",
            ret_llvm_ty, call_reg, ret_llvm_ty, result_label, fallback_label
        )
    };

    // ... emit the IR structure above ...
}
```

### 4.2 — Fallback value emission

Add a helper to emit the fallback value:

```rust
/// Emit the fallback value as LLVM IR.
fn emit_fallback_value(
    out: &mut String,
    builder: &mut LlvmBuilder,
    fallback: &Fallback,
    ret_llvm_ty: &str,
    state_ptr: &str,
) -> Result<String, String> {
    match fallback {
        Fallback::Static(expr) => {
            // Evaluate the expression at compile time
            let value = try_eval_const(expr)?;
            Ok(format!("{} {}", ret_llvm_ty, value))
        }
        Fallback::FnCall(name, args) => {
            // Emit call to @name(args)
            let arg_regs: Vec<String> = args.iter().map(|a| self.emit_expr(out, a, 0)).collect()?;
            Ok(format!("%fb = call {} @{}({})", ret_llvm_ty, name, arg_regs.join(", ")))
        }
        Fallback::Implicit | Fallback::None => {
            // Zero-value of the return type
            Ok(format!("{} 0", ret_llvm_ty))
        }
    }
}
```

**Tests:**
- `test_fallback_static_literal_llvm`
- `test_fallback_fn_call_llvm`
- `test_fallback_implicit_void_llvm`
- `test_fallback_wrapper_phi_structure`

---

## Phase 5: Export Unification + CLI Subcommands (DONE)

**Goal:** Add `brief export` and `brief link` subcommands, unify the two
export paths.

### 5.1 — Unify export extraction in `src/glue/export.rs`

**File:** `src/glue/export.rs`

Modify `extract_exports()` (around line 108) to handle `TopLevel::Export`:

```rust
fn extract_exports(items: &[TopLevel]) -> Vec<ExportDecl> {
    let mut exports = Vec::new();
    for item in items {
        // Form: export defn name(...) { ... } → TopLevel::Export
        if let TopLevel::Export(export) = item {
            if let TopLevel::Definition(defn) = export.inner.as_ref() {
                exports.push(ExportDecl { name: defn.name.clone(), ... });
            }
        }
    }
    // Deduplicate by name
    exports.sort_by_key(|e| e.name.clone());
    exports.dedup_by_key(|e| e.name.clone());
    exports
}
```

**Tests:** Update existing `extract_exports` tests to cover `TopLevel::Export`.

### 5.2 — Wire `brief export` CLI subcommand

**File:** `src/main.rs`

Add to the `match args[1].as_str()` (after line 30):

```rust
"export" => run_export(&args[2..]),
"link" => run_link(&args[2..]),
```

Add the handler functions:

```rust
fn run_export(args: &[String]) -> Result<(), String> {
    let file_path = args.first().ok_or("usage: brief export <bridge.bv> <language> --out <dir>")?;
    let language = args.get(1).ok_or("usage: brief export <bridge.bv> <language> --out <dir>")?;
    let mut out_dir = ".".to_string();
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--out" {
            out_dir = args.get(i + 1).ok_or("--out requires a directory argument")?.clone();
            i += 2;
        } else {
            return Err(format!("unknown flag: {}", args[i]));
        }
    }
    brief_compiler::glue::export::run_export_cli(file_path, language, &out_dir)
}
```

And `run_link()`:

```rust
fn run_link(args: &[String]) -> Result<(), String> {
    let lib_path = args.first().ok_or("usage: brief link <library.so/a/o>")?;
    let result = brief_compiler::glue::link::analyze_library(Path::new(lib_path))?;
    brief_compiler::glue::link::print_link_summary(&result);
    let bridge_bv = brief_compiler::glue::link::generate_bridge_bv(&result);
    // Write to stdout or file
    println!("{}", bridge_bv);
    Ok(())
}
```

### 5.3 — Adapt `run_export()` in `src/glue/export.rs`

Add a new entry point `run_export_cli()` that:

1. Reads and parses the `.bv` file (uses `parse_and_check` from `src/library.rs`)
2. Calls `extract_bridge_info()` to get exports/frgns/melds
3. Calls `find_adapter()` to find the language target (rename to `find_language` or adapt to TOML)
4. Calls codegen via `library::generate_with_exports()` to produce `.ll`
5. Runs `llc` → `.o` → `.so`/`.a` (reuses `library::run_library_mode()` logic)
6. Generates native wrapper source files (Python/Rust/Node — see Phase 5.4)
7. Writes `bridge-exports.dbvl` metadata

### 5.4 — Native wrapper generation

**New files per language template:**

Each language needs a wrapper generator. For Phase 1, implement Python and
Rust. Node can follow.

**Python wrapper** (`src/glue/gen/python.rs`):
```rust
pub fn generate_python_wrapper(exports: &[ExportDecl], bridge_name: &str) -> String {
    // Generate __init__.py with ctypes.CDLL wrappers
    let mut out = String::new();
    out.push_str(&format!(
        r#""""GLUE bridge for {} — auto-generated."
        import ctypes
        import os

        _lib = ctypes.CDLL(os.path.join(os.path.dirname(__file__), "bridge.so"))

        def _init_state():
            _lib.__brief_init_state.argtypes = []
            _lib.__brief_init_state.restype = ctypes.c_void_p
            return _lib.__brief_init_state()

        _STATE = _init_state()
"#, bridge_name));
    for e in exports {
        // Generate wrapper function for each export
    }
    out
}
```

**Rust wrapper** (`src/glue/gen/rust.rs`):
```rust
pub fn generate_rust_crate(exports: &[ExportDecl], bridge_name: &str) -> HashMap<String, String> {
    // Returns a map of filename → content:
    // Cargo.toml, build.rs, src/lib.rs, src/ffi.rs
}
```

### 5.5 — Update CLI usage text

**File:** `src/main.rs`, `print_usage()` (around line 49):

Add:
```
    eprintln!("  {} export <file.bv> <lang> [--out <dir>]  Generate a GLUE bridge for <lang>", name);
    eprintln!("  {} link <library.so/a/o>             Analyze a foreign library for frgn declarations", name);
```

---

## Phase 6: Layout Optimizer Pass

**Goal:** An analysis pass that considers adopting the foreign data layout
at frgn/export boundaries to minimize transforms.

### 6.1 — New analysis module

**New file:** `src/analysis/layout_optimizer.rs`

```rust
/// ── Layout Optimizer — "Become the Foreign" ──────────────────────────
/// 2026-07-22: Analysis pass that proposes type layout specialization at
/// frgn/export boundaries to minimize protocol transformation costs.
///
/// How it works:
///   1. Scan the AST for all frgn/export boundaries
///   2. For each parameter/return type that crosses a boundary:
///      a. Compute the protocol path cost (identity, shuffle, encode)
///      b. Check if the foreign type already has a preferred layout
///      c. If adopting the foreign layout would reduce the cost at the
///         boundary to identity, mark this type for specialization
///   3. Apply type layout changes in the function's type annotations
///      (changes `bytes <~ N`, alignment, and op signatures)
///
/// Safety: The contract system guarantees the specialization preserves
/// semantics. A specialized type must have CastTo/CastFrom back to the
/// original protocol — if it doesn't, the optimizer rejects it.

use crate::ast::TopLevel;
use crate::type_universe::TypeUniverse;

/// Result of the layout optimization pass: a set of type layout changes
/// to apply before codegen.
pub struct LayoutChange {
    /// The type to change
    pub type_name: String,
    /// New byte size
    pub new_bytes: u64,
    /// New alignment
    pub new_alignment: u64,
    /// The foreign type this was modeled after (for documentation)
    pub modeled_after: String,
}

/// Run the layout optimizer pass.
///
/// Returns a list of layout changes to apply. Returns empty vec if no
/// optimization is beneficial.
pub fn optimize_layouts(
    items: &[TopLevel],
    universe: &TypeUniverse,
    resolved_frgns: &HashMap<String, ResolvedFrgn>,
) -> Result<Vec<LayoutChange>, String> {
    let mut changes = Vec::new();

    for item in items {
        let TopLevel::ForeignKey(fb) = item else { continue; };
        let dispatch = match resolved_frgns.get(&fb.name) {
            Some(ResolvedFrgn::Bridge { .. }) => dispatch,
            _ => continue,  // Only optimize bridge path frgns
        };

        for (param_name, param_ty) in &fb.inputs {
            // 1. Find the foreign type's layout
            // 2. Compute current protocol path cost
            // 3. If adopting foreign layout reduces boundary cost:
            //    a. Verify safety (contract + CastTo/CastFrom)
            //    b. Propose layout change
            // 4. Record the change
        }
    }

    Ok(changes)
}
```

### 6.2 — Integration into `src/compile.rs`

Add the optimizer pass between protocol_verify and codegen:

```rust
// ── Layout optimization (frgn/export boundary) ──────────────────────────
let layout_changes = brief_compiler::analysis::layout_optimizer::optimize_layouts(
    &items, &universe, &resolved_frgns,
)?;
// Apply layout changes to the items
for change in &layout_changes {
    apply_layout_change(&mut items, change)?;
}
```

### 6.3 — Architecture considerations

The layout optimizer is a **new analysis pass**, not inlined into codegen.
Per Golden Rule 9 (NO PROTOTYPING — BUILD CLEAN), this must be a first-class
pass in its proper module.

The optimizer does NOT modify the backend. It modifies the AST's type
annotations before the backend sees them. The backend emits whatever type
layout it receives.

**Tests:**
- `test_layout_optimizer_no_boundary` — no frgns → no changes
- `test_layout_optimizer_identity_meld` — frgn with identity meld → no changes needed
- `test_layout_optimizer_adopt_foreign_layout` — frgn with different layout → proposes adoption
- `test_layout_optimizer_rejects_unsafe` — frgn without CastTo/CastFrom → no change

---

## Phase 7: Tests + Examples

**Goal:** Every code path has tests. Existing tests pass. Examples work.

### 7.1 — Update existing tests

- `tests/ffi_parser_tests.rs` — Add tests for `as` and `fallback` parsing
- `tests/ffi_typechecker_tests.rs` — Add tests for frgn with fallback types
- `tests/glue_test.rs` — Update to use TOML-based adapter lookup
- `src/parser/definitions.rs` — Existing parser tests must not break
- `src/lexer.rs` — Existing lexer tests must not break
- `src/backend/llvm/tests.rs` — Existing LLVM backend tests must not break

### 7.2 — New test files

| File | Tests |
|------|-------|
| `tests/frgn_export_e2e_tests.rs` | End-to-end: write a bridge .bv, run `brief export`, verify outputs |
| `tests/glue_bridge_tests.rs` | Unit tests for protocol chain computation, bridge codegen |
| `tests/fallback_tests.rs` | Fallback: static, fn call, implicit void |
| `tests/layout_optimizer_tests.rs` | Layout optimization scenarios |

### 7.3 — Update example files

**`examples/glue-rust-bridge/`:**
- Update `build.rs` to work with `brief export` output
- Add a `glue.toml` override (optional)

**`examples/glue-python-bridge/`:**
- Update `gluerun.py` to work with `brief export` output
- Document the new workflow

**`examples/test-bridge.bv`:**
- Add `as` and `fallback` examples to demonstrate syntax
- `frgn __calc(x: Int) -> Int as calculate from "libcalc.so" fallback 0;`

### 7.4 — Fix `tests/glue_integration.sh`

Update the stale test script to:

1. Use `brief-compiler export` instead of nonexistent `$BRIEF export`
2. Build the compiler first: `cargo build`
3. Test with a minimal `.bv` bridge file
4. Verify the output `.so` or `.a` exists
5. Skip if LLVM tools are unavailable (graceful degradation)

---

## Phase 8: Ship of Theseus — AST Pretty-Printer Stress Test

**Goal:** Port the AST pretty-printer (`src/ast/display.rs`) from Rust to Brief,
then call it through the GLUE bridge. This exercises the full FFI pipeline in
reverse direction (Brief → Host), validates protocol transforms on recursive
types, and provides the foundation for incremental self-hosting.

### 8.1 — Identify the port boundary

The AST pretty-printer is a set of `Display` impls in `src/ast/display.rs`:
- `TopLevel` → string
- `Definition` → string
- `Expr` → string (recursive, ~30 variants)
- `Type` → string
- `Statement` → string
- `ForeignBinding` → string
- `Pattern`, `MatchArm`, `Modifier`, etc.

Each is a pure function: `(AST node) -> String`. No global state, no I/O,
no mutable references. Perfect Brief candidate.

**Porting order** (increasing complexity):
1. `Type::Display` — flat, non-recursive shape, few variants
2. `ForeignBinding::Display` — moderate, has nested sub-structures
3. `Statement::Display` — introduces control flow keywords
4. `Expr::Display` — recursive, ~30 variants, the stress test
5. `Definition::Display` — wraps exprs and statements
6. `TopLevel::Display` — top-level dispatch

### 8.2 — Brief implementation

**New file:** `lib/pp/types.bv` — pretty-print functions for types:

```brief
// lib/pp/types.bv
// 2026-07-22: AST pretty-printer ported to Brief (Phase 8).

defn pp_type_int() -> String { term "Int"; }
defn pp_type_float() -> String { term "Float"; }
defn pp_type_bool() -> String { term "Bool"; }
defn pp_type_ptr(elem: String) -> String {
    term "Ptr<" ++ elem ++ ">";
}
// ... one function per Type variant
```

**New file:** `lib/pp/exprs.bv` — recursive expression pretty-printing:

```brief
// lib/pp/exprs.bv
// 2026-07-22: Recursive expression printer. Each Expr variant is a
// pure function taking sub-expression strings.

defn pp_binop(lhs: String, op: String, rhs: String) -> String {
    term "(" ++ lhs ++ " " ++ op ++ " " ++ rhs ++ ")";
}

defn pp_call(name: String, args: String) -> String {
    // args is already a joined string
    term name ++ "(" ++ args ++ ")";
}
// ...
```

### 8.3 — GLUE bridge wrapper

**New file:** `bridge/pp-bridge.bv` — export the pretty-printer:

```brief
// bridge/pp-bridge.bv
// 2026-07-22: Bridge for calling Brief pretty-printer from Rust compiler.
// Import the pretty-printer implementation
import "lib/pp/types.bv";
import "lib/pp/exprs.bv";

export defn brief_pp_type(type_tag: String, payload: String) -> String {
    // Dispatch on type_tag, call the appropriate pp function
    // ...
};
```

The Rust compiler calls `brief_pp_type` via the GLUE bridge:
```rust
// In src/ast/display.rs (modified):
fn display_type(ty: &Type) -> String {
    if let Ok(result) = glue::call("brief_pp_type", &[serialize_type(ty)]) {
        return result;
    }
    // Fallback: native Rust implementation
    // ...
}
```

### 8.4 — Verification

**Round-trip test**: For every AST in the test suite:
1. Generate the pretty-printed string using the Rust implementation
2. Generate it using the Brief implementation via GLUE bridge
3. Assert they match exactly

```rust
#[test]
fn test_pp_ast_roundtrip() {
    let ast = make_test_ast();
    let rust_pp = format!("{}", ast);      // Rust Display impl
    let brief_pp = call_brief_pp(&ast);    // Brief via GLUE
    assert_eq!(rust_pp, brief_pp);
}
```

**Regression test**: Run the full test suite with the GLUE bridge enabled and
disabled. Both must pass with identical output.

### 8.5 — Migration strategy

| Phase 8 step | What changes | Risk |
|-------------|-------------|------|
| 8a — `Type` | 4-5 Brief functions, simple strings | Low |
| 8b — `ForeignBinding` | Brief calls sub-printers | Low |
| 8c — `Statement` | Keywords, block formatting | Medium |
| 8d — `Expr` (all 30 variants) | Deeply recursive, stress-tests stack | **High** |
| 8e — `Definition` | Wraps exprs + statements | Medium |
| 8f — Full cutover | Rust Display impl delegates entirely to Brief | High |

### 8.6 — What this validates about the GLUE pipeline

| Requirement | How Phase 8 exercises it |
|------------|-------------------------|
| Protocol path resolution | Brief `String` ↔ Rust `String` via `#String<UTF8>` |
| Recursive type serialization | AST nodes contain sub-nodes (Expr in Expr) |
| Fallback correctness | If GLUE bridge fails, Rust fallback must produce identical output |
| Export wrapper generation | `brief export pp-bridge.bv rust --out ...` |
| `calling_convention = "lto"` | Rust LTO path — no C ABI boundary overhead |
| Performance | Pretty-printing is not hot code; tail-call optimization under Brief contracts |

### 8.7 — When to consider Phase 8 done

- `Type::Display` delegates to Brief (verified by round-trip test)
- `ForeignBinding::Display` delegates to Brief
- `Statement::Display` delegates to Brief
- `Expr::Display` delegates to Brief (the big one — all 30+ variants)
- `Definition::Display` and `TopLevel::Display` delegate to Brief
- Full test suite passes with GLUE bridge enabled
- Native Rust fallback produces identical output (verified by CI)

---

## Documentation

### Architecture docs to update/author

| File | Change |
|------|--------|
| `docs/architecture/frgn-export-glue-architecture.md` | Remove `#export` modifier references (deprecation syntax removed) |
| `docs/architecture/ship-of-theseus.md` | New: strategy for incremental self-hosting via GLUE bridge |
| `docs/architecture/glue-pipeline.md` | Update Mermaid diagram, remove `$!macro` references, add TOML path |
| `docs/architecture/casting-protocol.md` | Add note about frgn/export protocol path resolution |
| `docs/architecture/overview.md` | Add frgn dispatch + layout optimizer to pipeline diagram |

### Plan docs to reference

| File | Reference |
|------|-----------|
| `docs/plans/2026-07-20-protocol-and-meld-architecture.md` | Protocol path resolution foundation |
| `docs/plans/2026-07-10-glue-v2-ffi-unification.md` | Previous GLUE v2 design (superseded by this plan) |

### Inline doc comments to add

Every new/modified function gets a `///` doc comment per Coding Standards.

### Rationale comments at code sites

Every code site modified or added must have:

```
// 2026-07-22: <why this exists>
// <what problem it solves, what pattern it targets>
```

Key sites:
- `src/ast/top.rs` — Fallback enum, as_name field on ForeignBinding
- `src/parser/definitions.rs` — `as` and `fallback` parsing
- `src/compile.rs` — Frgn dispatch resolution + layout optimizer integration
- `src/analysis/frgn_dispatch.rs` — The entire module
- `src/analysis/layout_optimizer.rs` — The entire module
- `src/glue/bridge.rs` — The entire module
- `src/glue/config.rs` — The entire module (with TOML rationale)
- `src/glue/export.rs` — Unified export extraction
- `src/backend/llvm/emit_expr.rs` — Bridge path in emit_frgn_call
- `src/main.rs` — New CLI subcommands

---

## Edge Cases

### E1 — `from` with no recognized extension

Already handled: `ResolvedFrgn::Unsupported` produces a compile error.

### E2 — `as` with the same name as the Brief name

Not an error, but redundant. The compiler should emit a note:
```
note: frgn 'func' has as 'func' — same name, `as` is redundant.
```

### E3 — `fallback` return type mismatch

The fallback expression must match the frgn's return type. The typechecker
should validate this:
```
error: fallback type mismatch for frgn 'get_user'.
  Expected return type: User
  Fallback expression type: Int
```

### E4 — Bridge path with no protocol path for a parameter

If a frgn parameter type has no CastTo/CastFrom path to any known foreign
type, the compiler should error with suggestions:
```
error: no protocol path from 'CustomStruct' to 'python' target.
  Required by frgn 'process' in bridge.bv.
  Available paths:
    - Cast(#Bits) — raw bytes (always available)
    - Add a meld between 'CustomStruct' and a known Python type
    - Add op CastTo(#String) to 'CustomStruct' for string serialization
```

### E5 — `brief export` with no exports

Graceful: "No exports found in bridge file. Nothing to generate." Exit 0.

### E6 — `brief link` on a library with no T (text) symbols

Already handled by `analyze_library()` — returns error "No T (text) symbols found."

### E7 — Interpreter cannot load native libraries

The interpreter's `dispatch_ffi()` remains a stub. When the interpreter
encounters a frgn call during compile-time evaluation:
1. If the frgn has a `fallback` → use the fallback value
2. If no fallback → emit a warning and return zero-value

This is correct: the interpreter is for compile-time evaluation, not runtime
FFI. Contract verification at runtime handles the real case.

---

## Regression Guard

### What must NOT break

| File | What to check |
|------|---------------|
| `src/backend/llvm/emit_expr.rs` | `emit_frgn_call()` existing inline path must still work |
| `src/backend/llvm/mod.rs` | `frgn_map` insertion at line 1751 must still work |
| `src/compile.rs` | `collect_extra_objects()` must still compile .c sources |
| `src/library.rs` | `run_library_mode()` must still produce .a without GLUE |
| `src/glue/export.rs` | `extract_exports()` with `TopLevel::Export` must still work |
| `src/glue/link.rs` | `analyze_library()` + `generate_bridge_bv()` must still work |
| `src/parser/definitions.rs` | Existing `parse_frgn_decl()` tests must pass |

### Key additive-only points

- `ForeignBinding` gets new fields but existing construction paths remain
- `extract_exports()` gets a new match arm for `TopLevel::Export` but existing
  `#export` modifier path remains
- New `ResolvedFrgn` dispatch is additive — existing `frgn_map`/`emit_frgn_call()`
  still works for the Inline path
- The layout optimizer is a new pass — removing it should not affect correctness
- The TOML parser is a new module — removing it falls back to empty config

### Verification checklist

Before committing each phase:

1. `cargo test --lib` — all tests pass
2. `cargo build` — no warnings
3. Existing LLVM export tests pass (`src/backend/llvm/tests.rs`)
4. Existing parser tests pass (`src/parser/definitions.rs`)
5. New tests for the phase pass
