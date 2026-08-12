# Extending the `$` Macro System — Generic Compile-Time Intrinsics

**Date:** 2026-07-22
**Status:** Plan (ready for implementation)

---

## Philosophy

Every `$` intrinsic must be **fully generic** — useful to ANY plugin, not just
GLUE. The bridge generator is just ONE consumer of these primitives, written
as a `.bv` plugin file.

The test: "Could a non-GLUE plugin (linter, formatter, doc generator,
scaffolding tool) use this intrinsic?" If no, the design needs rethinking.

---

## Current State

The `$` system at `src/macros/eval.rs` handles two domains well:

| Domain | Commands | Status |
|--------|----------|--------|
| **AST selection** | `Tag$`, `Named$`, `WithKey$`, `WithAttr$`, `All$` | ✅ Working |
| **AST traversal** | `First$`, `Last$`, `Nth$`, `Children$`, `Descendants$`, `Parent$` | ✅ Working |
| **AST introspection** | `Count$`, `Names$`, `IsEmpty$` | ✅ Working |
| **AST positions** | `Before$`, `After$`, `Replace$`, `Inside$`, `AppendTo$` | ✅ Working |
| **AST actions** | `Insert$`, `Delete$`, `ReplaceWith$`, `Set$`, `Rename$` | ✅ Working |
| **AST constructors** | `Import$`, `Defn$`, `Call$`, `Block$` | ✅ Working |
| **Control flow** | `when`, `let`, `foreach`, block, `EmitInfo/Warning/Error$` | ✅ Working |
| **String operations** | None | ❌ Missing |
| **File I/O** | None | ❌ Missing |
| **Configuration** | None | ❌ Missing |
| **External processes** | None | ❌ Missing |
| **Universe queries** | None | ❌ Missing |
| **Template rendering** | None | ❌ Missing |
| **Type information** | None | ❌ Missing |

---

## Proposed Intrinsics

Each section lists: the intrinsic, justification of generic usefulness,
use cases beyond GLUE, and implementation notes.

---

### 1. `StrLen$`, `StrSubstr$`, `StrReplace$`, `StrJoin$`, `StrSplit$`

**Generic justification:** String manipulation is the most fundamental
capability missing from the `$` system. Every plugin that generates output
(file names, error messages, code snippets) needs basic string operations.

**Examples of generic use:**
- **Linter**: `StrSplit$(line, ",")` to parse CSV-based metadata
- **Doc generator**: `StrReplace$(template, "{{TITLE}}", title)` for page titles
- **Formatter**: `StrJoin$(lines, "\n")` to reassemble formatted output
- **Scaffolder**: `StrSubstr$(path, 0, -3)` to strip extension from filenames
- **GLUE bridge**: `StrReplace$(fn_template, "{{name}}", fn_name)` for wrapper code

**Implementation:**
```rust
"StrLen$" => {
    let s = expect_str_arg(args, 0, "StrLen$")?;
    Ok(NavValue::Int(s.len() as i64))
}
"StrReplace$" => {
    let s = expect_str_arg(args, 0, "StrReplace$")?;
    let from = expect_str_arg(args, 1, "StrReplace$")?;
    let to = expect_str_arg(args, 2, "StrReplace$")?;
    Ok(NavValue::Str(s.replace(&from, &to)))
}
"StrJoin$" => {
    let list = expect_nav_list(args, 0, "StrJoin$")?;
    let sep = expect_str_arg(args, 1, "StrJoin$")?;
    Ok(NavValue::Str(list.join(&sep)))
}
"StrSplit$" => {
    let s = expect_str_arg(args, 0, "StrSplit$")?;
    let pat = expect_str_arg(args, 1, "StrSplit$")?;
    let parts = s.split(&pat).map(|p| NavValue::Str(p.to_string())).collect();
    Ok(NavValue::List(parts))
}
```

**New `NavValue` variant needed:** `NavValue::List(Vec<NavValue>)` —
enables generic list manipulation (map, filter, join, etc.)

---

### 2. `FileRead$`, `FileWrite$`

**Generic justification:** A compile-time macro system that can't read or write
files is confined to AST transformations. File I/O opens the door to code
generation, scaffolding, template output, and build artifact production.

**Examples of generic use:**
- **Scaffolder**: `FileWrite$("src/main.bv", template_source)` generates a new project
- **Doc generator**: `FileWrite$("docs/api.md", markdown)` outputs documentation
- **Linter**: `FileRead$("config.toml")` loads custom lint rule configuration
- **Formatter**: `FileWrite$("formatted.bv", formatted_code)` writes formatted output
- **GLUE bridge**: `FileWrite$("src/lib.rs", rendered_wrapper)` outputs generated crate

