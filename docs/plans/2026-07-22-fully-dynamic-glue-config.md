# Fully Dynamic GLUE Config — Remove Hardcoded Language References

**Date:** 2026-07-22
**Status:** Implementation
**Applies to:** `src/glue/config.rs`, `src/analysis/frgn_dispatch.rs`

---

## Goal

Make the GLUE config system fully language-agnostic. Currently three Rust
structs/knowledge sites hardcode specific language identifiers ("python",
"node", "rust"). After this change, adding a new language requires ONLY
a `[language]` section in `lib/glue.toml` — zero Rust changes.

## Hardcoded Sites

### Site 1: `GlueConfigFile` struct (config.rs:47-55)

```rust
struct GlueConfigFile {
    python: Option<LanguageEntry>,
    node: Option<LanguageEntry>,
    rust: Option<LanguageEntry>,
}
```

**Fix:** Use `#[serde(flatten)]` to collect all top-level keys dynamically.

### Site 2: `load_glue_config()` three blocks (config.rs:114-154)

Three `if let Some(lang) = parsed.{python,node,rust} { ... }` blocks.

**Fix:** Replace with a single `for (name, entry)` loop over the flattened map.

### Site 3: `extension_to_language()` in both files (config.rs:189, frgn_dispatch.rs:192)

Hardcoded `match (ext, backend) { "py" => Some("python"), ... }`. **Dead code**
— zero production call sites. Only referenced in tests.

**Fix:** Remove from both files.

## Execution

### Step 1 — Fix `GlueConfigFile` + `load_gu_config` (config.rs)

Replace the three-field struct with `#[serde(flatten)] HashMap<String, LanguageEntry>`.
Replace the three `if let` blocks with a single `for` loop.
Remove `extension_to_language` function.

### Step 2 — Remove `extension_to_language` from `frgn_dispatch.rs`

Remove function definition and its tests.

### Step 3 — Verify

```bash
cargo build              # 0 errors
cargo test --lib         # 973 pass
cargo test --test pp_roundtrip_tests  # 8 pass
```

## Verification

After the change:
- `briv export pp-types.bv rust --out /tmp/test` should still work
- `briv export pp-types.bv python --out /tmp/test` should also work
- Adding a new language = adding TOML section, nothing else

## Risk Assessment

| Factor | Rating | Rationale |
|--------|--------|-----------|
| Removed lines | ~110 | Mostly dead code (`extension_to_language`) |
| Behavioral change | None | Same logic, dynamic collection |
| Test impact | ~8 tests removed | All tested dead code |
| New language cost | Zero Rust | Just TOML |
