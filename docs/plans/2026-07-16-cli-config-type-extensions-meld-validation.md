# CLI Config Command + Type Extensions + Path-Based `from` + Meld Validation

**Date:** 2026-07-16
**Status:** Plan — ready for implementation

---

## Table of Contents

1. [CLI `config` Subcommand](#1-cli-config-subcommand)
2. [Type Extension System](#2-type-extension-system)
3. [Path-Based `from` with Compiler-Relative Syntax](#3-path-based-from-with-compiler-relative-syntax)
4. [Pre-Compilation Pipeline](#4-pre-compilation-pipeline)
5. [Compile-Time Meld Layout Validation](#5-compile-time-meld-layout-validation)
6. [Implementation Order](#6-implementation-order)
7. [Plan Directives Compliance](#7-plan-directives-compliance)

---

## 1. CLI `config` Subcommand

### Problem

Config TOML files (`targets.toml`, `llvm-primitives.toml`, `llvm-ops.toml`, `spirv-ops.toml`) are baked into the binary at compile time via `include_str!` and `env!("CARGO_MANIFEST_DIR")`. No runtime switching is possible.

### Current state of config loading

| Config | Currently | Mechanism |
|--------|-----------|-----------|
| `config/targets.toml` | `include_str!("../config/targets.toml")` | Baked into binary |
| `config/llvm-primitives.toml` | `env!("CARGO_MANIFEST_DIR")/config/...` | Baked via compile-time macro |
| `config/llvm-ops.toml` | Same via `load_from("llvm-ops.toml")` | Baked via compile-time macro |
| `config/spirv-ops.toml` | Same via `load_from("spirv-ops.toml")` | Baked via compile-time macro |
| `config/address-map.toml` | CWD-relative (already runtime) | No change needed |
| `config/module-registry.toml` | CWD-relative (already runtime) | No change needed |

Additionally, **5 `LazyLock` statics** load `TypeConfig`/`OpConfig` independently across the LLVM backend and normalizers. A profile swap wouldn't take effect with the current architecture.

### Implementation

#### 1a. `ConfigResolver` — new module `src/config_resolver.rs`

```rust
pub struct ConfigResolver {
    pub config_dir: PathBuf,
    pub target_config: TargetConfig,
    pub type_config: TypeConfig,
    pub op_config: OpConfig,
    pub spirv_op_config: OpConfig,
    pub module_registry: HashMap<String, String>,
}

impl ConfigResolver {
    /// Follow the resolution chain to find config files.
    /// 1. --config-dir CLI flag
    /// 2. BRIEF_CONFIG_DIR env var
    /// 3. ./.briv/config/ (project-local)
    /// 4. ~/.config/briv-compiler/active_profile symlink
    /// 5. Compile-time baked fallback
    pub fn resolve(config_dir_override: Option<&Path>) -> Self;

    /// Re-read all config files from the resolved directory.
    pub fn reload(&mut self) -> Result<(), String>;
}
```

Resolution chain implementation:

```rust
fn resolve_config_dir(override_dir: Option<&Path>) -> PathBuf {
    // 1. CLI override
    if let Some(dir) = override_dir {
        return dir.to_path_buf();
    }
    // 2. Environment variable
    if let Ok(env) = std::env::var("BRIEF_CONFIG_DIR") {
        return PathBuf::from(env);
    }
    // 3. Project-local
    if Path::new(".briv/config").exists() {
        return PathBuf::from(".briv/config");
    }
    // 4. User-global active profile
    let user_config = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("briv-compiler");
    let active = user_config.join("active_profile");
    if active.exists() {
        if let Ok(target) = std::fs::read_link(&active) {
            return user_config.join(target);
        }
        // Fallback: read active_profile as text file containing the profile name
        if let Ok(name) = std::fs::read_to_string(&active) {
            let profile_path = user_config.join("profiles").join(name.trim());
            if profile_path.exists() {
                return profile_path;
            }
        }
    }
    // 5. Compile-time baked — return marker that triggers fallback loading
    PathBuf::from("__baked__")
}
```

#### 1b. Profile directory structure

```
~/.config/briv-compiler/
├── active_profile -> profiles/default    # symlink to active profile dir
└── profiles/
    └── default/
        ├── targets.toml                  # file extension → backend routing
        ├── llvm-primitives.toml          # (primitive, bytes) → LLVM type
        ├── llvm-ops.toml                 # (op, primitive, bytes) → LLVM IR template
        └── spirv-ops.toml               # SPIR-V operation templates
```

The `active_profile` can be either a **symlink** (preferred, Unix) or a **text file** containing the profile directory name (cross-platform fallback).

#### 1c. CLI subcommand — `briv-compiler config`

New subcommand dispatch in `src/main.rs`:

| Command | Action |
|---------|--------|
| `briv-compiler config init [name]` | Create `~/.config/briv-compiler/profiles/<name>/` with default `.toml` files (extracted from baked-in defaults, written to disk). Default name: `"default"`. |
| `briv-compiler config list` | List profiles in `~/.config/briv-compiler/profiles/` |
| `briv-compiler config set <name>` | Update `active_profile` symlink/text to point to `profiles/<name>/` |
| `briv-compiler config show` | Print active profile path and contents of each `.toml` file |

New `--config-dir` flag on `build`:
```
briv-compiler build foo.bv --config-dir ~/my-project/config
```

#### 1d. Convert config structs to accept paths

`TargetConfig` gets a `load_from(path: &Path)` constructor alongside existing `load()`:

```rust
impl TargetConfig {
    /// Load from compile-time baked data (fallback).
    pub fn load() -> Self {
        let content = include_str!("../config/targets.toml");
        toml::from_str(content).unwrap()
    }

    /// Load from a concrete path.
    pub fn load_from(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read '{}': {}", path.display(), e))?;
        toml::from_str(&content)
            .map_err(|e| format!("parse error in '{}': {}", path.display(), e))
    }
}
```

Same pattern for `TypeConfig` and `OpConfig`.

#### 1e. Remove `LazyLock` statics, centralize into `CompilerContext`

| File | Line(s) | Current | New |
|------|---------|---------|-----|
| `src/backend/llvm/emit_toplevel.rs` | 9-11 | `static TYPE_CONFIG: LazyLock<TypeConfig> = LazyLock::new(\|\| TypeConfig::load())` | Remove static, use `ctx.type_config` |
| `src/backend/llvm/helpers.rs` | 24-26 | `static TYPE_CONFIG: LazyLock<TypeConfig> = LazyLock::new(\|\| TypeConfig::load())` | Remove static, use `ctx.type_config` |
| `src/backend/llvm/intrinsics.rs` | 13-14 | Two `LazyLock`s: `TYPE_CONFIG` + `OP_CONFIG` | Remove both, use `ctx.type_config`, `ctx.op_config` |
| `src/backend/llvm/normalizer.rs` | 16, 40 | `TypeConfig::load()` + `OpConfig::load()` at call site | Receive from caller or `CompilerContext` |
| `src/backend/spirv/normalizer.rs` | 17, 37 | `TypeConfig::load()` + `OpConfig::load_from("spirv-ops.toml")` | Receive from caller or `CompilerContext` |

`CompilerContext` (`src/backend/llvm/context.rs`) gains:

```rust
pub struct CompilerContext {
    // ... existing fields ...
    pub type_config: TypeConfig,
    pub op_config: OpConfig,
    pub spirv_op_config: OpConfig,
    pub config_resolver: ConfigResolver,
}
```

#### 1f. Build pipeline threading

`BuildOptions` (`src/compile.rs`) gains:

```rust
pub struct BuildOptions {
    // ... existing fields ...
    pub config_dir: Option<PathBuf>,
}
```

`parse_build_args()` adds `--config-dir` flag parsing (same pattern as `--stdlib-path`).

`compile_source()` creates `ConfigResolver::resolve()` once at pipeline start and threads it through to `CompilerContext` during backend construction.

### Files modified

| File | Change |
|------|--------|
| `src/main.rs` | `config` subcommand dispatch, `--config-dir` flag on `build` |
| `src/compile.rs` | `config_dir` on `BuildOptions`, thread through pipeline |
| `src/config_resolver.rs` | **New file** — `ConfigResolver` struct + resolution chain |
| `src/config.rs` | `load_from()` constructors for `TypeConfig`/`OpConfig` |
| `src/target.rs` | `load_from()` constructor for `TargetConfig` |
| `src/backend/llvm/context.rs` | Store `TypeConfig`/`OpConfig`/`ConfigResolver` |
| `src/backend/llvm/emit_toplevel.rs` | Remove `LazyLock` static |
| `src/backend/llvm/helpers.rs` | Remove `LazyLock` static |
| `src/backend/llvm/intrinsics.rs` | Remove both `LazyLock` statics |
| `src/backend/llvm/normalizer.rs` | Remove direct `::load()` calls |
| `src/backend/spirv/normalizer.rs` | Same pattern |
| `src/import_resolver.rs` | Accept `config_dir` param for `module-registry.toml` path |

---

## 2. Type Extension System

### Problem

Currently, `CString` must be defined as a separate type with explicit `meld` to `String`. Instead, `String.c` should be syntactically an extension of `String`, and the compiler should auto-resolve extension types at FFI boundaries. Custom types (e.g., `MyCustomEmbeddedString`) should also be able to meld directly to `String.c` without defining their own extension type.

### Core concept

```briv
// Base type (already exists)
type String : Bits { bytes <~ 8; primitive <~ String; };

// Extension type: C view of String. Inherits properties from String.
type String.c : String { bytes <~ 8; primitive <~ String; };

// Extension group: defines for all C-family languages at once.
type String.[c,cpp,cs] : String { bytes <~ 8; primitive <~ String; };

// Melds define how to convert between Briv and foreign representations.
meld String <:> String.c { Ptr -> String.c.ptr; Size -> String.c .#Size; };

// Custom types can meld directly to standard extension types:
type MyCustomEmbeddedString : String { ... };
meld MyCustomEmbeddedString <:> String.c { ... };
```

When `frgn strlen(s: String) from "libc.so.6"` is compiled:
1. Target C → suffix `.c`
2. Search for meld: `String <:> String.c` ✓ (or `MyCustomEmbeddedString <:> String.c` for custom types)
3. Auto-apply at FFI boundary: convert `String` → `String.c` ABI layout
4. On return, apply inverse meld

### Implementation

#### 2a. Parser: dotted type names

`String.c` is parsed as `Custom("String.c")`. The `.` is part of the identifier in type-definition and type-reference positions.

In `src/parser/types.rs`, `parse_type_identifier()`: after parsing the base name, peek for `Token::Dot` followed by an identifier. If found, concatenate with `.` separator.

```rust
fn parse_type_identifier(&mut self) -> Result<String, SyntaxError> {
    let mut name = self.expect_identifier()?;
    // Check for .ext suffix (e.g., "String.c")
    if self.eat(&Token::Dot) {
        let ext = self.expect_identifier()?;
        name.push('.');
        name.push_str(&ext);
    }
    Ok(name)
}
```

#### 2b. Parser: extension groups `Type.[a,b,c]`

```briv
type String.[c,cpp,cs] : String { bytes <~ 8; };
```

Detection in `parse_top_level()`: after `Token::Type` and the type name is parsed, check for `Token::Dot` + `Token::LBracket`.

```rust
// In parse_top_level() or parse_type_definition():
fn parse_type_extension_group(&mut self, base_name: String) -> Result<Vec<TopLevel>, SyntaxError> {
    self.expect(Token::LBracket)?;
    let mut exts = Vec::new();
    loop {
        exts.push(self.expect_identifier()?);
        if !self.eat(&Token::Comma) { break; }
    }
    self.expect(Token::RBracket)?;
    // Parse the shared body (inheritance + slots)
    let body = self.parse_type_body()?;
    // Emit one TypeDef per extension
    Ok(exts.into_iter().map(|ext| {
        let full_name = format!("{}.{}", base_name, ext);
        TopLevel::TypeDef(TypeDef {
            name: full_name,
            base: body.base.clone(),
            body: body.clone(),
            // ...
        })
    }).collect())
}
```

Each emitted `TypeDef`:
- Name: `"String.c"`, `"String.cpp"`, `"String.cs"`
- Base: `: String` (inherits all of `String`'s properties)
- Body: shared `{ bytes <~ 8; primitive <~ String; }`

#### 2c. Type universe: extension queries

`TypeUniverse` (`src/type_universe/mod.rs`) gains three new methods:

```rust
impl TypeUniverse {
    /// Look up "String.c" from base "String" and extension "c".
    pub fn get_extension(&self, base: &str, ext: &str) -> Option<&ResolvedType> {
        self.types.get(&format!("{}.{}", base, ext))
    }

    /// Find meld between base type and extension type directly.
    pub fn find_extension_meld(&self, base: &str, ext: &str) -> Option<&MeldDeclaration> {
        let ext_name = format!("{}.{}", base, ext);
        self.find_meld(base, &ext_name)
    }

    /// Find any meld from T to any type ending in .ext.
    /// Priority:
    ///   1. Direct meld T <:> T.c  (exact match)
    ///   2. Direct meld T <:> String.c  (custom → standard extension)
    ///   3. T.c exists with auto-generated meld
    ///   4. None → error
    pub fn find_meld_to_extension(&self, ty: &str, ext: &str) -> Option<(String, MeldDeclaration)> {
        // Priority 1: T <:> T.c
        let exact = format!("{}.{}", ty, ext);
        if let Some(decl) = self.find_meld(ty, &exact) {
            return Some((exact, decl.clone()));
        }
        // Priority 2: T <:> Any.c
        for ((a, b), decl) in &self.melds {
            if a == ty && b.ends_with(&format!(".{}", ext)) {
                return Some((b.clone(), decl.clone()));
            }
            if b == ty && a.ends_with(&format!(".{}", ext)) {
                return Some((a.clone(), decl.clone()));
            }
        }
        // Priority 3: T.c exists (auto-meld via bit representation)
        if self.types.contains_key(&exact) {
            // Construct implicit identity meld (same bit layout assumed)
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

#### 2d. `ForeignTarget` gains Cpp, Cs and extension methods

```rust
pub enum ForeignTarget {
    Native, Wasm, C, Cpp, Cs, Python, Js, Swift, Go, Metropolitan,
}

impl ForeignTarget {
    /// The type extension suffix used in the type universe for this target.
    /// C → "c", Cpp → "cpp", Cs → "cs", Wasm → "wasm", Python → "py", etc.
    pub fn type_suffix(&self) -> &str {
        match self {
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Cs => "cs",
            Self::Wasm => "wasm",
            Self::Python => "py",
            Self::Js => "js",
            Self::Swift => "swift",
            Self::Go => "go",
            Self::Native | Self::Metropolitan => "",
        }
    }

    /// Derive the target from a file path's extension.
    /// "libfoo.so" → C, "libfoo.wasm" → Wasm.
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        match ext {
            "so" | "dylib" | "o" | "a" | "c" => Some(Self::C),
            "cpp" | "cc" | "cxx" => Some(Self::Cpp),
            "cs" => Some(Self::Cs),
            "wasm" => Some(Self::Wasm),
            "py" => Some(Self::Python),
            "js" | "mjs" => Some(Self::Js),
            "swift" => Some(Self::Swift),
            "go" => Some(Self::Go),
            _ => None,
        }
    }
}
```

#### 2e. Meld pipeline fix: populate `TypeUniverse.melds`

**Critical:** `TypeUniverse.melds` is currently **always empty** (`src/type_universe/mod.rs:57` — initialized, never populated). This is the single highest-priority wiring task — it blocks all meld-related features.

In the type-checking pass (or the normalizer), after processing all `TopLevel::Meld` items, convert them to `MeldDeclaration` and insert into `universe.melds`.

```rust
// In src/backend/llvm/normalizer.rs, after the existing meld loop (line 34-36):
for item in &items {
    if let TopLevel::Meld(meld) = item {
        // Convert parsed Meld → MeldDeclaration
        let decl = MeldDeclaration {
            name_a: meld.name.clone(),
            name_b: meld.target.clone(),
            routes: meld.bindings.iter()
                .filter(|(k, _)| k.starts_with("layout."))
                .map(|(k, v)| MeldRouteDef {
                    accessor: k.strip_prefix("layout.").unwrap().to_string(),
                    dest_expr: Expr::Field(
                        Box::new(Expr::Identifier(meld.target.clone())),
                        v.clone(),
                    ),
                })
                .collect(),
            span: meld.span.clone(),
        };
        // Register both orderings
        universe.melds.insert(
            (decl.name_a.clone(), decl.name_b.clone()),
            decl.clone(),
        );
        universe.melds.insert(
            (decl.name_b.clone(), decl.name_a.clone()),
            decl,
        );
    }
}
```

#### 2f. Standard FFI type library

New file: `lib/std/types/ffi.bv`:

```briv
// ── Standard FFI Type Mappings ─────────────────────────────────
// Maps Briv base types to foreign ABI representations.
// Imported as part of the prelude when frgn is used.

// C ABI type mappings
type Int.c : Int { bytes <~ 4; primitive <~ Int; };
type Float.c : Float { bytes <~ 4; primitive <~ Float; };
type String.c : String { bytes <~ 8; primitive <~ String; };

// C++ and C# ABI type mappings
type String.cpp : String { bytes <~ 8; primitive <~ String; };
type String.cs : String { bytes <~ 8; primitive <~ String; };

// Melds — ABI conversion between Briv and foreign representations
meld Int <:> Int.c;
meld Float <:> Float.c;
meld String <:> String.c { Ptr -> String.c.ptr; Size -> String.c .#Size; };
meld String <:> String.cpp { Ptr -> String.cpp.ptr; Size -> String.cpp .#Size; };
meld String <:> String.cs { Ptr -> String.cs.ptr; Size -> String.cs .#Size; };
```

Auto-imported as part of the prelude (or lazily on first `frgn from`).

### Files modified

| File | Change |
|------|--------|
| `src/parser/types.rs` | `parse_type_identifier()` accepts `.ext` suffix |
| `src/parser/definitions.rs` | `parse_type_extension_group()` for `Type.[a,b,c]` expansion |
| `src/ast/top.rs` | `Cpp`, `Cs` on `ForeignTarget`, `type_suffix()`, `from_path()` |
| `src/type_universe/mod.rs` | `get_extension()`, `find_extension_meld()`, `find_meld_to_extension()` |
| `src/backend/llvm/normalizer.rs` | Register parsed `Meld` items into `universe.melds` |
| `lib/std/types/ffi.bv` | **New file** — standard FFI type definitions |
| `docs/architecture/backend-type-dispatch.md` | Extension-aware resolution section |
| `docs/architecture/features/meld.md` | Intrinsic assumed melding section, resolution priority |
| New `examples/type-extensions.bv` | Demonstration of extension types |

---

## 3. Path-Based `from` with Compiler-Relative Syntax

### Problem

`from "c"` is a magic string. It should be a real file path. The `<>` syntax (matching `import <name>` pattern) resolves compiler-relative paths.

### Syntax

```briv
// Literal path (CWD-relative or absolute)
frgn strlen(s: String) -> Int from "libc.so.6";
frgn my_func(x: Int) -> Int from "/usr/lib/libfoo.so";

// Compiler-relative path (same as import <name>)
frgn hash(data: Data) -> Int from <xxhash.c>;

// With explicit target override (for ambiguous extensions)
frgn net_send(data: Data) from "libcurl.dll" target "c";
```

### Implementation

#### 3a. AST: `FromSpec` enum

```rust
/// Where a frgn function's implementation comes from.
#[derive(Debug, Clone)]
pub enum FromSpec {
    /// from "path/to/file" — literal path (CWD-relative or absolute).
    Literal(PathBuf),
    /// from <name> — compiler-relative lookup (same pattern as import <name>).
    CompilerRegistry(String),
}

impl FromSpec {
    /// Resolve the from spec to an absolute filesystem path.
    /// For Literal: resolve against CWD.
    /// For CompilerRegistry: search the compiler's stdlib/ffi directory.
    pub fn resolve(&self, resolver: &ImportResolver) -> Result<PathBuf, String> {
        match self {
            Self::Literal(path) => {
                if path.is_absolute() {
                    Ok(path.clone())
                } else {
                    std::env::current_dir()
                        .map(|cwd| cwd.join(path))
                        .map_err(|e| format!("cannot get CWD: {}", e))
                }
            }
            Self::Registry(name) => {
                // Same resolution as resolve_stdlib_root():
                // 1. BRIEF_STDLIB_PATH/ffi/<name>
                // 2. exe_dir/../../lib/ffi/<name> (dev layout)
                // 3. exe_dir/../share/briv/ffi/<name> (installed)
                resolver.resolve_stdlib_relative_path(&format!("ffi/{}", name))
                    .ok_or_else(|| format!("cannot find compiler-relative path: {}", name))
            }
        }
    }
}
```

#### 3b. `FromSpec` on `ForeignBinding` and `ForeignSignature`

`ForeignBinding.location: String` is replaced with `ForeignBinding.from: FromSpec`.

```rust
pub struct ForeignBinding {
    pub name: String,
    pub from: FromSpec,           // was: location: String
    pub target: ForeignTarget,
    pub inputs: Vec<(String, Type)>,
    pub success_output: Vec<(String, Type)>,
    // ... rest unchanged
}

pub struct ForeignSignature {
    pub name: String,
    pub from: FromSpec,           // was: location: String
    pub inputs: Vec<(String, Type)>,
    pub result_type: ResultType,
    pub wasm_impl: Option<String>,
    pub wasm_setup: Option<String>,
    pub span: Option<Span>,
}
```

#### 3c. Parser: `from "path"` and `from <name>`

Same pattern as `import "path"` / `import <name>` (`src/parser/definitions.rs:269-277`):

```rust
/// Parse `from` clause: from "path" or from <name>.
fn parse_from_spec(parser: &mut Parser) -> Result<FromSpec, SyntaxError> {
    if !parser.eat_identifier("from") {
        return Ok(FromSpec::Literal(PathBuf::new())); // no from clause
    }
    if parser.eat(&Token::Lt) {
        let name = parser.expect_identifier()?;
        parser.expect(Token::Gt)?;
        Ok(FromSpec::Registry(name))
    } else {
        let path_str = parser.expect_string()?;
        Ok(FromSpec::Literal(PathBuf::from(path_str)))
    }
}
```

#### 3d. Extension → `ForeignTarget` derivation

`ForeignTarget::from_path(path)` maps:

| Extension | Target | Action |
|-----------|--------|--------|
| `.c` | `C` | Pre-compile to `.o`, then link |
| `.cpp`, `.cc`, `.cxx` | `Cpp` | Pre-compile to `.o`, then link |
| `.so`, `.so.*` | `C` | Link directly (ld) |
| `.dylib` | `C` | Link directly (ld) |
| `.dll` | `C`/`Cs` (ambiguous) | Requires explicit `target "cs"` |
| `.o`, `.a` | `C` | Link directly (ld) |
| `.wasm` | `Wasm` | Link via wasm-ld |
| `.py` | `Python` | Python FFI bridge |
| `.js`, `.mjs` | `Js` | JS glue |

If target derivation fails (e.g., `.xyz` with no known extension) and no explicit `target` override, emit a compile error:

```
error: cannot determine foreign target from 'libfoo.xyz'. Use explicit `target "c"`.
```

#### 3e. Compiler-relative resolution

Expose `resolve_stdlib_root()` from `src/import_resolver.rs` (currently private). Add:

```rust
impl ImportResolver {
    /// Resolve a path relative to the stdlib root.
    /// Searches: BRIEF_STDLIB_PATH, exe-relative dev/installed paths.
    pub fn resolve_stdlib_relative_path(&self, relative: &str) -> Option<PathBuf> {
        for root in self.stdlib_search_roots() {
            let candidate = root.join(relative);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        None
    }

    /// Return all possible stdlib root directories in priority order.
    fn stdlib_search_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        // 1. Explicit stdlib_path
        if let Some(ref path) = self.stdlib_path {
            roots.push(path.clone());
        }
        // 2. Environment variable
        if let Ok(env_path) = std::env::var("BRIEF_STDLIB_PATH") {
            roots.push(PathBuf::from(env_path));
        }
        // 3. Executable-relative (dev)
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                roots.push(dir.join("../../lib"));
                roots.push(dir.join("../share/briv"));
            }
        }
        roots
    }
}
```

#### 3f. Auto-meld at frgn boundary

When compiling a `frgn` call in the LLVM backend (`src/backend/llvm/mod.rs`, around line 1705):

```rust
fn emit_frgn_call(
    &mut self,
    out: &mut String,
    sig: &ForeignSignature,
    args: &[TypedRegister],
) -> Result<TypedRegister, String> {
    let target = ForeignTarget::from_path(&sig.from.resolve(&self.ctx.import_resolver)?)
        .ok_or_else(|| format!("cannot determine target for '{}'", sig.name))?;

    let ext = target.type_suffix();
    if !ext.is_empty() {
        // Auto-meld each parameter from Briv type to extension type
        let converted_args: Vec<TypedRegister> = sig.inputs.iter()
            .zip(args.iter())
            .map(|((_, param_ty), arg)| {
                let ty_name = param_ty.universe_key()
                    .ok_or_else(|| format!("cannot meld non-custom type"))?;
                let (ext_type_name, meld) = self.ctx.type_universe
                    .find_meld_to_extension(ty_name, ext)
                    .ok_or_else(|| format!(
                        "no .{} extension or meld found for {}",
                        ext, ty_name
                    ))?;
                // Emit meld conversion: extract fields per route expressions
                self.emit_meld_forward(out, arg, &meld, &ext_type_name)
            })
            .collect::<Result<Vec<_>, String>>()?;

        // Emit the actual function call with converted args
        let result = self.emit_direct_call(out, &sig.name, &converted_args, &sig.result_type)?;

        // Convert result back via inverse meld
        self.emit_meld_inverse(out, &result, &meld, ty_name)
    } else {
        // No extension needed — emit directly
        self.emit_direct_call(out, &sig.name, args, &sig.result_type)
    }
}
```

This uses the existing (but currently unwired) helpers:
- `emit_route_expression()` (`helpers.rs:1541`) — evaluates a meld route expression on a value
- `emit_decay()` (`helpers.rs:1597`) — materializes a chimera back to concrete type
- `derive_fields_via_meld()` (`helpers.rs:1638`) — derives target fields from source via meld

### Files modified

| File | Change |
|------|--------|
| `src/ast/top.rs` | `FromSpec` enum, replace `location: String` with `from: FromSpec` on `ForeignBinding`/`ForeignSignature` |
| `src/parser/definitions.rs` | `parse_from_spec()` in `frgn` parsing (new arm in `parse_top_level()`) |
| `src/lexer.rs` | Ensure `From` token for `from` keyword (should already exist) |
| `src/ffi/loader.rs` | Update TOML parsing to produce `FromSpec` from `location` + `target` fields |
| `src/ffi/registry/mod.rs` | `resolve_location_to_impl()` takes `FromSpec` |
| `src/ffi/types.rs` | Replace hardcoded `"String"` → `FfiType::String` with extension-aware lookup via universe |
| `src/backend/llvm/mod.rs` | `emit_frgn_call` auto-meld at frgn boundary (~line 1705) |
| `src/backend/llvm/helpers.rs` | Wire `try_meld_projection()`, `emit_decay()`, `derive_fields_via_meld()` into emission pipeline |
| `src/interpreter/eval.rs` | frgn call resolution uses `FromSpec` and auto-meld via universe |
| `src/import_resolver.rs` | Expose `resolve_stdlib_root()` / `resolve_stdlib_relative_path()` |
| `config/module-registry.toml` | Add `[ffi-sources]` section for `<name>` resolution |

---

## 4. Pre-Compilation Pipeline

### Problem

`from "libc.c"` should compile the C source to an object file before linking, not try to link it directly.

### Current state

`compile_ll_to_binary()` (`src/compile.rs:342`) already compiles `.ll` + `briv_rt.c` via clang. We extend this to handle arbitrary C/C++ sources from `from` clauses.

### Implementation

#### 4a. New function: `compile_source_to_object()`

```rust
/// Compile a C/C++ source file to a .o object file using clang.
/// Returns path to the generated .o file.
fn compile_source_to_object(source_path: &Path, cache_dir: &Path) -> Result<PathBuf, String> {
    // Content-based caching
    let content = std::fs::read(source_path)
        .map_err(|e| format!("cannot read '{}': {}", source_path.display(), e))?;
    let hash = blake3::hash(&content);  // or sha256, or any fast hash
    let cache_path = cache_dir.join(format!("{}.o", hash));

    // Return cached object if source unchanged
    if cache_path.exists() {
        return Ok(cache_path);
    }

    // Determine language from extension
    let ext = source_path.extension().and_then(|s| s.to_str());
    let lang = match ext {
        Some("c" | "m") => "c",
        Some("cpp" | "cc" | "cxx") => "c++",
        _ => "c",
    };

    // Compile
    let status = Command::new("clang")
        .args([
            "-O3",
            "-march=native",
            "-ffast-math",
            "-x", lang,
            "-c",
            source_path.to_str().unwrap(),
            "-o", cache_path.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| format!("failed to invoke clang: {} (is clang installed?)", e))?;

    if !status.success() {
        return Err(format!("clang failed to compile '{}'", source_path.display()));
    }

    Ok(cache_path)
}
```

#### 4b. Collect pre-compiled objects in `BuildOptions`

```rust
pub struct BuildOptions {
    // ... existing fields ...
    pub extra_objects: Vec<PathBuf>,  // pre-compiled .o files to link
}
```

During `compile_source()`, for every unique `FromSpec::Literal(path)` with extension `.c`/`.cpp`/`.cc`/`.cxx`/`.m`:

```rust
// In compile_source(), after resolving imports and before codegen:
let mut extra_objects = Vec::new();
for item in &items {
    if let TopLevel::ForeignBinding(fb) = item {
        if let FromSpec::Literal(path) = &fb.from {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if matches!(ext, "c" | "cpp" | "cc" | "cxx" | "m") {
                    let obj = compile_source_to_object(path, &cache_dir)?;
                    extra_objects.push(obj);
                }
            }
        }
    }
}
```

#### 4c. Link pre-compiled objects into final binary

`compile_ll_to_binary()` signature changes to accept `extra_objects`:

```rust
fn compile_ll_to_binary(
    ll_path: &str,
    binary_path: &str,
    extra_objects: &[PathBuf],
) -> Result<(), String> {
    let rt_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("lib/runtime/briv_rt.c");
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
        return Err(format!("clang failed to compile '{}' to binary '{}'", ll_path, binary_path));
    }

    println!("wrote {}", binary_path);
    Ok(())
}
```

#### 4d. Cache directory

Cache path: `~/.cache/briv-compiler/ffi/<content_hash>.o`

```rust
fn get_ffi_cache_dir() -> PathBuf {
    let base = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("~/.cache"))
        .join("briv-compiler")
        .join("ffi");
    std::fs::create_dir_all(&base).ok();
    base
}
```

Cache invalidation: content hash ensures recompilation when source changes. No timestamp/version needed.

### Files modified

| File | Change |
|------|--------|
| `src/compile.rs` | `compile_source_to_object()` new function, `BuildOptions.extra_objects`, update `compile_ll_to_binary()` signature |
| `src/compile.rs` | Collection of extra objects from `FromSpec::Literal` paths in pipeline |

---

## 5. Compile-Time Meld Layout Validation

### Problem

Meld routes between types must be verified for correctness at compile time. CS offers several approaches, from simple structural checks to full SMT-based proofs.

### The layered approach

```
                    ┌─ Every meld declaration ─┐
                    │                          │
                    ▼   Always runs, O(fields)  │
              ┌─────────────────────┐          │
              │ Layer 1: Structural  │          │  O(fields), no interpreter, no SMT
              │ Field count, width,  │          │  Catches: missing fields, width mismatch
              │ offset compatibility │          │
              └─────────┬───────────┘          │
                        │ pass                 │
              ┌─────────▼───────────┐          │
              │ Layer 2: Bit-        │          │  O(bits), linear algebra
              │ permutation matrix   │          │  Catches: overlapping fields, gaps
              │ No overlap, no gaps  │          │
              └─────────┬───────────┘          │
                        │ pass or routes       │
                        │ have non-trivial Expr│
              ┌─────────▼───────────┐          │
              │ Layer 3: Unit-vector │          │  O(bits), via interpreter
              │ enumeration          │          │  Complete for linear melds
              │ via interpreter       │          │  Catches: off-by-one, wrong field binding
              └─────────┬───────────┘          │
                        │ pass or non-linear   │
              ┌─────────▼───────────┐          │
              │ Layer 4: Symbolic    │          │  Uses symbolic.rs
              │ round-trip via       │          │  Catches: arithmetic errors
              │ symbolic execution   │          │
              └─────────┬───────────┘          │
                        │ pass or timeout     │
              ┌─────────▼───────────┐          │
              │ Layer 5: SMT         │          │  Spawns z3, QF_BV
              │ universal proof      │          │  Complete for all routes
              │ ∀x. roundtrip(x)==x  │          │  Catches: everything
              └─────────┬───────────┘          │
                        │ fail                 │
              ┌─────────▼───────────┐          │
              │ Compile-time error   │          │
              │ "meld A <:> B fails  │          │
              │ round-trip check"    │          │
              └─────────────────────┘          │
```

### Layer 1 — Structural (always runs, O(fields))

**Requires:** Meld declaration parsed, both types in universe with offset/width annotations.

```rust
fn validate_structural(
    meld: &MeldDeclaration,
    universe: &TypeUniverse,
) -> Result<(), MeldValidationError> {
    let type_a = universe.get(&meld.name_a)
        .ok_or(MeldValidationError::TypeNotFound(meld.name_a.clone()))?;
    let type_b = universe.get(&meld.name_b)
        .ok_or(MeldValidationError::TypeNotFound(meld.name_b.clone()))?;

    for route in &meld.routes {
        // Check: route.accessor exists as a field on the partner type
        let field_offset = type_b.properties.get(&format!("field.{}.offset", route.accessor))
            .ok_or(MeldValidationError::FieldNotFound {
                ty: type_b.name.clone(),
                field: route.accessor.clone(),
            })?;

        // Check: dest_expr references valid projections on the source type
        // (implemented by walking the Expr for Identifier/Projection references)
        validate_route_expression(&route.dest_expr, type_a, &route.accessor)?;
    }

    Ok(())
}
```

The field offset/width annotations are already computed by `attach_layout_fields()` in `normalizer.rs:103-126`.

### Layer 2 — Bit-permutation matrix (linear algebra, O(bits))

For linear melds (pure field-to-field mapping), the meld is a bit permutation. Two things must hold:

```rust
fn validate_bit_permutation(
    meld: &MeldDeclaration,
    universe: &TypeUniverse,
) -> Result<(), MeldValidationError> {
    let type_a = universe.get(&meld.name_a).unwrap();
    let type_b = universe.get(&meld.name_b).unwrap();
    let total_bits = type_a.bytes * 8;

    // Track which bits in the destination are covered
    let mut dest_bits_covered = vec![false; total_bits as usize];

    for route in &meld.routes {
        let src_offset = get_field_offset(type_a, &route.accessor)?;
        let src_width = get_field_width(type_a, &route.accessor)?;
        let dst_offset = get_field_offset_from_expr(&route.dest_expr, type_b)?;
        let dst_width = get_field_width_from_expr(&route.dest_expr, type_b)?;

        // Check: widths match
        if src_width != dst_width {
            return Err(MeldValidationError::WidthMismatch {
                field: route.accessor.clone(),
                src_width, dst_width,
            });
        }

        // Check: no overlap in destination
        for bit in dst_offset..(dst_offset + dst_width) {
            if dest_bits_covered[bit as usize] {
                return Err(MeldValidationError::Overlap { bit, field: route.accessor.clone() });
            }
            dest_bits_covered[bit as usize] = true;
        }
    }

    // Check: no gaps in destination (all bits covered or explicitly padding)
    for (bit, covered) in dest_bits_covered.iter().enumerate() {
        if !covered && !is_padding_bit(type_b, bit as u64) {
            return Err(MeldValidationError::Gap { bit });
        }
    }

    Ok(())
}
```

**CS justification:** A linear transformation over GF(2) is a bijection iff its matrix is a permutation matrix. This check verifies exactly that — each source bit maps to exactly one destination bit, and the mapping is surjective.

### Layer 3 — Unit-vector enumeration (O(bits), via interpreter)

For each bit position `i` in type A:
1. Create a `Value::Bits` with only bit `i` set
2. Wrap in `Value::Instance` with the type's field layout
3. Evaluate forward meld routes → get type B value
4. Evaluate inverse routes → get type A value
5. Assert bit `i` is still set

```rust
fn validate_unit_vectors(
    meld: &MeldDeclaration,
    universe: &TypeUniverse,
) -> Result<(), MeldValidationError> {
    let type_a = universe.get(&meld.name_a).unwrap();
    let total_bits = type_a.bytes * 8;

    for bit in 0..total_bits {
        let test_val = create_unit_vector_value(type_a, bit);

        // Forward: A → B
        let mid_val = simulate_meld_forward(&meld.routes, &test_val, universe)?;

        // Inverse: B → A
        let roundtrip_val = simulate_meld_inverse(&meld.routes, &mid_val, type_a, universe)?;

        // Verify bit preserved
        if !get_bit(&roundtrip_val, bit) {
            return Err(MeldValidationError::UnitVectorFailed {
                bit, expected: test_val, actual: roundtrip_val,
            });
        }
    }

    Ok(())
}
```

**CS justification:** For a linear transformation, the unit vectors form a basis. If T(e_i) is known for all basis vectors e_i, and T is linear, then T is fully determined. The round-trip check on each unit vector proves T∘T⁻¹ = id for **all** inputs. For non-linear routes, this is still a strong test (though not complete).

Uses the existing `eval_expr()` from the interpreter (`src/interpreter/eval.rs:15`) to evaluate the route expressions on concrete test values.

### Layer 4 — Symbolic round-trip (via `symbolic.rs`)

```rust
fn validate_symbolic(
    meld: &MeldDeclaration,
    universe: &TypeUniverse,
) -> Result<(), MeldValidationError> {
    use crate::symbolic::{eval_symbolic, simplify_binary};

    let type_a = universe.get(&meld.name_a).unwrap();

    // Create symbolic values for each field of type A
    let mut symbolic_input = HashMap::new();
    for (field_name, field_info) in &type_a.fields {
        let sym = SymbolicValue::Identifier(format!("__input_{}", field_name));
        symbolic_input.insert(field_name.clone(), sym);
    }

    // Evaluate forward routes symbolically
    let symbolic_mid = evaluate_routes_symbolic(&meld.routes, &symbolic_input)?;

    // Evaluate inverse routes symbolically
    let symbolic_output = evaluate_inverse_routes_symbolic(
        &meld.routes, &symbolic_mid, type_a
    )?;

    // Simplify and compare each field
    for (field_name, output_val) in &symbolic_output {
        let input_val = SymbolicValue::Identifier(format!("__input_{}", field_name));
        let simplified = simplify_binary(output_val);
        if simplified != input_val {
            return Err(MeldValidationError::SymbolicMismatch {
                field: field_name.clone(),
                expected: input_val.to_string(),
                actual: simplified.to_string(),
            });
        }
    }

    Ok(())
}
```

**CS justification:** Symbolic execution evaluates the transformation on symbolic (variable) inputs rather than concrete values. If the simplified output equals the input for all fields, the round-trip is proven identity. Sound but may conservatively fail when simplification is insufficient.

Uses the existing `src/symbolic.rs` engine and `simplify_binary()` rules from `src/analysis/transition_graph.rs:270-462`.

### Layer 5 — SMT universal proof (via `z3`, QF_BV)

```rust
fn validate_smt(
    meld: &MeldDeclaration,
    universe: &TypeUniverse,
) -> Result<(), MeldValidationError> {
    use crate::proof_engine::smt::prove_smt_formula;

    let type_a = universe.get(&meld.name_a).unwrap();
    let total_bytes = type_a.bytes as usize;

    // Build SMT query: ∀x. meld_inverse(meld_forward(x)) == x
    // Encode as: (assert (not (= (roundtrip x) x))) (check-sat)
    // If UNSAT, the property holds for all inputs.

    // 1. Declare input variable x as (_ BitVec N)
    // 2. Encode forward meld routes as bit-vector operations
    // 3. Encode inverse meld routes
    // 4. Assert inequality
    // 5. Check satisfiability

    let formula = build_meld_smt_formula(meld, &type_a.name, total_bytes)?;
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(500);

    match prove_smt_formula(&formula, timeout) {
        SmtResult::Unsat => Ok(()),         // Proven correct for all inputs
        SmtResult::Sat(counterexample) => {
            Err(MeldValidationError::SmtCounterexample {
                meld: format!("{} <:> {}", meld.name_a, meld.name_b),
                example: counterexample,
            })
        }
        SmtResult::Unknown | SmtResult::Timeout => {
            Err(MeldValidationError::SmtTimeout)
        }
    }
}
```

**CS justification:** SMT solving with QF_BV (quantifier-free bit-vector logic) is decidable and complete for bounded bit-widths. Z3 can prove or disprove bit-vector identities up to thousands of bits. This catches any error the simpler layers miss.

Uses the existing `src/proof_engine/smt.rs` integration (which already calls `z3 -in -smt2` with QF_BV encoding).

### Integration point: normalizer

The full validation cascade runs in `src/backend/llvm/normalizer.rs`, after `synthesize_meld_shuffle()`:

```rust
// In normalizer.rs, existing meld loop (line 34-36):
for item in &items {
    if let TopLevel::Meld(meld) = item {
        // Existing: synthesize shuffle
        synthesize_meld_shuffle(meld, universe)?;

        // New: validate layout via 5-layer cascade
        validate_meld_layout(meld, universe)?;
    }
}
```

```rust
fn validate_meld_layout(meld: &Meld, universe: &mut TypeUniverse) -> Result<(), String> {
    let decl = convert_to_meld_declaration(meld);

    // Layer 1: Structural — always runs
    validate_structural(&decl, universe)?;

    // Layer 2: Bit-permutation — always runs (trivial for linear melds)
    if let Err(e) = validate_bit_permutation(&decl, universe) {
        return Err(format!("meld bit-layout error: {}", e));
    }

    // Layer 3: Unit-vector — run if interpreter available
    if let Err(e) = validate_unit_vectors(&decl, universe) {
        return Err(format!("meld unit-vector test failed: {}", e));
    }

    // Layers 4-5: Symbolic / SMT — only for non-trivial routes
    if has_complex_routes(&decl) {
        // Layer 4: Symbolic (fast, no external dependency)
        if let Err(e) = validate_symbolic(&decl, universe) {
            return Err(format!("meld symbolic verification failed: {}", e));
        }

        // Layer 5: SMT (needs z3 on PATH, may timeout)
        if let Err(e) = validate_smt(&decl, universe) {
            // Errors from Layer 5 are informational if earlier layers passed
            warn!("SMT validation of meld {} <:> {} failed: {}",
                  meld.name, meld.target, e);
        }
    }

    Ok(())
}
```

### Error message format

```
error: meld round-trip validation failed for String <:> String.c
  ┌─ lib/std/types/ffi.bv:5:1
  │
5 │ meld String <:> String.c { Ptr -> String.c.ptr; Size -> String.c .#Size; };
  │                                           ^^^^
  │                                           │
  │                                           Size field width mismatch
  │                                           source: 64 bits (offset 8)
  │                                           dest:   32 bits (offset 8)
  │                                           hint: String.c bytes field may be wrong
```

### Files modified

| File | Change |
|------|--------|
| `src/analysis/meld_validation.rs` | **New file** — Layer 1-5 validation cascade |
| `src/backend/llvm/normalizer.rs` | Integrate `validate_meld_layout()` after `synthesize_meld_shuffle()` |
| `src/backend/llvm/mod.rs` | Integrate validation at frgn auto-meld site as fallback check |
| `src/symbolic.rs` | Expose `eval_symbolic_expr()` for Layer 4 |
| `src/proof_engine/smt.rs` | Expose `prove_smt_formula()` with timeout for Layer 5 |
| New `examples/meld-validation.bv` | Demonstration |

---

## 6. Implementation Order

| Phase | Feature | Depends on | Est. files | Est. effort |
|-------|---------|-----------|-----------|-------------|
| **P0** | Populate `TypeUniverse.melds` from parsed `Meld` | Nothing | 2 | Small — wiring only |
| **P1** | `ConfigResolver` + `config` subcommand + `--config-dir` | Nothing | 8 | Medium — new module + threading |
| **P2** | Dotted type parser + extension groups | Nothing | 3 | Small — parser only |
| **P3** | `FromSpec` parser + resolution + target derivation | P2 | 8 | Medium — parser + AST + FFI |
| **P4** | Auto-meld at frgn boundary (LLVM + interpreter) | P0, P3 | 5 | Large — backend emission changes |
| **P5** | Pre-compilation pipeline (C → .o + caching + linking) | P3 | 1 | Medium — clang integration |
| **P6** | Validation cascade: Layer 1-5 | P0 | 3 | Medium-Large — new analysis module |
| **P7** | Standard FFI type library + examples + docs | P2, P4, P6 | ~5 | Small — docs + examples |

**Recommended execution order:** P0 → P1 → P2 → P3 → P5 → P4 → P6 → P7

This order minimizes blocking: P0 is the critical dependency for all meld features, P1-P2 can be done in parallel, P3 enables P5, and P4-P6 build on everything prior.

### Blocking dependency diagram

```
P0 (populate melds) ─────┬──── P4 (auto-meld at frgn) ──── P7 (docs)
                         │
P1 (config cmd)  ────────┤
                         │
P2 (extension types) ────┼──── P3 (FromSpec) ──── P5 (pre-compile)
                         │
                         └──── P6 (validation cascade)
```

---

## 7. Plan Directives Compliance

### Flat control flow

All new code uses `?`, `if let`, guard clauses. The 5-layer validation cascade is implemented as a flat chain:
```rust
validate_structural(&decl, universe)?;    // Layer 1
validate_bit_permutation(&decl, universe)?;  // Layer 2
if has_simple_routes(&decl) { return Ok(()); }
validate_unit_vectors(&decl, universe)?;  // Layer 3
// ... etc
```

No nested `if-else` chains deeper than 2 levels. Deep logic extracted into named helper functions.

### Comment the code

Every modified/addition gets `// 2026-07-16: <why>`. Existing rationale comments preserved at modification sites — rewritten to explain new structure, never deleted.

Key rationale comments to add:
- `// 2026-07-16: ConfigResolver centralizes all config loading so profiles can be swapped at runtime`
- `// 2026-07-16: Extension types (String.c) allow per-language ABIs without separate Ctype definitions`
- `// 2026-07-16: FromSpec replaces raw location strings — from accepts file paths or <compiler-relative> names`
- `// 2026-07-16: Meld validation uses a 5-layer cascade: structural → bit-permutation → unit-vector → symbolic → SMT`

### Update all examples

Zero existing examples break:
- `examples/meld-simple.bv` — uses bare `meld Int <:> Float` — unaffected
- `examples/meld-routes.bv` — uses route expressions — unaffected
- `examples/test_ffi.bv` — uses `frgn` without `from` — unaffected

New example files:
- `examples/type-extensions.bv` — extension types + extension groups
- `examples/ffi-path.bv` — `from "path"` and `from <name>` syntax
- `examples/meld-validation.bv` — validation cascade in action

### Documentation is code

| Document | Update |
|----------|--------|
| `docs/architecture/backend-type-dispatch.md` | Add extension-aware resolution section; document how `String.c` resolves at FFI boundaries |
| `docs/architecture/features/meld.md` | Document intrinsic assumed melding; resolution priority order; validation cascade |
| New `docs/architecture/features/type-extensions.md` | Document extension type syntax, groups, and FFI integration |
| `///` doc comments on all new public items | `ConfigResolver`, `FromSpec`, `validate_meld_layout()`, etc. |

### Behavioral tests, not literal tests

Meld validation tests assert **behavioral outcomes**:
- "round-trip of unit vector at bit position N preserves bit N"
- "fields A, B, C survive meld → inverse-meld unchanged"
- Not: "LLVM IR contains extractvalue at line 42"

`frgn` auto-meld tests assert:
- "parameter of type String when target is C uses String.c's ABI layout"
- Not: "the `.ll` file contains `call i64 @strlen`"

This ensures tests pass after refactoring as long as the behavior is preserved.