**Implementation:**
```rust
"FileRead$" => {
    let path = expect_str_arg(args, 0, "FileRead$")?;
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("FileRead$: cannot read '{}': {}", path, e))?;
    Ok(NavValue::Str(content))
}
"FileWrite$" => {
    let path = expect_str_arg(args, 0, "FileWrite$")?;
    let content = expect_str_arg(args, 1, "FileWrite$")?;
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("FileWrite$: cannot create dir '{}': {}", parent.display(), e))?;
    }
    std::fs::write(&path, &content)
        .map_err(|e| format!("FileWrite$: cannot write '{}': {}", path, e))?;
    Ok(NavValue::Void)
}
```

---

### 3. `ConfigGet$`

**Generic justification:** Any plugin that needs configuration beyond hardcoded
values should read from TOML. Currently the prelude hardcodes import paths —
a linter, formatter, or test runner would read rules from config.

**Examples of generic use:**
- **Prelude**: `ConfigGet$("prelude", "auto_import")` reads auto-import list from TOML
- **Linter**: `ConfigGet$("linter", "allowed_break_patterns")` reads lint rules
- **Formatter**: `ConfigGet$("fmt", "max_line_length")` reads formatting config
- **Proof oracle**: `ConfigGet$("prover", "unroll_limit")` reads proof limits
- **GLUE bridge**: `let tmpl = ConfigGet$("rust.templates", "fn_template")` reads templates

**Implementation:**
```rust
"ConfigGet$" => {
    let section = expect_str_arg(args, 0, "ConfigGet$")?;
    let key = expect_str_arg(args, 1, "ConfigGet$")?;
    let targets = crate::glue::config::load_glue_config(None)
        .map_err(|e| format!("ConfigGet$: {}", e))?;
    let target = targets.get(&section)
        .ok_or_else(|| format!("ConfigGet$: no section '{}' in lib/glue.toml", section))?;
    // Resolve dotted key (e.g., "templates.fn_template" → target.templates["fn_template"])
    if let Some(dot) = key.find('.') {
        let (sub, field) = key.split_at(dot);
        let field = &field[1..];
        match sub {
            "templates" => {
                let val = target.templates.get(field)
                    .ok_or_else(|| format!("ConfigGet$: no template '{}'", field))?;
                Ok(NavValue::Str(val.clone()))
            }
            "protocols" => {
                let proto = target.protocols.get(&format!("#{}", field));
                // Returns native:c_abi pair as a string for now
                Ok(NavValue::Str(format!("{:?}", proto)))
            }
            _ => Err(format!("ConfigGet$: unknown sub-section '{}'", sub)),
        }
    } else {
        Err("ConfigGet$: need dotted key like 'templates.fn_template'".into())
    }
}
```

**Alternative:** Make `ConfigGet$` return `NavValue::Map` for structured config,
and let subsequent `StrReplace$` calls access fields via dot notation:

```briev
let cfg = ConfigGet$("rust");
// cfg is NavValue::Map with keys "templates", "protocols", etc.
let tmpl = cfg["templates"]["fn_template"];
```

This requires `NavValue::Map(HashMap<String, NavValue>)` — a new variant.

---

### 4. `DocRead$` — Universe Documentation Queries

**Generic justification:** The `$` system can query AST structure via `Tag$`/`Named$`
but has no access to the TypeUniverse's type metadata. Type documentation is
the most commonly requested feature for IDE plugins and doc generators.

**Examples of generic use:**
- **Doc generator**: `DocRead$("String", "properties")` returns all properties
- **LSP plugin**: `DocRead$("my_type", "llvm_type")` returns LLVM type info
- **Linter**: `DocRead$("String", "proto")` returns protocol categories
- **Proof oracle**: `DocRead$("my_fn", "contract")` returns pre/post conditions
- **GLUE bridge**: `let cat = DocRead$(param.Type, "Cast.#...")` finds protocol participation

