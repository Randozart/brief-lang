# Data Briev Implementation — New Syntax & Parser Rewrite

**Date:** 2026-07-26
**Status:** Complete (merged 2026-07-26)
**Worktree:** `../briev-compiler-data-briev` (branch: `feat/data-briev`)

---

## 1. Scope

Implement the new Data Briev syntax as specified in
`docs/architecture/data-briev.md`. The changes span three parser layers,
one bridge layer, and one feature module, plus removal of `.dbvs` support.

### What Changes

| Component | File | Change |
|-----------|------|--------|
| **Parser V1** (legacy) | `src/dbriev/parser.rs` | Rewrite: `;` separator, `>` directives, no quotes default |
| **Parser V2** (modern) | `src/dbriev/v2.rs` | Rewrite: same changes, remove `.dbvs` schema support |
| **Bridge** | `src/dbriev/bridge.rs` | Update AST conversion for new parser output |
| **GLUE dbvl reader** | `src/glue/dbvl_reader.rs` | Rewrite: `;` not `,`, `>` not `#` |
| **GLUE dbvs validator** | `src/glue/dbvs_validator.rs` | **Delete** — replaced by `.dbv` schema import |
| **Features dbvl** | `src/features/dbvl.rs` | Update for new line format |
| **Import resolver** | `src/import_resolver.rs` | Remove `.dbvs` extension, update `.dbvl` import |
| **Hardware validator** | `src/hardware_validator.rs` | Remove `.dbvs` dependency |
| **Schema validator** | `src/analysis/schema_validator.rs` | Remove `.dbvs` references |
| **Hardware handoff** | `src/hardware/handoff.rs` | Replace `.dbvs` generation with inline `.dbv` |
| **Wrapper generator** | `src/wrapper/generator.rs` | Update for new `.dbv` syntax |
| **Archive** | `src/archive/mod.rs` | Update `.dbvl` archive writer |
| **FFI archive registry** | `src/ffi/archive/registry/mod.rs` | Update for new parser |
| **LSP** | `src/lsp.rs` | Remove `.dbvs` extension dispatch |
| **Syntax highlighting** | `syntax-highlighter/` | Remove `.dbvs` from file types |

### What Stays

| Component | Reason |
|-----------|--------|
| `src/dbriev/ast.rs` | AST types need only minor field additions (props, `>` flag) |
| `src/dbriev/mod.rs` | Module declaration stays; re-exports updated |
| `src/backend/llvm/bindings.dbvl` | Already uses the new-compatible line format |

---

## 2. Syntax Migration Rules

Every parser must implement these transformations:

### 2.1 Token Changes

| Old | New | Where |
|-----|-----|-------|
| `,` | `;` | Every field/value separator |
| `#` | `>` | Directive prefix (`.dbvl` line start) |
| `@` | `>` | Positional entry marker (`.dbv` block) |
| `"..."` default | Bare tokens default | Quotes opt-in via `--quoted` flag |
| `.dbvs` import | `.dbv` schema import or inline | Every `import "*.dbvs"` → `schema Name from "*.dbv"` |

### 2.2 Schema Import Changes

```
// Old:
import "hardware.dbvs";

// New:
schema Hardware from "hardware.dbv";
```

### 2.3 Data Entry Changes

```
// Old (.dbvl):
rust, glue/rust/types.bv, rs, x86_64-unknown-linux-gnu

// New (.dbvl):
rust; glue/rust/types.bv; rs; x86_64-unknown-linux-gnu
```

```
// Old (.dbv `as` block with positional):
@ 0; rw;
@ 4; ro;

// New (.dbv `as` block with positional):
> 0; rw;
> 4; ro;
```

### 2.4 Directive Changes

```
// Old:
#schema Person from "person.dbv"
#import "addresses.dbvl"

// New:
>schema Person from "person.dbv"
>import "addresses.dbvl"
```

---

## 3. Implementation Order

### Phase 1: Parser V2 (`src/dbriev/v2.rs`) — The Primary Parser

V2 is the modern parser used by both the import resolver and the bridge.
All other consumers read through V2. Fixing V2 first cascades fixes everywhere.

