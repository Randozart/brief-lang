# TOML → DBV Conversion Guide

**How to translate common TOML patterns into `.dbv` and `.dbvl`.**

---

## 1. Simple Key-Value

```toml
# TOML
name = "briev-compiler"
version = "1.0.0"
debug = false
```

```dbv
// .dbv — schema + single entry
schema Config {
    name: String;
    version: String;
    debug: Bool;
};

as Config {
    > briev-compiler; 1.0.0; false;
};
```

```dbvl
// .dbvl — flat line, no schema
briev-compiler; 1.0.0; false;
```

---

## 2. Tables (Sections)

```toml
# TOML
[".bv"]
backend = "llvm"
defaults = ["--budget", "256"]
plugins = ["prelude", "env", "print"]
```

```dbv
// .dbv — table becomes an entry, array becomes nested block
schema TargetConfig {
    extension: String;
    backend: String;
    defaults: String;
    plugins: String[];
};

as TargetConfig {
    > .bv; llvm; --budget 256; { prelude; env; print; };
    > .ebv; llvm; --optimize-size --budget 0; { prelude; };
};
```

The table heading `[".bv"]` becomes key/positional value `.bv`.
Array `["prelude", "env", "print"]` becomes nested block `{ prelude; env; print; }`.

---

## 3. Nested Tables

```toml
# TOML
[compiler]
optimization = 3

[compiler.backend]
target = "wasm"
```

```dbv
schema CompilerConfig {
    optimization: Int;
    backend_target: String;
};

as CompilerConfig {
    > 3; wasm;
};
```

TOML's dotted-key path syntax collapses to flat field names in the schema.
Alternative: nested schemas for each hierarchy level.

---

## 4. Arrays of Tables

```toml
# TOML
[[bindings]]
name = "json"
version = "2.0"

[[bindings]]
name = "http"
version = "1.5"
```

```dbv
schema Binding (name) {
    name: String;
    version: String;
};

as Binding {
    > json; 2.0;
    > http; 1.5;
};
```

Each TOML `[[array]]` section becomes one positional entry in the `as` block.

---

## 5. Inline Tables

```toml
# TOML
adapter = { language = "rust", types = "glue/rust/types.bv" }
```

```dbv
schema AdapterEntry {
    language: String;
    types: String;
};

as AdapterEntry {
    > rust; glue/rust/types.bv;
};
```

Or as a map-valued field:

```dbv
schema AdapterEntry {
    adapter: Map;
};

as AdapterEntry {
    > { language: rust; types: glue/rust/types.bv; };
};
```

---

## 6. Arrays of Inline Tables

```toml
# TOML
routes = [
    { path = "/api", handler = "api_handler" },
    { path = "/health", handler = "health_check" },
]
```

```dbv
schema Route {
    path: String;
    handler: String;
};

as Route {
    > /api; api_handler;
    > /health; health_check;
};
```

---

## 7. Multi-line Strings

```toml
# TOML
description = """
A long description
that spans multiple
lines.
"""
```

```dbv
// Bare tokens span lines by default:
schema Doc {
    description: String;
};

as Doc {
    > A long description that spans multiple lines.;
};
```

Or reference an external file:

```dbv
schema Doc {
    description_file: String;
};

as Doc {
    > docs/description.txt;
};
```

---

## 8. Boolean and Numeric Values

```toml
# TOML
debug = true
count = 42
ratio = 3.14
```

```dbv
schema Vals {
    debug: Bool;
    count: Int;
    ratio: Float;
};

as Vals {
    > true; 42; 3.14;
};
```

Booleans: `true` and `false` (bare keywords, no quotes).
Numbers: bare digits, optional `-` and `.`. Hex like `0x4000` is a string.

---

## 9. Key-Value Maps

```toml
# TOML
[type_map]
Int = "int64_t"
Float = "double"
Bool = "bool"
```

```dbv
// As a Map field:
schema RegistryEntry {
    type_map: Map;
};

as RegistryEntry {
    > { Int: int64_t; Float: double; Bool: bool; };
};
```

The `:` inside `{ }` creates key-value pairs — not fields of a schema.

---

## 10. Putting It All Together

This TOML:

```toml
# config/targets.toml
[".bv"]
backend = "llvm"
defaults = ["--budget", "256"]
plugins = ["prelude", "env", "print"]

[".ebv"]
backend = "llvm"
defaults = ["--optimize-size", "--budget", "0"]
plugins = ["prelude"]
```

Becomes this `.dbvl`:

```
>schema TargetConfig from "config/schemas/target.dbv"
.bv; llvm; --budget 256; { prelude; env; print; };
.ebv; llvm; --optimize-size --budget 0; { prelude; };
```

With this schema in `config/schemas/target.dbv`:

```dbv
schema TargetConfig {
    extension: String;
    backend: String;
    defaults: String;
    plugins: String[];
};
```

The `.dbvl` file is:
- **Shorter** than the TOML (1 line per entry vs 4 lines per `[section]`)
- **Faster to parse** (single-pass byte scanner, no key-name allocation)
- **Schema-validated** at the parse boundary
- **Human-readable** in any terminal