**Implementation:**
```rust
"DocRead$" => {
    let type_name = expect_str_arg(args, 0, "DocRead$")?;
    let prop = expect_str_arg(args, 1, "DocRead$")?;
    let rt = universe.get(&type_name)
        .ok_or_else(|| format!("DocRead$: type '{}' not in universe", type_name))?;
    match prop {
        "properties" => {
            let props: Vec<String> = rt.properties.keys().cloned().collect();
            Ok(NavValue::Names(props))
        }
        "bytes" => Ok(NavValue::Int(rt.bytes as i64)),
        "llvm_type" => Ok(NavValue::Str(rt.llvm_type.clone().unwrap_or_default())),
        _ => {
            let val = rt.properties.get(&prop);
            match val {
                Some(v) => Ok(NavValue::Str(v.clone())),
                None => Err(format!("DocRead$: no property '{}' on type '{}'", prop, type_name)),
            }
        }
    }
}
```

---

### 5. `TypeInfo$` — Type Information from AST Nodes

**Generic justification:** The AST navigation DSL can select `Named$("fn_name")`
but can't inspect the types of parameters, return values, or fields. A bridge
generator needs to emit code that depends on each parameter's type.

**Examples of generic use:**
- **Bridge generator**: `TypeInfo$(param, "name")` → param name, `TypeInfo$(param, "type")` → type
- **Doc generator**: `TypeInfo$(defn, "params")` → list of param types
- **Linter**: `TypeInfo$(defn, "output_type")` → check return type rules
- **Formatter**: `TypeInfo$(field, "type")` → include type annotations in output

**Implementation:**

This one is trickier because it requires extending `NavValue` and the item
selection system to expose AST node properties. A simpler first step:
`NavValue::TopLevel` already exists — extend it to support field access:

```briev
let defn = Named$("my_fn").First$();
let param_count = TypeInfo$(defn, "params.count");  // number of params
let first_param_name = TypeInfo$(defn, "params.0.name");  // first param name
let first_param_type = TypeInfo$(defn, "params.0.type");  // first param type
```

**Rust implementation** would match on the `TopLevel` variant and extract
the requested field:

```rust
fn type_info_from_toplevel(tl: &TopLevel, field: &str) -> Result<String, String> {
    match (tl, field) {
        (TopLevel::Definition(d), "name") => Ok(d.name.clone()),
        (TopLevel::Definition(d), "params.count") => Ok(d.parameters.len().to_string()),
        (TopLevel::Definition(d), f) if f.starts_with("params.") => {
            // params.0.name or params.0.type
            let rest = &f[7..]; // strip "params."
            let parts: Vec<&str> = rest.split('.').collect();
            let idx: usize = parts[0].parse().map_err(|_| "invalid param index")?;
            let param = d.parameters.get(idx).ok_or("param index out of bounds")?;
            match parts.get(1) {
                Some(&"name") => Ok(param.0.clone()),
                Some(&"type") => Ok(format!("{}", param.1)),
                _ => Err("unknown param field".into()),
            }
        }
        _ => Err(format!("TypeInfo$: unknown field '{}' for this item", field)),
    }
}
```

---

### 6. `CastPath$` — Protocol BFS for Type Conversion

**Generic justification:** Protocol path computation is a generic type-theoretic
operation with uses beyond bridge generation.

**Examples of generic use:**
- **Linter**: `CastPath$("String", "#Bits")` → verify any type can be bitcast
- **Proof oracle**: `CastPath$("ASCIIString", "#String")` → verify protocol participation
- **Optimizer**: `CastPath$("structA", "structB")` → check if layouts are compatible
- **Doc generator**: `CastPath$("Float", "#String")` → document available conversions
- **GLUE bridge**: `CastPath$(param_type, "#String")` → compute protocol path at boundary

**Implementation:**
```rust
"CastPath$" => {
    let src = expect_str_arg(args, 0, "CastPath$")?;
    let tgt = expect_str_arg(args, 1, "CastPath$")?;
    let path = crate::analysis::layout_optimizer::find_cast_path(universe, &src, &tgt);
    match path {
        Some(types) => {
            let steps: Vec<NavValue> = types.into_iter()
                .map(|t| NavValue::Str(t)).collect();
            Ok(NavValue::List(steps))
        }
        None => Ok(NavValue::List(vec![])),
    }
}
```

---

### 7. `ShellCmd$` — External Command Execution

**Generic justification:** Compile-time access to external tools is essential
for code generation pipelines that need compilation, formatting, or validation
by external tools.

**Examples of generic use:**
- **Bridge generator**: `ShellCmd$("llc", "-filetype=obj", "bridge.ll")` — compile LLVM IR
- **Formatter**: `ShellCmd$("briev", "check", "--format", "file.bv")` — run sub-compiler
- **Linter**: `ShellCmd$("clang-tidy", "file.c")` — run external linter
- **Test runner**: `ShellCmd$("cargo", "test")` — run tests from within Briev
- **Doc generator**: `ShellCmd$("pandoc", "doc.md", "-o", "doc.pdf")` — convert docs

