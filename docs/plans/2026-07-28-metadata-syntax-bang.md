# Metadata Syntax: `key <~ value;` → `!> key: value;`
## 2026-07-28

### Motivation

The `<~` operator was overloaded — it looked like an assignment arrow but behaved
differently from `<-` (arrow operator). The `!>` prefix uses `!` (already associated
with compile-time operations) and introduces a colon delimiter between key and value.

### Changes

| File | Change | Lines |
|------|--------|-------|
| `src/lexer.rs` | Add `ExclaimArrow` token for `!>` | 3 |
| `src/lexer.rs` | Display impl for `ExclaimArrow` | 1 |
| `src/lexer.rs` | Update test for old token | 1 |
| `src/parser/statements.rs` | `parse_metadata_statement`: `TildeArrow` → `ExclaimArrow` + expect Colon | 4 |
| `src/parser/metadata.rs` | `parse_body_metadata`: same | 4 |
| `src/parser/definitions.rs` | catch-all at line 1157: same | 3 |
| `src/ast/display.rs` | Display format: `"{} <~ {:?};"` → `"!> {}: {:?};"` | 1 |
| `lib/std/*.bv` | All `key <~ value;` → `!> key: value;` | ~50 |

### Testing

- `cargo test --lib` — all pass
- `cargo build` — no warnings
- Scan stdlib `.bv` files for remaining `<~` patterns
