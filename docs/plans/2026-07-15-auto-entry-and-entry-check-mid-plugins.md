# Auto-Entry + Entry-Check + CheckReactive$ — Mid-Stage Plugins

**Date:** 2026-07-15
**Status:** Active — implementation in progress
**Branch:** `main`

## Table of Contents

1. [Summary](#1-summary)
2. [Scope](#2-scope)
3. [Documentation Strategy](#3-documentation-strategy)
4. [Phase A: Fix BVIR Contract Round-Trip](#4-phase-a-fix-bvir-contract-round-trip)
5. [Phase B: plugins/mid/auto-main.bv](#5-phase-b-pluginsmidauto-mainbv)
6. [Phase C: plugins/mid/entry-check.bv](#6-phase-c-pluginsminentr-checkbv)
7. [Phase D: CheckReactive$ Intrinsic](#7-phase-d-checkreactive-intrinsic)
8. [Phase E: Plugin Registration](#8-phase-e-plugin-registration)
9. [Phase F: Tests](#9-phase-f-tests)
10. [Phase G: Documentation](#10-phase-g-documentation)
11. [Verification Gates](#11-verification-gates)

---

## 1. Summary

Currently the compiler has no explicit entry-point mechanism. `defn main` is
implicitly treated as entry, but there's no way to inspect or enforce this
in the plugin system. This plan introduces three Mid-stage plugins:

| Plugin | Stage | What it does |
|--------|-------|-------------|
| `auto-main` | Mid | Finds `defn main` or `txn main` and sets `Contract.is_entry = true` |
| `entry-check` | Mid | Verifies at least one entry mechanism exists (`[#]`, `rct txn`, or `trg`) |
| `check-reactive` | Mid | Verifies each `rct txn` has at least one live field binding it consumes |

All three are **enabled by default**.

**Prerequisite:** The BVIR serialize/deserialize round-trip drops
`Contract.is_entry` — this must be fixed first so `MatchIR$` can add
the entry marker.

---

## 2. Scope

**Included:**

- BVIR: serialize `(entry)` when `Contract.is_entry`, deserialize `(entry)` → set `is_entry = true`
- New file: `plugins/mid/auto-main.bv`
- New file: `plugins/mid/entry-check.bv`
- New intrinsic: `CheckReactive$` in `src/plugin/intrinsics.rs`
- Registration: both plugins added to built-in list in `src/plugin/loader.rs`
- Tests: BVIR round-trip, plugin behavior, intrinsic behavior
- Docs: `docs/architecture/features/plugins.md`

**Not included:**
- Removal of any existing entry-point magic (if any) — the plugin just replaces magic with inspectable steps
- Changes to existing `src/backend/llvm/` — `is_entry` is already read during codegen

---

## 3. Documentation Strategy

### 3.1 Rationale comments to add

- `src/bvir/serialize.rs` at contract emission: `// 2026-07-15: (entry) preserves is_entry through BVIR round-trip`
- `src/bvir/deserialize.rs` at contract parsing: `// 2026-07-15: Restore is_entry from (entry) marker`
- `src/plugin/intrinsics.rs` at `CheckReactive$`: `// 2026-07-15: Phase 8 — verifies rct txn has live field bindings`
- `src/plugin/loader.rs` at registration: `// 2026-07-15: auto-main plugin (Mid)`

### 3.2 Architecture docs to update

- `docs/architecture/features/plugins.md`: Add sections for `auto-main`, `entry-check`, `check-reactive` with stage, behavior, and usage notes.

---

## 4. Phase A: Fix BVIR Contract Round-Trip

### 4.1 Serialization (`src/bvir/serialize.rs`)

Current contract serialization:
```
(contract (pre expr) (post expr))
```

New: emit `(entry)` when `is_entry` is true:
```
(contract (entry) (pre expr) (post expr))
```

`(entry)` must come first so the deserializer can set the flag before
parsing pre/post conditions.

### 4.2 Deserialization (`src/bvir/deserialize.rs`)

Add handler for tag `"entry"`: set `is_entry = true`, skip value (no
operand needed — it's a flag).

### 4.3 Display (`src/ast/display.rs`)

The display already handles `is_entry` correctly (renders `[#]`).
No change needed.

### 4.4 Test

Round-trip test: construct a `Contract` with `is_entry = true`, serialize
to BVIR string, parse back, verify `is_entry` is `true`.

---

## 5. Phase B: `plugins/mid/auto-main.bv`

Uses `MatchIR$` to rewrite `defn main` or `txn main`, inserting `(entry)`
into the contract.

```brief
$(Mid) {
    // 2026-07-15: Auto-wire [#] to defn main
    MatchIR$(
        "(defn main ?contract ?params ?ret ?body)",
        "(defn main (contract (entry) (pre true) (post true)) ?params ?ret ?body)"
    );
    // 2026-07-15: Auto-wire [#] to txn main
    MatchIR$(
        "(txn main ?contract ?params [?pre] [?post] ?body)",
        "(txn main (contract (entry) (pre true) (post true)) ?params [?pre] [?post] ?body)"
    );
};
```

**Pattern notes:**
- `?contract` captures the existing contract (which may be absent for `defn`)
- The replacement replaces whatever contract was there with a fresh `(entry)`

---

## 6. Phase C: `plugins/mid/entry-check.bv`

Uses `Collect$` to count entry mechanisms, emits error if zero.

```brief
$(Mid) {
    let has_entry: Int = Collect$("(contract (entry) ??rest)");
    let has_rct: Int = Collect$("(rct txn ?name ?contract ?params [?pre] [?post] ?body)");
    let has_trg: Int = Collect$("(trg ?name ?type @ ?binding)");

    [has_entry == 0 && has_rct == 0 && has_trg == 0] {
        EmitError$(
            "no entry point: add [#] to defn/txn main, "
            "use rct txn, or declare a trg"
        );
    };
};
```

---

## 7. Phase D: `CheckReactive$` Intrinsic

### 7.1 Dispatch registration

Add `"CheckReactive$"` → `intrinsic_check_reactive` in
`dispatch_intrinsic()`.

### 7.2 Handler logic

```rust
fn intrinsic_check_reactive(args: &[Expr], program: &[TopLevel]) -> Result<(), String> {
    // Collect all let-bindings that provide initial values
    let live_fields: HashSet<&str> = program.iter()
        .filter_map(|item| match item {
            TopLevel::Binding(b) if matches!(&b.value, Expr::Decimal(_) | Expr::Float(_) | Expr::Bool(_) | Expr::String(_) | Expr::Quoted(_)) => Some(b.name.as_str()),
            _ => None,
        })
        .collect();

    // Collect all let-bindings with any value (not just decl)
    let all_bindings: HashSet<&str> = program.iter()
        .filter_map(|item| match item {
            TopLevel::Binding(b) => Some(b.name.as_str()),
            _ => None,
        })
        .collect();

    // For each rct txn, check it reads at least one live field
    let mut live_rct_count = 0u32;
    for item in program {
        if let TopLevel::Transaction(txn) = item {
            if txn.is_reactive {
                // Check if any let-binding name appears in the txn body
                let body_text = format!("{:?}", txn.body);
                let reads_live = all_bindings.iter().any(|name| body_text.contains(name));
                if reads_live || txn.contract.is_entry {
                    live_rct_count += 1;
                }
            }
        }
    }

    if live_rct_count == 0 {
        return Err("no reactive transaction has live field bindings or [#] entry".into());
    }
    Ok(())
}
```

**Note:** This is a heuristic (text search of body). A precise
implementation would walk the expression tree. For Phase 8 the
heuristic is sufficient — false positives mean a field name happens
to appear in a string literal, which is rare.

### 7.3 Tests

- `test_check_reactive_rejects_dead`: rct txn with no fields → error
- `test_check_reactive_accepts_live`: rct txn reading a let-binding → ok
- `test_check_reactive_accepts_entry`: rct txn with `is_entry` → ok

---

## 8. Phase E: Plugin Registration

In `src/plugin/loader.rs`, add three entries to the built-in plugin list:

```rust
PluginRegistration {
    name: "auto-main",
    priority: 0,
    stage: PluginStage::Mid,
    enabled_by_default: true,
    source: include_str!("../../plugins/mid/auto-main.bv"),
},
PluginRegistration {
    name: "entry-check",
    priority: 1,
    stage: PluginStage::Mid,
    enabled_by_default: true,
    source: include_str!("../../plugins/mid/entry-check.bv"),
},
PluginRegistration {
    name: "check-reactive",
    priority: 2,
    stage: PluginStage::Mid,
    enabled_by_default: true,
    // Inline Rust intrinsic — no .bv source
    source: "",
},
```

Priority order matters: `auto-main` runs first (adds `[#]` to `main`),
then `entry-check` (sees the newly-added `[#]`), then `check-reactive`
(verifies reactive depth).

---

## 9. Phase F: Tests

| # | Test | What it verifies |
|---|------|-----------------|
| 1 | `test_bvir_contract_entry_roundtrip` | `is_entry` survives serialize/deserialize |
| 2 | `test_auto_main_adds_entry_to_defn` | `defn main` → `is_entry = true` |
| 3 | `test_auto_main_adds_entry_to_txn` | `txn main` → `is_entry = true` |
| 4 | `test_auto_main_skips_if_no_main` | No `main` → no change |
| 5 | `test_entry_check_rejects_empty` | No entry → error |
| 6 | `test_entry_check_accepts_entry` | `[#]` → passes |
| 7 | `test_entry_check_accepts_rct` | `rct txn` → passes |
| 8 | `test_entry_check_accepts_trg` | `trg` → passes |
| 9 | `test_check_reactive_rejects_dead` | rct with no live field → error |
| 10 | `test_check_reactive_accepts_live` | rct with live field → ok |
| 11 | `test_check_reactive_accepts_entry` | rct with `is_entry` → ok |

---

## 10. Verification Gates

Before final commit:

1. `cargo test --lib` — all tests pass (including existing 855+)
2. `cargo build` — no warnings
3. New example files pass `brief check`
4. Run Praetor on new/changed files
5. `docs/architecture/features/plugins.md` updated with all three plugins