**Changes:**
1. Replace all `,` token checks with `;` in the lexer/parser
2. Replace `#` and `@` token checks with `>`
3. Remove `.dbvs` schema branch — `schema` keyword always expects `.dbv` or inline
4. Remove default quote parsing — bare token becomes default
5. Add `--quoted` flag plumbing (passed through `parse_document_flags`)
6. Update `SchemaDef` to support key field annotation `schema Name (key) { ... }`
7. Add `Props` field to schema definitions
8. Preserve `{ }` for nested blocks and maps
9. Keep `:` for key-value pairs inside `{ }` maps
10. Keep error reporting format (file + line + byte range + expected vs actual)

**Backward compat:** Remove all old comma-support branches.

**Tests:**
- Each syntax change must have a unit test in v2.rs
- Fixtures in a `test_data/` subdirectory
- Run `cargo test --lib -- dbriev::v2` after every sub-phase

### Phase 2: Bridge (`src/dbriev/bridge.rs`)

The bridge converts `DbrievDocument` (V2 output) to Briev AST `TopLevel` items.

**Changes:**
1. Update `import` conversion — emit `schema Name from "path"` instead of old import
2. Ensure key field annotation `(key)` is preserved in bridge output
3. Update error messages for new syntax
4. Remove any `.dbvs` -> `.dbv` path rewriting

**Tests:**
- Round-trip tests: `.dbv` → parse → bridge → Briev AST
- Validate that schema key annotations survive the bridge

### Phase 3: GLUE dbvl Reader (`src/glue/dbvl_reader.rs`)

The GLUE reader is separate from `dbriev::v2` — it has its own comma-splitting
logic that must be updated.

**Changes:**
1. Replace `split_line_by_commas()` with `split_line_by_semicolons()`
2. Update `parse_dbvl()` to use `>` instead of `#` for schema directives
3. Replace comma-based map parsing `{Int:int64_t Float:double}` to
   semicolon-based `{ Int: int64_t; Float: double; }`
4. Update `DbvlEntry` — remove `Raw` variant if no longer needed
5. Add `DbvlEntry` test fixtures

**Tests:**
- `cargo test --lib -- glue::dbvl_reader`
- Parse the existing `bindings.dbvl` and verify output is equivalent

### Phase 4: Remove `.dbvs` Support (4 files)

| File | Action |
|------|--------|
| `src/glue/dbvs_validator.rs` | Delete file, remove `pub mod` from `glue/mod.rs` |
| `src/import_resolver.rs` | Remove `.dbvs` from extension check and import dispatch |
| `src/lsp.rs` | Remove `.dbvs` from file type switch |
| `src/dbriev/mod.rs` | Remove `pub mod dbvs_*` re-exports |

### Phase 5: Update Consumers (7 files)

These files consume the parser output or generate `.dbv`/`.dbvl` content.
They need minimal changes — mostly updating output formatting.

| File | Change |
|------|--------|
| `src/hardware_validator.rs` | Replace `parse_dbvs()` with `parse_document()` from V2 |
| `src/analysis/schema_validator.rs` | Remove `.dbvs` path checks |
| `src/hardware/handoff.rs` | Generate new `.dbv` format instead of `.dbvs` |
| `src/wrapper/generator.rs` | Update `generate_bindings_dbvs()` → `.dbv` output |
| `src/archive/mod.rs` | Update `.dbvl` writer to use `;` separator |
| `src/ffi/archive/registry/mod.rs` | Replace `parse_dbvs()` call with V2 |
| `src/features/dbvl.rs` | Update internal parsing to match new `.dbvl` format |

### Phase 6: Syntax Highlighting

| File | Change |
|------|--------|
| `syntax-highlighter/syntaxes/briev.tmLanguage.json` | Remove `.dbvs` from file types |
| `syntax-highlighter/syntaxes/dbriev.tmLanguage.json` | Update grammar for `>`, `;`, bare tokens |
| `syntax-highlighter/package.json` | Remove `.dbvs` extension entry |

---

## 4. New Parser Behavior Details

### 4.1 Bare Token Lexing (No Quotes Default)

A bare token starts after whitespace and ends at `;`, `}`, `>`, or EOF.

```
alice: Alice Smith; 30;
```
Tokens: `alice`, `Alice Smith`, `30`

The token `Alice Smith` includes the space — the parser does not split on
whitespace inside a token. Whitespace is only significant between tokens.

### 4.2 `>` as Dual-Purpose Symbol

Same byte, two contexts:

