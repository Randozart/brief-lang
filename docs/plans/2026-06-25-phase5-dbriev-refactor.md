# Phase 5 Refactor: Replace TOML with D-Briev v2 Schemas

**Date:** 2026-06-25
**Replaces:** TOML-based `import "target"` implementation

---

## Problem

The current `import "target"` implementation parses a custom TOML format that
duplicates what the D-briev v2 schema system already does. Device schemas
live in `.dbvs` files, board layouts in `.dbvl` files — both use the existing
D-briev v2 parser and bridge. The TOML path is a parallel system that bypasses
this infrastructure.

## Goal

Replace the TOML-based implementation with one that uses D-briev v2 `.dbvl`
files that reference device `.dbvs` schemas. `import "target"` resolves to a
`.dbvl` board file, which imports device schemas and instantiates peripherals.

```
lib/boards/stm32f407.dbvl
  ├── schema lib/devices/uart.dbvs;     // Import schema definitions
  ├── schema lib/devices/gpio.dbvs;
  └── uart1 { base_addr: 0x40011000; size: 0x18; };   // Data with implicit schema
      gpioa { base_addr: 0x40020000; size: 0x400; };
```

## Implementation

### Part A: `schema <path>;` directive in D-briev v2 parser (30 min)

**File:** `src/dbriev/v2.rs`

The v2 parser's main dispatch loop recognizes `'s'/'S'` and calls
`parse_schema()` — which expects `schema Name { fields }`. Add a check: if
the identifier after `schema` contains `/` or `.` (a file path), treat it as
a `schema <path>;` import directive instead:

```rust
's' | 'S' if self.starts_with_ignore_case("schema") => {
    // Try schema <path>; (import directive) first
    if let Some(path) = self.try_parse_schema_import()? {
        doc.imports.push(path);
    } else {
        let schema = self.parse_schema_definition()?;
        doc.schemas.push(schema);
    }
}
```

Where `try_parse_schema_import()`:
1. Consumes `schema` keyword
2. Reads the path identifier (may contain `/`, `.`, `-`, `_`, alphanumeric)
3. Splits on `.` and `/` to produce the import path
4. Expects `;`
5. Returns `Some(path_string)` if it was a file path, `None` if it was a schema name

The parsed `.dbvs` files need to be loaded and their schemas merged into
`doc.schemas`. This happens in the import resolver, not in the parser — the
parser just records the import path, and the resolver loads and bridges.

### Part B: `schema <path>;` as active-schema directive (1 hr)

**File:** `src/dbriev/v2.rs`

Add a `current_schema: Option<String>` field to the `Parser` struct.

When `schema <path>;` is parsed, set `current_schema` to the filename stem
(e.g., `"uart"` for `lib/devices/uart.dbvs`).

When `parse_data_line()` encounters a non-keyed block:
```
uart1 { base_addr: 0x40011000; size: 0x18; };
```
If `current_schema` is set, wrap it as `uart1 as uart { ... }` so the bridge
produces typed `StructInstance` nodes matching the schema.

The `parse_positional_values` and `parse_data_line` functions already handle
the `key { val; val; }` format. The change is:
1. After parsing `key { ... }`, if `current_schema` is `Some(name)`, set
   `entry.schema_name = current_schema.clone()`
2. The bridge already uses `entry.schema_name` to resolve fields by schema

### Part C: Write `.dbvs` device schema files (15 min)

**Location:** `lib/devices/`

```
// lib/devices/uart.dbvs
schema UartPeripheral {
    base_addr: UInt[64] = 0;
    size: UInt[32] = 0;
    registers: List<Register>;
    struct Register {
        name: String;
        offset: UInt[32];
        size: UInt[8];
        access: String;
    }
}
```

Note: nested `struct` definitions within a schema are not currently handled
by the v2 parser's `FieldType`. The v2 parser would need a `Struct` variant
or this must be simplified to flat schemas:

```
schema UartRegs {
    base_addr: UInt[64] = 0;
    size: UInt[32] = 0;
    dr_offset: UInt[32] = 0;
    sr_offset: UInt[32] = 1;
    cr1_offset: UInt[32] = 12;
    cr2_offset: UInt[32] = 16;
}
```

Flat schemas are simpler and align with how the data will be used (individual
offset constants, not nested register traversal at compile time).

### Part D: Write `.dbvl` board files (15 min)

**Location:** `lib/boards/`

```
// lib/boards/stm32f407.dbvl
schema lib/devices/uart.dbvs;
schema lib/devices/gpio.dbvs;
schema lib/devices/timer.dbvs;

uart1 { base_addr: 0x40011000; size: 0x18; };
uart2 { base_addr: 0x40004400; size: 0x18; };
gpioa { base_addr: 0x40020000; size: 0x400; };
gpiob { base_addr: 0x40020400; size: 0x400; };
timer2 { base_addr: 0x40000000; size: 0x400; };
```

### Part E: Rewrite `import "target"` resolver (30 min)

**File:** `src/import_resolver.rs`

Replace `resolve_target_import()`:

```rust
fn resolve_target_import(&mut self) -> Result<Program, String> {
    let board = self.board_name.as_deref().unwrap_or("stm32f407");
    let file_name = format!("{}.dbvl", board);
    // ... search paths for lib/boards/<name>.dbvl ...

    let content = std::fs::read_to_string(&path)?;
    let doc = dbriev_v2::parse_document(&content)?;

    // Resolve schema imports (schema <path>; directives)
    let mut all_schemas = doc.schemas.clone();
    for import_path in &doc.imports {
        // Load and parse the referenced .dbvs file
        let schema_path = resolve_schema_path(import_path)?;
        let schema_content = std::fs::read_to_string(&schema_path)?;
        if let Ok(schema_doc) = dbriev_v2::parse_document(&schema_content) {
            all_schemas.extend(schema_doc.schemas);
        }
    }

    // Merge schemas back and bridge to TopLevel constants
    let merged_doc = DbrievDocument {
        imports: doc.imports.clone(),
        schemas: all_schemas,
        data_groups: doc.data_groups,
        rules: doc.rules,
        key_offsets: doc.key_offsets,
    };

    let items = dbriev::bridge::document_to_program(&merged_doc, &board);
    Ok(Program { items, ... })
}
```

### Part F: Remove TOML files (5 min)

- Delete `lib/boards/stm32f407.toml` (replaced by `.dbvl`)
- Delete the TOML parsing code from `resolve_target_import`

## Summary of Changes

| File | Change |
|---|---|
| `src/dbriev/v2.rs` | Add `schema <path>;` import directive parsing |
| `src/dbriev/v2.rs` | Add `current_schema` field for active-schema data lines |
| `lib/devices/uart.dbvs` | UART peripheral schema (flat fields) |
| `lib/devices/gpio.dbvs` | GPIO peripheral schema |
| `lib/devices/timer.dbvs` | Timer peripheral schema |
| `lib/boards/stm32f407.dbvl` | Board layout with schema imports + data |
| `src/import_resolver.rs` | Replace TOML parsing with D-briev v2 pipeline |
| `lib/boards/stm32f407.toml` | **Deleted** (replaced by .dbvl) |

## Per-commit checklist

- `cargo test --lib` — all tests pass
- `cargo build` — no warnings
- `import "target"` resolves UART1 base_addr to `0x40011000`
- Board `.dbvl` files parse through v2 parser without errors
- No TOML crate dependency changes
- Existing schema validation tests pass