**Implementation:**
```rust
"ShellCmd$" => {
    let cmd = expect_str_arg(args, 0, "ShellCmd$")?;
    let cmd_args: Vec<String> = args[1..].iter()
        .map(|a| expect_str_arg_inner(a))
        .collect::<Result<Vec<_>, _>>()?;
    let output = std::process::Command::new(&cmd)
        .args(&cmd_args)
        .output()
        .map_err(|e| format!("ShellCmd$: failed to execute '{}': {}", cmd, e))?;
    if !output.status.success() {
        let stderr = String::from_UTF8_lossy(&output.stderr);
        return Err(format!("ShellCmd$: '{}' failed: {}", cmd, stderr));
    }
    Ok(NavValue::Str(String::from_UTF8_lossy(&output.stdout).to_string()))
}
```

---

## New `NavValue` Variants

| Variant | Purpose | Used by |
|---------|---------|---------|
| `NavValue::List(Vec<NavValue>)` | Generic lists (string lists, selection lists) | `StrJoin$`, `StrSplit$`, `CastPath$` |
| `NavValue::Map(HashMap<String, NavValue>)` | Structured config data | `ConfigGet$` with deep access |
| `NavValue::Error(String)` | Propagate errors from sub-operations | All intrinsics |

---

## Implementation Order

| Step | Intrinsics | Lines of Rust | Dependencies |
|------|------------|---------------|-------------|
| 1 | `StrLen$`, `StrReplace$`, `StrJoin$`, `StrSplit$` | ~60 | `NavValue::List` |
| 2 | `FileRead$`, `FileWrite$` | ~40 | None |
| 3 | `ConfigGet$` | ~60 | `src/glue/config.rs` (exists) |
| 4 | `DocRead$` | ~40 | `TypeUniverse` (exists) |
| 5 | `TypeInfo$` | ~50 | `TopLevel` AST (exists) |
| 6 | `CastPath$` | ~30 | `find_cast_path()` (exists) |
| 7 | `ShellCmd$` | ~30 | None |

Total: ~310 lines added to `src/macros/eval.rs`, one new file per doc update.

---

## Documentation Updates

### `docs/architecture/macro-system.md` (NEW)

Should document:
- The contract: every `$` intrinsic is fully generic, never GLUE-specific
- The testing ground: a generic intrinsic must have 3+ non-GLUE use cases
- The rule: if a proposed intrinsic is only useful for bridge generation,
  it belongs in the bridge generator `.bv` file, not in the Rust engine itself
- The catalog: all available `$` intrinsics with signatures and examples

### `learn-briev/16-plugins.md`

Should be extended with:
- How plugins use `$` intrinsics for file I/O, config, and external commands
- How to write a simple code generator plugin
- How the bridge generator plugin works (as an example of combining multiple
  `$` intrinsics)

### `learn-briev/07-ffi.md`

Add a briev section about the `$` intrinsics used in the GLUE pipeline.
Currently it only covers `frgn` and `export defn` — should mention that
the bridge generator itself is a `$(Glue @ highest)` plugin using `ConfigGet$`,
`StrReplace$`, `FileWrite$`, etc.

---

## Verification

```bash
# All tests pass
cargo test --lib

# The prelude still works (exercise the full pipeline)
briev build counter.bv --llvm

# GLUE bridge still works
briev export pp-types.bv rust --out /tmp/test
cd /tmp/test && cargo build
```

---

## Appendix: Current String Operations via Plugin Engine

The `$` system currently has ZERO string operations. There is no way to:
- Get the length of a string
- Extract a substring
- Replace substrings
- Join or split strings
- Compare strings
- Concatenate strings at compile time

This is the single biggest gap. Every plugin that generates output needs
at minimum `StrReplace$` for template substitution. Six of the seven proposed
intrinsics are string-based or file-based — only `DocRead$` and `CastPath$`
are pure type operations.

The GLUE bridge generator specifically needs:
- `StrReplace$` — substitute `{{name}}`, `{{params}}` in templates (step 1)
- `ConfigGet$` — read protocol mappings and templates from `lib/glue.toml` (step 3)
- `TypeInfo$` — get parameter names and types from exported definitions (step 5)
- `CastPath$` — compute protocol path for each parameter type (step 6)
- `FileWrite$` — output the generated crate/module files (step 2)
- `ShellCmd$` — run `llc` and `cc` to compile `.ll` → `.o` → `.so` (step 7)