- **Line start in `.dbvl`**: `>schema`, `>import`, `>encoding`, `>version`
- **After `{` or `;` in `.dbv` `as` block**: `> Alice Smith;` — positional entry

The parser disambiguates: in `.dbvl` mode, any line starting with `>` is a
directive. In `.dbv` mode, `>` inside a block is a positional entry marker.
`>` appearing as a bare token in a field value (e.g., `a > b`) is fine —
it's only special at line-start or after block-open/entry-terminator.

### 4.3 Semicolons in Data

Since `;` is the universal terminator, data containing a literal `;` must
use the `--quoted` parser flag:

```
// Without --quoted: ERROR — ; terminates the field
alice: Alice Smith; age 30;

// With --quoted:
alice: "Alice Smith; age 30";
```

When `--quoted` is enabled, `"` opens a quoted segment. Inside quotes,
only `"` and `\` are special. Outside quotes, `;` and `}` are terminators
as before.

### 4.4 Key Field Annotation

```
// Schema declares key field
schema Person (name) {
    name: String;
    age: Int;
};

// Keyed entry: key = "alice", name = "Alice Smith", age = 30
as Person {
    alice: Alice Smith; 30;
};
```

The parser stores the key field name in `SchemaDef.key_field: Option<String>`.
The bridge uses it to auto-assign keys in positional entries.

### 4.5 Trailing `;` Optional

The last field in any block may omit `;`:

```
as Person {
    alice: Alice Smith; 30      // no ; after 30
};
```

The parser must accept this at every nesting depth.

---

## 5. Error Messages

Every error must include:
1. File path and line number
2. Offending byte range (start offset, end offset)
3. What was expected vs what was found

New error cases specific to this rewrite:

| Condition | Error |
|-----------|-------|
| Bare token contains `;` without `--quoted` | "Field contains semicolon at byte N. Use --quoted flag or escape." |
| `>` appears mid-line in `.dbv` (not after `{` or `;`) | "Unexpected '>' in field value at byte N. '>' is only valid at start of an entry." |
| `.dbvs` extension used | "'.dbvs' extension is removed. Use '.dbv' with inline schema, or 'schema Name from \"file.dbv\"' to import." |
| Schema not found before `as` block | "Schema 'X' not defined. Declare with 'schema X { ... }' or import with 'schema X from \"path\"'." |
| Field count mismatch | "Entry has N fields but schema 'X' expects M fields." |

---

## 6. File Deletions

| File | Reason |
|------|--------|
| `src/glue/dbvs_validator.rs` | `.dbvs` is removed; validation moves to V2 parser |
| `examples/data-briev/schema.dbvs` | Already deleted in main branch |
| `docs/DATABRIEV.md` | Archived (forwarding note present) |
| `docs/DATABRIEV_GUIDE.md` | Archived (forwarding note present) |

---

## 7. Migration of Existing `.dbvl`/`.dbv` Files

| File | New Format |
|------|------------|
| `src/backend/llvm/bindings.dbvl` | `;` sep, `>` directives |
| `examples/data-briev/adapters.dbvl` | Already updated |
| `examples/data-briev/config.dbv` | Already updated |
| `examples/data-briev/hardware.dbv` | Already updated |
| `glue/rust/types.bv` (comments only) | No change needed |
| `lib/std/ffi/metro_bridge.bv` (comments only) | No change needed |

All `.bv` files referencing `.dbvs` in comments are documentation-only
and do not need code changes.

---

## 8. Benchmark / Performance Targets

The new parser must be at least as fast as the old one:

| Format | Old parse time (target) | New parse time (target) |
|--------|------------------------|------------------------|
| `.dbvl` (1K lines) | < 50µs | < 30µs (simpler tokenizer) |
| `.dbv` (100 entries) | < 100µs | < 80µs |
| `.dbv` with nested blocks | < 200µs | < 150µs |

The speedup comes from:
- No quote-tracking state machine (default path)
- Single-byte token checks (`;` and `}`) instead of multi-character
- No `.dbvs` import resolution overhead

---

## 9. Testing Strategy

### Unit Tests (in each module)

| Module | Test | What it checks |
|--------|------|----------------|
| `dbriev::v2` | `test_bare_token_default` | No quotes → parse succeeds |
| `dbriev::v2` | `test_semicolon_separator` | `;` works everywhere `,` used to |
| `dbriev::v2` | `test_gt_directive` | `>schema` parsed as directive |
| `dbriev::v2` | `test_gt_positional` | `>` inside block = positional entry |
| `dbriev::v2` | `test_key_field_annotation` | `schema X (key) { ... }` |
| `dbriev::v2` | `test_trailing_semicolon_optional` | Last field without `;` accepted |
| `dbriev::v2` | `test_quoted_flag` | `--quoted` enables `"..."` |
| `dbriev::v2` | `test_nested_blocks` | `{ }` with and without keys |
| `dbriev::v2` | `test_map_syntax` | `{ k: v; k2: v2; }` |
| `dbriev::v2` | `test_dbvs_rejected` | `.dbvs` reference → clear error |
| `dbriev::bridge` | `test_roundtrip` | `.dbv` → parse → bridge → validate AST |
| `glue::dbvl_reader` | `test_semicolon_split` | `;` splitting with maps |
| `glue::dbvl_reader` | `test_gt_directive` | `>schema` in GLUE context |

### Integration Tests

```
cargo test --lib
```

Every existing test must pass after migration. **No existing tests may be
weakened or removed** — only updated for new syntax.

### Negative Tests

| Test | What it confirms |
|------|------------------|
| `test_old_comma_rejected` | `,` as field separator → error mentioning `;` |
| `test_old_quote_default_rejected` | Unquoted `;` in data → error mentioning `--quoted` |
| `test_old_hash_rejected` | `#schema` → error mentioning `>schema` |
| `test_old_at_in_dbv` | `@` in block → error mentioning `>` |
| `test_missing_schema` | `as X` without `schema X` → error |

