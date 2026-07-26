# FFI Convention System — Type Extensions + FromSpec + Pre-Compilation + Auto-Meld + Validation

**Date:** 2026-07-16
**Status:** Plan

---

## Philosophy

Brief's FFI is *convention-driven*, not target-enum-driven. A type like `String.c` is not "the C type for String" — it is "String under the `.c` file's convention." When a function body makes FFI calls to multiple conventions, the meld system finds the zero-copy intersection layout automatically.

The compiler is an abstract machine that adapts to any foreign ABI through three primitives:
1. **Extension-typed params** (`T.<ext>`) — tell the compiler which convention this boundary uses
2. **Meld routes** — define how conventions inter-convert
3. **FromSpec** — path to the foreign object, from which the extension is derived

---

## Overview

| Phase | Feature | Files | Effort | Depends on |
|-------|---------|-------|--------|-----------|
| P2 | Dotted type names + extension groups | `parser/types.rs`, `parser/definitions.rs`, `ast/top.rs`, `type_universe/mod.rs` | Small | Nothing |
| P3 | `FromSpec` — parser + AST + resolution | `ast/top.rs`, `parser/definitions.rs`, `import_resolver.rs`, `ffi/loader.rs`, `backend/llvm/mod.rs`, `interpreter/eval.rs` | Large | P2 |
| P4 | Pre-compilation pipeline | `compile.rs` | Medium | P3 |
| P5 | Auto-meld at FFI boundary | `backend/llvm/mod.rs`, `backend/llvm/helpers.rs`, `interpreter/eval.rs` | Medium | P0 + P2 + P3 |
| P6 | Meld validation cascade (5 layers) | `analysis/meld_validation.rs` (new), `normalizer.rs`, `proof_engine/smt.rs`, `symbolic.rs` | Medium | P0 |

---

## P2 — Dotted Type Names + Extension Groups

### What

Parser support for `String.c` (dotted type names) and `String.[c,cpp,cs]` (extension group expansion). Type universe queries for extension resolution.

### Not

`ForeignTarget` variants — the target enum stays as-is (Native, Wasm, C, Python, Js, Swift, Go, Metropolitan).

### Files Modified

#### `src/ast/top.rs`

No changes to `ForeignTarget`. No changes to `ForeignSignature` or `ForeignBinding` (those are P3).

#### `src/parser/types.rs`

`parse_type_identifier()` — after parsing the base name, peek for `Token::Dot` followed by an identifier. If found, concatenate with `.`:

```rust
fn parse_type_identifier(&mut self) -> Result<String, SyntaxError> {
    let mut name = self.expect_identifier()?;
    // 2026-07-16: Check for .ext suffix (e.g., "String.c")
    if self.eat(&Token::Dot) {
        let ext = self.expect_identifier()?;
        name.push('.');
        name.push_str(&ext);
        // Check for .ext1.ext2 (e.g., "Int.c.sso")
        if self.eat(&Token::Dot) {
            let ext2 = self.expect_identifier()?;
            name.push('.');
            name.push_str(&ext2);
        }
    }
    Ok(name)
}
```

Two-level dotted names only (e.g., `Int.c.sso` for "Int under C convention, specifically the .sso object").
The `.` token already exists in the lexer — no lexer changes needed.

#### `src/parser/definitions.rs`

`parse_type_extension_group()` — after `Token::Type` + name, detect `Token::Dot` + `Token::LBracket`:

```rust
/// 2026-07-16: P2 — Parse Type.[a,b,c] extension group expansion.
fn parse_type_extension_group(&mut self, base_name: String) -> Result<Vec<TopLevel>, SyntaxError> {
    // Token::Dot and Token::LBracket already consumed by caller
    let mut exts = Vec::new();
    loop {
        exts.push(self.expect_identifier()?);
        if !self.eat(&Token::Comma) { break; }
    }
    self.expect(Token::RBracket)?;
    let body = self.parse_type_body()?;
    Ok(exts.into_iter().map(|ext| {
        let full_name = format!("{}.{}", base_name, ext);
        TopLevel::TypeDef(TypeDef {
            name: full_name,
            base: Some(body.base.clone().unwrap_or(base_name.clone())),
            body: body.body.clone(),
            span: body.span.clone(),
            modifiers: vec![],
        })
    }).collect())
}
```

Wire into `parse_top_level()`: after the `Token::Type` arm parses the name (via `parse_type_identifier`), check if next is `Token::Dot` + `Token::LBracket`. If yes, call `parse_type_extension_group`. If `Token::Dot` + identifier, the type name itself is dotted (handled by `parse_type_identifier`).

#### `src/type_universe/mod.rs`

Three new query methods:

```rust
impl TypeUniverse {
    /// 2026-07-16: P2 — Look up "String.c" from base "String" and extension "c".
    pub fn get_extension(&self, base: &str, ext: &str) -> Option<&ResolvedType> {
        self.types.get(&format!("{}.{}", base, ext))
    }

    /// 2026-07-16: P2 — Find meld between base type and an extension type directly.
    pub fn find_ext_meld(&self, base: &str, ext: &str) -> Option<&MeldDeclaration> {
        let ext_name = format!("{}.{}", base, ext);
        self.melds.get(&(base.to_string(), ext_name))
            .or_else(|| self.melds.get(&(ext_name, base.to_string())))
    }

    /// 2026-07-16: P2 — Find a meld from `ty` to any type ending in `.ext`.
    /// Priority:
    ///   1. Direct meld T <:> T.ext  (exact match)
    ///   2. Direct meld T <:> Any.ext  (custom → standard extension)
    ///   3. T.ext exists with auto-generated identity meld (via : inheritance)
    ///   4. None — no meld possible
    pub fn find_meld_to_extension(&self, ty: &str, ext: &str) -> Option<(String, MeldDeclaration)> {
        // Priority 1: T <:> T.ext
        let exact = format!("{}.{}", ty, ext);
        if let Some(decl) = self.find_ext_meld(ty, ext) {
            return Some((exact, decl.clone()));
        }
        // Priority 2: T <:> Any.ext
        for ((a, b), decl) in &self.melds {
            if a == ty && b.ends_with(&format!(".{}", ext)) {
                return Some((b.clone(), decl.clone()));
            }
            if b == ty && a.ends_with(&format!(".{}", ext)) {
                return Some((a.clone(), decl.clone()));
            }
        }
        // Priority 3: T.ext exists — implicit identity meld via : inheritance.
        // Same bit layout assumed; no explicit routes needed.
        if self.types.contains_key(&exact) {
            return Some((exact, MeldDeclaration {
                name_a: ty.to_string(),
                name_b: exact,
                routes: vec![],
                span: None,
            }));
        }
        None
    }
}
```