---

## 10. Branch and Commit Strategy

All work on branch `feat/data-briev` in worktree `../briev-compiler-data-briev`.

### Commit Order

1. **Parser V2**: `src/dbriev/v2.rs` — new syntax, remove old
2. **Bridge**: `src/dbriev/bridge.rs` — update AST conversion
3. **GLUE reader**: `src/glue/dbvl_reader.rs` — semicolons and `>`
4. **Remove `.dbvs`**: Delete `dbvs_validator.rs`, update mod.rs, import_resolver, LSP
5. **Update consumers**: hardware, analysis, handoff, wrapper, archive, ffi, features
6. **Syntax highlighting**: Remove `.dbvs`, update grammar
7. **Final pass**: `cargo test --lib`, fix any remaining issues

Each commit must pass `cargo test --lib` and `cargo build`.

---

## 11. Post-Merge Cleanup

After `feat/data-briev` merges to `main`:

1. Remove the old `docs/DATABRIEV.md` and `docs/DATABRIEV_GUIDE.md` files
   (archival period: one release cycle)
2. Update `spec/SPEC.md` section 8 to reference new spec
3. Remove worktree `../briev-compiler-data-briev`
4. Update CI config if it references `.dbvs`

---

## 12. Documentation

### Rationale Comments to Add

Every file modified must add provenance comments at each changed code site:

```
// 2026-07-26: Data Briev syntax migration
// ; replaces , as universal terminator. > replaces # and @.
// Bare tokens are the default; --quoted flag enables ".
// See docs/architecture/data-briev.md for full spec.
```

### Doc Comments to Update

| Module | Update |
|--------|--------|
| `dbriev::v2` | Module-level doc: "New syntax parser for .dbv and .dbvl" |
| `dbriev::bridge` | Module-level doc: "Converts DbrievDocument to Briev AST" |
| `glue::dbvl_reader` | Module-level doc: "Line-based .dbvl reader using ; sep" |

No doc comments should reference the old comma/quote syntax.

---

## 13. Risk Assessment

| Risk | Probability | Mitigation |
|------|-------------|------------|
| Existing `.dbvl` files in `bindings.dbvl` break | Low | Its format is already compatible with `;`-sep |
| `.dbvs` removal breaks third-party Briev projects | Medium | Detection in V2 parser produces clear error: "'.dbvs' is removed. Use .dbv." |
| `--quoted` flag overlooked by users migrating old data | Medium | Error message for `;` inside field mentions `--quoted` |
| Performance regression from unbounded bare tokens | Low | Bare token lexing is simpler than quote-tracking; should be faster |
| GLUE pipeline breaks from `.dbvl` format change | Low | Test fixtures in `glue::tests` cover the reader |