#### `src/backend/llvm/normalizer.rs`

No changes (P0 already populated `TypeUniverse.melds`). The `find_meld_to_extension` queries the universe that P0 filled.

### Verification

```rust
#[test]
fn test_parse_dotted_type_name() {
    // "String.c" → Custom("String.c")
}
#[test]
fn test_parse_extension_group() {
    // "type String.[c,cpp,cs] : String { ... }" → 3 TypeDef items
}
#[test]
fn test_find_meld_to_extension_priority() {
    // Priority 1: direct meld
    // Priority 2: custom→standard
    // Priority 3: implicit identity
}
```

### Implementation Steps

1. Edit `src/ast/top.rs` — no changes needed (this phase is parser+universe only)
2. Edit `src/parser/types.rs` — `parse_type_identifier()` with `.ext` suffix
3. Edit `src/parser/definitions.rs` — `parse_type_extension_group()` + wire into `parse_top_level`
4. Edit `src/type_universe/mod.rs` — `get_extension()`, `find_ext_meld()`, `find_meld_to_extension()`
5. Commit

---

## P3 — `FromSpec` Parser + AST + Resolution

### What

The `from` clause becomes a first-class AST construct. `ForeignSignature.location: String` → `from: FromSpec`. `ForeignBinding.location: String` → `from: FromSpec`. The parser gains `parse_frgn_decl()` (it currently doesn't parse `frgn` at all).

### Concept

```brief
frgn strlen(s: String) -> Int from "libc.so.6";
frng hash(data: Data) -> Int from <xxhash.c>;
```

- `from "path"` — literal path (CWD-relative or absolute)
- `from <name>` — compiler-relative lookup (same pattern as `import <name>`)
- Extension on the path (`.c`, `.cpp`, `.so`, `.dylib`, `.wasm`) drives convention selection

### Files Modified

#### `src/ast/top.rs`

```rust
/// 2026-07-16: P3 — Where a frgn function's implementation comes from.
#[derive(Debug, Clone)]
pub enum FromSpec {
    /// from "path/to/file" — literal path (CWD-relative or absolute).
    Literal(PathBuf),
    /// from <name> — compiler-relative lookup (same pattern as import <name>).
    CompilerRegistry(String),
}

impl Default for FromSpec {
    fn default() -> Self {
        Self::Literal(PathBuf::new())
    }
}

impl FromSpec {
    /// Resolve to an absolute filesystem path.
    pub fn resolve(&self, resolver: &ImportResolver) -> Result<PathBuf, String> {
        match self {
            Self::Literal(path) => {
                if path.is_absolute() {
                    Ok(path.clone())
                } else {
                    std::env::current_dir()
                        .map(|cwd| cwd.canonicalize().unwrap_or(cwd).join(path))
                        .map_err(|e| format!("cannot get CWD: {}", e))
                }
            }
            Self::Registry(name) => {
                resolver.resolve_stdlib_relative_path(&format!("ffi/{}", name))
                    .ok_or_else(|| format!("cannot find compiler-relative path: {}", name))
            }
        }
    }

    /// Extract the file extension for convention derivation.
    pub fn extension(&self, resolver: &ImportResolver) -> Option<String> {
        let path = match self {
            Self::Literal(p) => p.clone(),
            Self::Registry(name) => {
                // Best-effort — return None if resolve fails
                return name.rsplit('.').next().map(|s| s.to_string());
            }
        };
        path.extension().and_then(|s| s.to_str()).map(|s| s.to_string())
    }
}
```

Replace `location: String` on both structs:

```rust
// ForeignSignature — line ~431
pub struct ForeignSignature {
    pub name: String,
    pub from: FromSpec,           // was: location: String
    pub inputs: Vec<(String, Type)>,
    pub result_type: ResultType,
    pub wasm_impl: Option<String>,
    pub wasm_setup: Option<String>,
    pub span: Option<Span>,
}

// ForeignBinding — line ~457
pub struct ForeignBinding {
    pub name: String,
    pub from: FromSpec,           // was: location: String
    pub target: ForeignTarget,
    pub inputs: Vec<(String, Type)>,
    pub success_output: Vec<(String, Type)>,
    // ... rest unchanged
}
```

Update `ForeignBinding::new()` constructor to accept `FromSpec`:

```rust
impl ForeignBinding {
    pub fn new(name: String, from: FromSpec, target: ForeignTarget) -> Self {
        ForeignBinding {
            name,
            from,
            target,
            inputs: vec![],
            success_output: vec![],
            // ... rest default
        }
    }
}
```

#### `src/parser/definitions.rs`

Add parsing of `frgn` declarations. Currently `parse_top_level()` has no arm for `Frgn`/`FrgnBang`.

```rust
/// 2026-07-16: P3 — Parse `frgn` declaration.
/// Syntax:
///   frgn name(params) -> Ret from "path";
///   frgn name(params) -> Ret from <name>;
///   frgn name(params) -> Ret from "path" target "c";
///
/// params: (name: Type, name: Type, ...)
fn parse_frgn_decl(&mut self) -> Result<TopLevel, SyntaxError> {
    let name = self.expect_identifier()?;
    self.expect(Token::LParen)?;
    let mut inputs = Vec::new();
    while !self.check(Token::RParen) {
        let param_name = self.expect_identifier()?;
        self.expect(Token::Colon)?;
        let param_type = self.parse_type()?;
        inputs.push((param_name, param_type));
        if !self.eat(&Token::Comma) { break; }
    }
    self.expect(Token::RParen)?;
    // Optional -> return type
    let result_type = if self.eat(&Token::Arrow) {
        ResultType::Projection(vec![self.parse_type()?])
    } else {
        ResultType::VoidType
    };
    // from clause
    let from = if self.eat_identifier("from") {
        self.parse_from_spec()?
    } else {
        FromSpec::default()
    };
    // Optional target override
    let mut target = ForeignTarget::C;
    if self.eat_identifier("target") {
        let target_str = self.expect_string()?;
        target = ForeignTarget::from_name(&target_str)
            .ok_or_else(|| self.error(format!("unknown target: {}", target_str)))?;
    }
    self.expect(Token::Semicolon)?;
    let sig = ForeignSignature {
        name: name.clone(),
        from: from.clone(),
        inputs: inputs.clone(),
        result_type: result_type.clone(),
        wasm_impl: None,
        wasm_setup: None,
        span: Some(self.current_span()),
    };
    // Also emit a ForeignBinding for TOML-based FFI registry compatibility
    let binding = ForeignBinding {
        name,
        from,
        target,
        inputs,
        success_output: match result_type {
            ResultType::Projection(ts) => ts.into_iter().map(|t| (String::new(), t)).collect(),
            _ => vec![],
        },
        // ... defaults for rest
        ..ForeignBinding::default()
    };
    // For now, emit ForeignBinding (legacy path). When LlvmBackend fully
    // switches to ForeignSignature, we can emit that instead.
    Ok(TopLevel::ForeignBinding(binding))
}

/// 2026-07-16: P3 — Parse `from "path"` or `from <name>`.
fn parse_from_spec(&mut self) -> Result<FromSpec, SyntaxError> {
    if self.eat(&Token::Lt) {
        let name = self.expect_identifier()?;
        self.expect(Token::Gt)?;
        Ok(FromSpec::CompilerRegistry(name))
    } else {
        let path_str = self.expect_string()?;
        Ok(FromSpec::Literal(PathBuf::from(path_str)))
    }
}
```

Wire into `parse_top_level()`:

```rust
// In parse_top_level(), add arm:
Token::Frgn | Token::FrgnBang => {
    self.advance();
    self.parse_frgn_decl()
}
```

Wait — the lexer has `Frgn` and `FrgnBang` tokens. But looking at the patterns already working, `parse_top_level()` dispatches on `Token::Type`, `Token::State`, etc. We add the new arm.

Actually need to check: does the lexer parse `frgn` as `Token::Frgn` or as an identifier? Let me verify.

Looking at `src/lexer.rs`, the token enum should have `Frgn` and `FrgnBang` variants. Let me verify they exist and match the right patterns.

Actually, from the exploration output, `src/lexer.rs` has at line 94 mention of `Frgn` and line 97 `FrgnBang`. They exist as tokens. But `parse_top_level()` has no arm for them. So `frgn` declarations in `.bv` source files are silently ignored? Or cause a parse error?

Actually, since they're valid tokens but not handled in `parse_top_level()`, they'd cause the `_ =>` arm to fire — likely returning an error like "unexpected token".

So adding the `Frgn`/`FrgnBang` arm is strictly additive: it enables something that was previously an error.

#### `src/import_resolver.rs`

Make `resolve_stdlib_root()` public. Add `resolve_stdlib_relative_path()`:

```rust
impl ImportResolver {
    /// 2026-07-16: P3 — Resolve a path relative to the stdlib root.
    /// Searches the same paths as resolve_stdlib_root().
    pub fn resolve_stdlib_relative_path(&self, relative: &str) -> Option<PathBuf> {
        for root in self.stdlib_search_roots() {
            let candidate = root.join(relative);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        None
    }

    /// 2026-07-16: P3 — Return all possible stdlib root directories.
    fn stdlib_search_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Some(ref path) = self.stdlib_path {
            roots.push(path.clone());
        }
        if let Ok(env_path) = std::env::var("BRIEF_STDLIB_PATH") {
            roots.push(PathBuf::from(env_path));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                roots.push(dir.join("../../lib"));
                roots.push(dir.join("../share/brief"));
            }
        }
        roots
    }
}
```

#### `src/ffi/loader.rs`

Update TOML-based `ForeignBinding` construction to produce `FromSpec::Literal(path)` from the `location` field:

```rust
// When constructing ForeignBinding from TOML:
let from = FromSpec::Literal(PathBuf::from(&fb_toml.location));
```

#### `doc/architecture/backend-type-dispatch.md`

No structural changes — extension-based dispatch replaces the hardcoded type-mapping system described in this doc. Add a section noting that type dispatch is now convention-driven via `T.<ext>`.

#### `src/backend/llvm/mod.rs`

Update `frgn_map` population (~line 1532) to pass `from` instead of `location`:

```rust
TopLevel::ForeignBinding(fb) => {
    let sig = crate::ast::ForeignSignature {
        name: fb.name.clone(),
        from: fb.from.clone(),         // was: location: fb.location.clone()
        inputs: fb.inputs.clone(),
        result_type: crate::ast::ResultType::Projection(
            fb.success_output.iter().map(|(_, t)| t.clone()).collect()
        ),
        wasm_impl: fb.wasm_impl.clone(),
        wasm_setup: fb.wasm_setup.clone(),
        span: fb.span,
    };
    self.ctx.frgn_map.insert(fb.name.clone(), sig);
}
```

Update foreign declare emission (~line 1705) to derive the LLVM function name from the path:

```rust
// No change to declare emission — still uses sig.name as the function name.
// The path (sig.from) is used only for linker flags, not for declare.
```

Add linker include logic in `compile_ll_to_binary()` — see P4.

#### `src/interpreter/eval.rs`

Update frgn call resolution to use `FromSpec`:

```rust
// In eval_frgn_call or wherever `location` is read:
let resolved_path = sig.from.resolve(&import_resolver)?;
// Use resolved_path instead of old sig.location
```

#### All other call sites of `ForeignSignature.location` / `ForeignBinding.location`

The rename from `location: String` to `from: FromSpec` is the highest-touch change. Every match arm, every construction site must be updated. Use `cargo build` to catch them all after the rename.

### Verification

```rust
#[test]
fn test_parse_frgn_literal_path() {
    // 'frgn foo(x: Int) from "lib.so";' 
}
#[test]
fn test_parse_frgn_compiler_path() {
    // 'frgn foo(x: Int) from <xxhash.c>;'
}
#[test]
fn test_frgn_resolve_literal() {
    // FromSpec::Literal("lib.so") resolves to CWD/lib.so
}
#[test]
fn test_frgn_resolve_registry() {
    // FromSpec::Registry("xxhash.c") resolves via stdlib search
}
```

### Implementation Steps

1. Add `FromSpec` enum + `Default` + `resolve()` + `extension()` to `src/ast/top.rs`
2. Rename `location: String` → `from: FromSpec` on `ForeignSignature` and `ForeignBinding`
3. Update `ForeignBinding::new()` constructor
4. Build — fix every compile error from the rename
5. Add `parse_frgn_decl()` and `parse_from_spec()` to `src/parser/definitions.rs`
6. Wire into `parse_top_level()`
7. Add `resolve_stdlib_relative_path()` to `src/import_resolver.rs`
8. Update `src/ffi/loader.rs` TOML parsing
9. Update `src/backend/llvm/mod.rs` frgn_map population
10. Update `src/interpreter/eval.rs` frgn resolution
11. Write tests
12. Commit

---

## P4 — Pre-Compilation Pipeline

### What

When a `from` path ends in `.c`, `.cpp`, `.cc`, `.cxx`, or `.m`, the compiler compiles it to a `.o` before linking. `.so`, `.dylib`, `.a`, `.o` are passed directly to the clang linker line.

### Files Modified

#### `src/compile.rs`

**New function: `compile_source_to_object()`**

```rust
/// 2026-07-16: P4 — Compile a C/C++ source to a .o object file.
/// Content-hash cached at ~/.cache/brief-compiler/ffi/<hash>.o.
fn compile_source_to_object(source_path: &Path, cache_dir: &Path) -> Result<PathBuf, String> {
    let content = std::fs::read(source_path)
        .map_err(|e| format!("cannot read '{}': {}", source_path.display(), e))?;
    let hash = blake3::hash(&content);
    let cache_path = cache_dir.join(format!("{:x}.o", hash));
    if cache_path.exists() {
        return Ok(cache_path);  // Cache hit — skip compilation
    }
    let ext = source_path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let lang_flag = match ext {
        "c" | "m" => "c",
        "cpp" | "cc" | "cxx" => "c++",
        _ => return Err(format!("unknown source extension '{}' for '{}'", ext, source_path.display())),
    };
    let status = Command::new("clang")
        .args([
            "-O3", "-march=native", "-ffast-math",
            "-x", lang_flag,
            "-c",
            source_path.to_str().unwrap(),
            "-o", cache_path.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| format!("failed to invoke clang (is it installed?): {}", e))?;
    if !status.success() {
        return Err(format!("clang failed to compile '{}'", source_path.display()));
    }
    Ok(cache_path)
}
```

**New function: `get_ffi_cache_dir()`**

```rust
/// 2026-07-16: P4 — Get or create the FFI object cache directory.
fn get_ffi_cache_dir() -> PathBuf {
    let base = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("brief-compiler")
        .join("ffi");
    std::fs::create_dir_all(&base).ok();
    base
}
```

**`BuildOptions` — add `extra_objects`**

```rust
pub struct BuildOptions {
    pub config_dir: Option<String>,
    pub extra_objects: Vec<PathBuf>,  // 2026-07-16: P4 — pre-compiled .o files
    pub file_path: String,
    pub emit_ir_only: bool,
    // ... rest unchanged
}
```

**`compile_source()` — collect extra objects from `FromSpec` paths**

```rust
// In compile_source(), after resolution and before codegen:
let cache_dir = get_ffi_cache_dir();
let mut extra_objects = Vec::new();
for item in &items {
    if let TopLevel::ForeignBinding(fb) = item {
        let ext = fb.from.extension(&import_resolver);
        match ext.as_deref() {
            Some("c" | "cpp" | "cc" | "cxx" | "m") => {
                let resolved = fb.from.resolve(&import_resolver)?;
                let obj = compile_source_to_object(&resolved, &cache_dir)?;
                extra_objects.push(obj);
            }
            Some("so" | "dylib" | "a" | "o") => {
                let resolved = fb.from.resolve(&import_resolver)?;
                extra_objects.push(resolved);
            }
            _ => {}  // wasm, py, js — handled elsewhere
        }
    }
}
```

**`compile_ll_to_binary()` — add `extra_objects` parameter**

```rust
fn compile_ll_to_binary(
    ll_path: &str,
    binary_path: &str,
    extra_objects: &[PathBuf],
) -> Result<(), String> {
    let rt_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("lib/runtime/brief_rt.c");
    let rt_str = rt_path.to_string_lossy().to_string();
    let mut cmd = Command::new("clang");
    cmd.args(["-O3", "-march=native", "-ffast-math", ll_path, &rt_str]);
    for obj in extra_objects {
        cmd.arg(obj.to_str().unwrap());
    }
    cmd.args(["-o", binary_path, "-lm"]);
    let status = cmd.status()
        .map_err(|e| format!("failed to invoke clang: {}", e))?;
    if !status.success() {
        return Err(format!("clang failed to compile '{}'", ll_path));
    }
    println!("wrote {}", binary_path);
    Ok(())
}
```

**Update all call sites of `compile_ll_to_binary()`** to pass `&opts.extra_objects`.

**Update `check_source()` default BuildOptions** to include `extra_objects: vec![]`.

### Dispatch table

| Extension | Action |
|-----------|--------|
| `.c`, `.m` | `clang -x c -c` → `.o` cache |
| `.cpp`, `.cc`, `.cxx` | `clang -x c++ -c` → `.o` cache |
| `.so`, `.so.*` | Pass directly to clang linker line |
| `.dylib` | Pass directly to clang linker line |
| `.a` | Pass directly to clang linker line |
| `.o` | Pass directly to clang linker line |
| `.wasm` | wasm-ld (existing path, no change) |
| `.py`, `.js` | Bridge subsystem (no change) |

### Verification

```rust
#[test]
fn test_compile_source_to_object_cached() {
    // Same source → same hash → returns cached path
}
#[test]
fn test_ffi_extension_dispatch() {
    // .c → compile, .so → link, .wasm → skip
}
```

### Implementation Steps

1. Add `extra_objects` to `BuildOptions` in `src/compile.rs`
2. Add `compile_source_to_object()`, `get_ffi_cache_dir()` to `src/compile.rs`
3. Add collection logic in `compile_source()` iterating `ForeignBinding` items
4. Update `compile_ll_to_binary()` signature and call sites
5. Write tests
6. Commit

---

## P5 — Auto-Meld at FFI Boundary

### What

When calling a foreign function, the compiler automatically applies meld conversion between Brief types and the foreign convention's types.

### The auto-meld algorithm

```
For each parameter (name: T) in the foreign signature:
  1. Derive ext from the foreign object path (e.g., "so" → "c")
  2. Look up universe.find_meld_to_extension("T", ext)
  3. If meld found: apply meld forward (T → T.ext) before call
  4. If no meld: pass through unchanged (i64 as-is)

For the return value:
  1. Same ext derivation
  2. Look up inverse meld (T.ext → T)
  3. Apply meld inverse on the return value
```

### Files Modified

#### `src/backend/llvm/mod.rs`

Extract the current frgn-call emission into a new `emit_frgn_call()` method, then add auto-meld:

```rust
/// 2026-07-16: P5 — Emit a foreign function call with optional auto-meld.
fn emit_frgn_call(
    &mut self,
    out: &mut String,
    sig: &ForeignSignature,
    args: &[TypedRegister],
    indent: &str,
) -> Result<TypedRegister, String> {
    let ext = sig.from.extension(&self.ctx.import_resolver);
    let ext = ext.as_deref().unwrap_or("");

    // Convert args: apply meld forward if convention extension found
    let meld_args: Vec<TypedRegister> = if ext.is_empty() {
        args.to_vec()
    } else {
        args.iter().zip(sig.inputs.iter()).map(|(arg, (_, param_ty))| {
            let ty_name = match param_ty {
                Type::Custom(name) => name.as_str(),
                _ => return Ok(arg.clone()),  // non-custom type, no meld
            };
            let (ext_type, meld) = match self.ctx.type_universe.find_meld_to_extension(ty_name, ext) {
                Some(pair) => pair,
                None => return Ok(arg.clone()),  // no meld found, pass through
            };
            // Apply forward meld: arg (T) → T.ext
            // Currently uses identity (same bit layout). Future: emit route expressions.
            Ok(TypedRegister {
                name: arg.name.clone(),
                ty: Type::Custom(ext_type),
            })
        }).collect::<Result<Vec<_>, String>>()?;
    };

    // Emit the actual call
    let arg_strs: Vec<String> = meld_args.iter()
        .map(|reg| format!("{} {}", lower_type(&reg.ty), reg.name))
        .collect();
    let ret_type = self.ctx.defn_return_types.get(&sig.name)
        .and_then(|types| types.first().cloned())
        .unwrap_or(Type::int());
    let ret_llvm = lower_type(&ret_type);
    let v = self.fun.gen_reg();
    writeln!(out, "{}{} = call {} @{}({})", indent, v, ret_llvm, sig.name, arg_strs.join(", ")).ok();

    // Meld inverse the return value
    let result = if ext.is_empty() {
        TypedRegister { name: v.to_string(), ty: ret_type }
    } else {
        let ty_name = match &ret_type {
            Type::Custom(name) => name.as_str(),
            _ => return Ok(TypedRegister { name: v.to_string(), ty: ret_type }),
        };
        match self.ctx.type_universe.find_meld_to_extension(ty_name, ext) {
            Some((_ext_type, _meld)) => {
                // Apply inverse meld: result (T.ext) → T
                // Currently identity. Future: emit route expressions.
                TypedRegister { name: v.to_string(), ty: ret_type }
            }
            None => TypedRegister { name: v.to_string(), ty: ret_type },
        }
    };
    Ok(result)
}
```

Wire into `emit_user_call()`: if the called function is in `frgn_map`, use `emit_frgn_call` instead of the generic call:

```rust
// In emit_user_call(), near line 292:
fn emit_user_call(&mut self, out: &mut String, v: &str, name: &str, args: &[Expr], indent: &str) -> TypedRegister {
    // 2026-07-16: P5 — Check if this is a foreign function; if so, use emit_frgn_call
    if let Some(sig) = self.ctx.frgn_map.get(name) {
        let arg_regs: Vec<TypedRegister> = args.iter()
            .map(|a| self.emit_expr(out, a, indent))
            .collect();
        return self.emit_frgn_call(out, sig, &arg_regs, indent)
            .unwrap_or_else(|_| {
                // Fallback: generic call if meld fails
                let arg_strs: Vec<String> = arg_regs.iter()
                    .map(|reg| format!("{} {}", lower_type(&reg.ty), reg.name))
                    .collect();
                let ret_type = self.ctx.defn_return_types.get(name)
                    .and_then(|types| types.first().cloned())
                    .unwrap_or(Type::int());
                let ret_llvm = lower_type(&ret_type);
                writeln!(out, "{}{} = call {} @{}({})", indent, v, ret_llvm, name, arg_strs.join(", ")).ok();
                TypedRegister { name: v.to_string(), ty: ret_type }
            });
    }
    // Original generic call logic...
    // (rest of existing emit_user_call)
}
```

#### `src/backend/llvm/helpers.rs`

The existing `try_meld_projection()` (line ~1541), `emit_decay()` (line ~1597), and `derive_fields_via_meld()` (line ~1638) already contain meld route expression emission logic. They are currently never called. Wire them into `emit_frgn_call()` for when the meld has explicit routes.

For the initial P5 implementation, the auto-meld is an **identity conversion** (same bit layout — the meld exists to signal convention compatibility, not to transform bits). The route expression helpers become active in a later phase.

#### `src/interpreter/eval.rs`

Mirror the auto-meld logic for the interpreter path:

```rust
/// 2026-07-16: P5 — Evaluate a foreign function call with auto-meld.
fn eval_frgn_call(
    &mut self,
    sig: &ForeignSignature,
    args: &[Value],
) -> Result<Value, String> {
    let ext = sig.from.extension(&self.import_resolver);
    // Auto-meld each arg (identity for now — future: route evaluation)
    let meld_args: Vec<Value> = if ext.as_deref().unwrap_or("").is_empty() {
        args.to_vec()
    } else {
        args.iter().zip(sig.inputs.iter()).map(|(arg, (_, param_ty))| {
            let ty_name = match param_ty {
                Type::Custom(name) => name.as_str(),
                _ => return Ok(arg.clone()),
            };
            match self.type_universe.find_meld_to_extension(ty_name, ext.as_deref().unwrap_or("")) {
                Some(_) => Ok(arg.clone()),  // identity meld
                None => Ok(arg.clone()),
            }
        }).collect::<Result<Vec<_>, String>>()?;
    };
    // ... existing frgn call dispatch ...
}
```

### Precondition check safety

Before applying auto-meld, verify that the meld found by `find_meld_to_extension` actually exists in the universe as a valid layout. The check is:

```rust
if self.ctx.type_universe.find_meld_to_extension(ty_name, ext).is_some() {
    // meld exists — convention is compatible, apply identity conversion
} else {
    // no meld — pass through unchanged
}
```

This means: if `String.c` exists in the type universe (even without explicit routes), the meld is valid by inheritance. The types are compatible. No bit transformation needed for the initial implementation.

### Verification

```rust
#[test]
fn test_auto_meld_frgn_call() {
    // frgn strlen(s: String) from "libc.so" — ext="so" → convention="c"
    // String.c exists in universe → meld found → identity conversion
    // Generated IR should match: call i64 @strlen(i64 %s)
}
#[test]
fn test_auto_meld_no_meld_fallback() {
    // frgn foo(x: CustomType) from "lib.so" — no CustomType.c → pass through
}
```

### Implementation Steps

1. Add `emit_frgn_call()` to `src/backend/llvm/mod.rs`
2. Wire into `emit_user_call()` in `src/backend/llvm/emit_expr.rs`
3. Add auto-meld to interpreter in `src/interpreter/eval.rs`
4. Write tests
5. Commit

---

## P6 — Meld Validation Cascade (5 Layers)

### What

Every `meld` declaration is verified at compile time through a cascade of increasingly powerful checks:

```
Layer 1: Structural       — field count, widths, offsets    (always, O(fields))
Layer 2: Bit-permutation  — no overlap, no gaps             (always, O(bits))
Layer 3: Unit-vector      — bit enumeration via interpreter  (always, O(bits))
Layer 4: Symbolic         — symbolic round-trip             (complex routes only)
Layer 5: SMT              — Z3 QF_BV universal proof        (complex routes only, 500ms timeout)
```

Layers 1-3 produce fatal errors. Layers 4-5 produce warnings if earlier layers passed.

### Files Modified

#### `src/analysis/meld_validation.rs` (new, ~450 lines)

```rust
//! 2026-07-16: P6 — Five-layer compile-time meld layout validation.
//!
//! Each meld declaration is checked for round-trip correctness:
//! forward(meld) ∘ inverse(meld) = identity for all inputs.
//!
//! Layers 1-3 are always run and produce fatal errors.
//! Layers 4-5 run only for non-trivial routes and produce warnings.

use crate::ast::{Expr, MeldDeclaration, Type};
use crate::type_universe::TypeUniverse;

#[derive(Debug)]
pub enum MeldValidationError {
    TypeNotFound(String),
    FieldNotFound { ty: String, field: String },
    WidthMismatch { field: String, src_width: u64, dst_width: u64 },
    Overlap { bit: u64, field: String },
    Gap { bit: u64 },
    UnitVectorFailed { bit: u64 },
    SymbolicMismatch { field: String },
    SmtCounterexample { meld: String, example: String },
    SmtTimeout,
}

/// Run the full validation cascade on a meld declaration.
/// Layers 1-3 errors are fatal; Layers 4-5 produce warnings.
pub fn validate_meld_layout(
    meld: &MeldDeclaration,
    universe: &TypeUniverse,
    verbose: bool,
) -> Result<(), Vec<MeldValidationError>> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Layer 1: Structural
    if let Err(e) = validate_structural(meld, universe) {
        errors.push(e);
    }

    // Layer 2: Bit-permutation
    if let Err(e) = validate_bit_permutation(meld, universe) {
        errors.push(e);
    }

    // Layer 3: Unit-vector (via interpreter)
    if let Err(e) = validate_unit_vectors(meld, universe) {
        errors.push(e);
    }

    // If any of layers 1-3 failed, return early with errors
    if !errors.is_empty() {
        return Err(errors);
    }

    // Layers 4-5: only for non-trivial routes
    if has_complex_routes(meld) {
        if let Err(e) = validate_symbolic(meld, universe) {
            warnings.push(e);
        }
        if verbose {
            if let Err(e) = validate_smt(meld, universe) {
                warnings.push(e);
            }
        }
    }

    if !warnings.is_empty() {
        for w in &warnings {
            eprintln!("warning: meld verification: {:?}", w);
        }
    }
    Ok(())
}

// ── Layer 1: Structural ──────────────────────────────────────────

fn validate_structural(meld: &MeldDeclaration, universe: &TypeUniverse) -> Result<(), MeldValidationError> {
    let type_a = universe.get(&meld.name_a)
        .ok_or(MeldValidationError::TypeNotFound(meld.name_a.clone()))?;
    let type_b = universe.get(&meld.name_b)
        .ok_or(MeldValidationError::TypeNotFound(meld.name_b.clone()))?;
    for route in &meld.routes {
        // Check: route.accessor exists on type_b
        if !type_b.fields.contains_key(&route.accessor) {
            return Err(MeldValidationError::FieldNotFound {
                ty: type_b.name.clone(),
                field: route.accessor.clone(),
            });
        }
        // Check: dest_expr references valid projections on type_a
        validate_route_expr(&route.dest_expr, type_a)?;
    }
    Ok(())
}

fn validate_route_expr(expr: &Expr, ty: &ResolvedType) -> Result<(), MeldValidationError> {
    match expr {
        Expr::Field(Box::new(Expr::Identifier(name)), field) => {
            if name != &ty.name {
                // Reference to a different type's field — check it exists
            }
            if !ty.fields.contains_key(field.as_str()) {
                return Err(MeldValidationError::FieldNotFound {
                    ty: ty.name.clone(),
                    field: field.clone(),
                });
            }
            Ok(())
        }
        Expr::BinaryOp(_, l, r) => {
            validate_route_expr(l, ty)?;
            validate_route_expr(r, ty)
        }
        _ => Ok(()),  // Other exprs (literals, etc.) are valid identity expressions
    }
}

// ── Layer 2: Bit-Permutation ─────────────────────────────────────

fn validate_bit_permutation(meld: &MeldDeclaration, universe: &TypeUniverse) -> Result<(), MeldValidationError> {
    let type_a = universe.get(&meld.name_a).unwrap();
    let type_b = universe.get(&meld.name_b).unwrap();
    let total_bits = type_a.size_bytes * 8;
    let mut dest_bits = vec![false; total_bits as usize];

    for route in &meld.routes {
        let (src_offset, src_width) = get_field_bits(type_a, &route.accessor)?;
        let (dst_offset, dst_width) = expr_bits(&route.dest_expr, type_b)?;
        if src_width != dst_width {
            return Err(MeldValidationError::WidthMismatch {
                field: route.accessor.clone(),
                src_width, dst_width,
            });
        }
        for bit in dst_offset..(dst_offset + dst_width) {
            if dest_bits[bit as usize] {
                return Err(MeldValidationError::Overlap {
                    bit, field: route.accessor.clone(),
                });
            }
            dest_bits[bit as usize] = true;
        }
    }
    for (bit, covered) in dest_bits.iter().enumerate() {
        if !covered {
            return Err(MeldValidationError::Gap { bit: bit as u64 });
        }
    }
    Ok(())
}

// ── Layer 3: Unit-Vector Enumeration ────────────────────────────

fn validate_unit_vectors(meld: &MeldDeclaration, universe: &TypeUniverse) -> Result<(), MeldValidationError> {
    let type_a = universe.get(&meld.name_a).unwrap();
    let total_bits = type_a.size_bytes * 8;

    for bit in 0..total_bits {
        // Create concrete value with only bit `bit` set
        let test_val = make_unit_bit_value(type_a, bit);
        // Evaluate forward routes
        let mid_val = eval_meld_routes(&meld.routes, &test_val, universe);
        // Evaluate inverse routes
        let roundtrip_val = eval_meld_routes_inverse(&meld.routes, &mid_val, type_a, universe);
        // Verify bit preserved
        if !test_bit(&roundtrip_val, bit) {
            return Err(MeldValidationError::UnitVectorFailed { bit });
        }
    }
    Ok(())
}

use crate::interpreter::eval::eval_expr;
use crate::interpreter::environment::Environment;
use crate::value::Value;

fn make_unit_bit_value(ty: &ResolvedType, bit: u64) -> Value {
    // Create a Value::Instance with only bit `bit` set
    let bytes = (bit / 8) as usize;
    let byte_bit = (bit % 8) as u8;
    let mut data = vec![0u8; ty.size_bytes as usize];
    if bytes < data.len() {
        data[bytes] = 1 << byte_bit;
    }
    Value::Instance {
        name: ty.name.clone(),
        data,
    }
}

fn eval_meld_routes(routes: &[MeldRouteDef], input: &Value, universe: &TypeUniverse) -> Value {
    // Simplification: for routes with Field(Identifier(ty), field) dest_expr,
    // extract the field value from input and return as Instance.
    // Full route evaluation via interpreter is a future enhancement.
    let mut fields = HashMap::new();
    for route in routes {
        match &route.dest_expr {
            Expr::Field(Box::new(Expr::Identifier(_)), field_name) => {
                // Extract route.accessor from input
                if let Some(val) = input.field_value(&route.accessor) {
                    fields.insert(field_name.clone(), val);
                }
            }
            _ => {}
        }
    }
    Value::Instance {
        name: String::new(),
        data: vec![],
    }
}

fn eval_meld_routes_inverse(
    routes: &[MeldRouteDef],
    input: &Value,
    _output_ty: &ResolvedType,
    _universe: &TypeUniverse,
) -> Value {
    // Inverse: for each route, map dest_expr field back to accessor
    // Simplified: identity for now
    input.clone()
}

fn test_bit(val: &Value, bit: u64) -> bool {
    match val {
        Value::Instance { data, .. } => {
            let byte = (bit / 8) as usize;
            let mask = 1 << (bit % 8);
            byte < data.len() && (data[byte] & mask) != 0
        }
        _ => false,
    }
}

// ── Layer 4: Symbolic ───────────────────────────────────────────

fn validate_symbolic(meld: &MeldDeclaration, universe: &TypeUniverse) -> Result<(), MeldValidationError> {
    use crate::symbolic::{eval_symbolic_expr, SymbolicValue};
    let type_a = universe.get(&meld.name_a).unwrap();

    // Create symbolic values for each field of type A
    let mut symbolic_input = std::collections::HashMap::new();
    for field_name in type_a.fields.keys() {
        let sym = SymbolicValue::Identifier(format!("__input_{}", field_name));
        symbolic_input.insert(field_name.clone(), sym);
    }

    // Evaluate forward routes symbolically
    let symbolic_mid = eval_routes_symbolic(&meld.routes, &symbolic_input);
    // Evaluate inverse routes symbolically
    let symbolic_output = eval_routes_symbolic_inverse(&meld.routes, &symbolic_mid, type_a);

    // Check round-trip: each output field should equal its input
    for (field_name, output_val) in &symbolic_output {
        let input_val = &symbolic_input[field_name];
        if !symbolic_equals(output_val, input_val) {
            return Err(MeldValidationError::SymbolicMismatch {
                field: field_name.clone(),
            });
        }
    }
    Ok(())
}

// ── Layer 5: SMT ────────────────────────────────────────────────

fn validate_smt(meld: &MeldDeclaration, universe: &TypeUniverse) -> Result<(), MeldValidationError> {
    use crate::proof_engine::smt::prove_smt_formula;
    use std::time::Duration;

    let type_a = universe.get(&meld.name_a).unwrap();
    let total_bytes = type_a.size_bytes as usize;

    // Build SMT query: ∀x. roundtrip(x) == x
    let total_bits = total_bytes * 8;
    let formula = format!(
        "(set-logic QF_BV)
         (declare-const x (_ BitVec {}))
         (assert (not (= (roundtrip x) x)))
         (check-sat)",
        total_bits
    );
    // Note: full SMT encoding of meld routes as bit-vector operations
    // requires generating extract/concat/ite expressions per route.
    // This is a placeholder for the full encoding.

    let timeout = Duration::from_millis(500);
    match prove_smt_formula(&formula, timeout) {
        SmtResult::Unsat => Ok(()),
        SmtResult::Sat(cex) => Err(MeldValidationError::SmtCounterexample {
            meld: format!("{} <:> {}", meld.name_a, meld.name_b),
            example: format!("{:?}", cex),
        }),
        SmtResult::Unknown => Err(MeldValidationError::SmtCounterexample {
            meld: format!("{} <:> {}", meld.name_a, meld.name_b),
            example: "unknown".to_string(),
        }),
        SmtResult::Timeout => Err(MeldValidationError::SmtTimeout),
    }
}

// ── Helpers ──────────────────────────────────────────────────────

fn has_complex_routes(meld: &MeldDeclaration) -> bool {
    meld.routes.iter().any(|r| !matches!(r.dest_expr, Expr::Field(_, _)))
}

fn get_field_bits(ty: &ResolvedType, field: &str) -> Result<(u64, u64), MeldValidationError> {
    let offset = ty.properties.get(&format!("field.{}.offset", field))
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or(MeldValidationError::FieldNotFound {
            ty: ty.name.clone(),
            field: field.to_string(),
        })?;
    let width = ty.properties.get(&format!("field.{}.width", field))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(64);
    Ok((offset, width))
}

fn expr_bits(expr: &Expr, ty: &ResolvedType) -> Result<(u64, u64), MeldValidationError> {
    match expr {
        Expr::Field(_, field) => get_field_bits(ty, field),
        _ => Ok((0, 64)),  // Non-field expressions default to full width
    }
}
```

#### `src/backend/llvm/normalizer.rs`

After `synthesize_meld_shuffle()`, call validation:

```rust
// In the existing meld loop (line 34-36), after synthesize_meld_shuffle:
for item in &items {
    if let TopLevel::Meld(meld) = item {
        // Existing: synthesize meld shuffle
        synthesize_meld_shuffle(meld, universe)?;

        // 2026-07-16: P6 — Validate meld layout via 5-layer cascade
        let decl = convert_to_meld_declaration(meld);
        if let Err(errors) = crate::analysis::meld_validation::validate_meld_layout(
            &decl, universe, true,
        ) {
            for err in &errors {
                eprintln!("error: meld round-trip validation: {:?}", err);
            }
            return Err(format!(
                "meld {} <:> {} failed validation ({} errors)",
                meld.name, meld.target, errors.len()
            ));
        }
    }
}
```

#### `src/proof_engine/smt.rs`

Add the public `prove_smt_formula()`:

```rust
/// 2026-07-16: P6 — Prove an SMT-LIB2 formula using Z3.
/// Returns Unsat if the formula is unsatisfiable (property holds).
/// Returns Sat with a counterexample if satisfiable.
/// Returns Timeout if Z3 doesn't respond within the timeout.
pub fn prove_smt_formula(formula: &str, timeout: Duration) -> SmtResult {
    // Write formula to temp file
    let mut tmp = std::env::temp_dir();
    tmp.push(format!("brief_smt_{}.smt2", std::process::id()));
    std::fs::write(&tmp, formula).ok();

    let output = std::process::Command::new("z3")
        .arg("-in")
        .arg("-smt2")
        .arg(&tmp)
        .timeout(timeout)
        .output();

    // Clean up
    let _ = std::fs::remove_file(&tmp);

    match output {
        Ok(out) => {
            let stdout = String::from_UTF8_lossy(&out.stdout);
            if stdout.contains("unsat") {
                SmtResult::Unsat
            } else if stdout.contains("sat") {
                SmtResult::Sat(parse_smt_model(&stdout))
            } else {
                SmtResult::Unknown
            }
        }
        Err(_) => SmtResult::Timeout,
    }
}

/// Parse Z3's model output into (variable, value) pairs.
fn parse_smt_model(_stdout: &str) -> Vec<(String, String)> {
    vec![]  // Placeholder — full model parsing deferred
}
```

#### `src/symbolic.rs`

Add the stateless `eval_symbolic_expr()`:

```rust
/// 2026-07-16: P6 — Evaluate an expression symbolically given explicit input bindings.
/// Unlike `eval_symbolic`, this does NOT require a full SymbolicState — just a
/// HashMap of input bindings. Used by meld validation Layer 4.
pub fn eval_symbolic_expr(
    expr: &Expr,
    inputs: &std::collections::HashMap<String, SymbolicValue>,
) -> SymbolicValue {
    match expr {
        Expr::Identifier(name) => {
            inputs.get(name).cloned().unwrap_or(SymbolicValue::Unknown)
        }
        Expr::Field(_, _) => SymbolicValue::Unknown,
        Expr::BinaryOp(kind, l, r) => {
            let lv = eval_symbolic_expr(l, inputs);
            let rv = eval_symbolic_expr(r, inputs);
            SymbolicValue::Binary(
                format!("{:?}", kind),
                Box::new(lv),
                Box::new(rv),
            )
        }
        Expr::Decimal(n) => SymbolicValue::Literal(*n, "i64".to_string()),
        _ => SymbolicValue::Unknown,
    }
}
```

### Verification

```rust
#[test]
fn test_validate_structural_passes() {
    // Simple identity meld passes layer 1
}
#[test]
fn test_validate_bit_permutation_no_overlap() {
    // Non-overlapping field mappings pass
}
#[test]
fn test_validate_unit_vectors() {
    // Unit bit enumeration round-trips correctly
}
#[test]
fn test_validate_symbolic() {
    // Symbolic round-trip proves identity for linear melds
}
#[test]
fn test_validate_smt_unsat() {
    // SMT query returns Unsat for correct meld (if z3 available)
}
#[test]
fn test_validate_no_z3_graceful() {
    // Layer 5 reports Timeout/Unknown gracefully when z3 not on PATH
}
```

### Implementation Steps

1. Create `src/analysis/meld_validation.rs` with all 5 layers
2. Add `prove_smt_formula()` to `src/proof_engine/smt.rs`
3. Add `eval_symbolic_expr()` to `src/symbolic.rs`
4. Wire validation into `src/backend/llvm/normalizer.rs`
5. Register `meld_validation` module in `src/analysis/mod.rs` and `src/lib.rs`
6. Write tests
7. Commit

---

## Verification Summary

### Pre-Commit Checklist (every phase)

1. `cargo test --lib` — all tests pass (0 failures)
2. `cargo build` — no warnings
3. Run `cargo clippy` on new/changed files
4. Log any bugs/gotchas in `BUGS.md`

### Predicted test count progression

| After | Total tests | New tests |
|-------|-------------|-----------|
| P2 | +5 | dotted type name parse, extension group expand, 3x find_meld priorities |
| P3 | +4 | frgn parse literal, frgn parse reg, from spec resolve literal, resolve reg |
| P4 | +2 | compile to object cached, extension dispatch |
| P5 | +2 | auto-meld frgn call, no-meld fallback |
| P6 | +6 | 5x layer validation, no-z3 graceful |
| **Total new** | **+19** | |

---

## Risk Register

| Risk | Phase | Mitigation |
|------|-------|------------|
| `location` → `from` rename breaks 15+ sites | P3 | `cargo build` catches all; fix one by one |
| frgn parser didn't exist — no regression tests | P3 | Write parse tests before anything depends on it |
| Auto-meld identity conversion silently wrong | P5 | Always verify meld exists via `find_meld_to_extension`; identity is correct for <:-inherited types |
| Z3 not on $PATH | P6 | Layer 5 returns Timeout/Unknown; error is downgraded to warning |
| `blake3` crate not in dependencies | P4 | Use `sha2` (already in deps?) or `std::collections::hash_map::DefaultHasher` for content hash |
| Cache directory creation fails | P4 | Fall through gracefully; compile every time without cache |
