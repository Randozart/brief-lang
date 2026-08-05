# LTO Closure + MMIO Address Plumbing + Hardware Handoff Pipeline

**Timestamp**: 2026-06-03 13:35
**Status**: Done (Phases 1-4 complete)

## Context

Two orthogonal gaps in the LLVM codegen pipeline that converge on a unified hardware-target system:

1. **LTO gap**: `briv_rt.c` is compiled as machine code (`cc -c -O2`). The Briv IR goes through `opt -O3 → llc`. They're linked as ELF objects. Zero cross-module inlining — `__print_int`, `__wait_for_event`, thread pool primitives are forever opaque calls.

2. **MMIO gap**: `let x: Int @ 0x1000 = 0;` is parsed correctly into `StateDecl.address = Some(0x1000)` but the LLVM backend drops it in `build_field_index()` (`src/backend/llvm.rs:1704`). Every `@`-addressed field ends up as a plain `%State` struct member via GEP. No `inttoptr`, no `volatile`, no MMIO. The Verilog, VHDL, AArch64, x86_64, and C backends all use `decl.address` — LLVM is the only outlier.

3. **Hardware handoff gap**: No path from Vivado's `system.xsa`/`xparameters.h` to compiler-visible addresses. The DBVS alias system exists (`src/dbriv/`) but never flows into LLVM codegen.

## Architecture: DBVS Schema + Target Binding

```
Vivado .xsa/hwh
      │
      ▼
hw-handoff generator ──── produces:
      │
      ├── chip.dbvs        (schema: register names + types, no addresses)
      └── zcu4ev.dbv       (bindings: alias → physical address for this target)
            ultra96.dbv    (different target, same schema)
            sim.dbv        (no-address: aliases are struct members = current behavior)

.bv/.ebv program:
      │
      import "chip.dbvs"    → schema loaded, names available for contracts
      │
      compile --target zcu4ev.dbv
      │
      ▼
Compiler resolves each schema alias → physical address
  → Proof engine validates on logical names (unchanged)
  → LLVM backend emits MMIO load/store volatile via inttoptr
  → Same program compiles for sim.dbv with GEP %State (no MMIO)
```

## Phase 1: LTO Closure (LOW RISK ~30 lines)

### Problem
`briv_rt.c` is compiled as machine code (`cc -c -O2`), then linked as ELF with the Briv-generated `.o`. The `opt -O3` pass sees only the Briv IR — `__print_int`, `__wait_for_event`, and thread pool barriers are opaque `call @symbol` instructions with zero inlining opportunity.

### Fix
Replace `cc -c` with `clang -c -emit-llvm`, merge with `llvm-link` before `opt`:

```
Before:  cc -c -O2 briv_rt.c → briv_rt.o
         opt -O3 program.ll → program.opt.ll
         llc -filetype=obj program.opt.ll → program.o
         cc -O2 program.o briv_rt.o → a.out

After:   clang -c -emit-llvm -O2 briv_rt.c → briv_rt.bc
         llvm-link program.bc briv_rt.bc → merged.bc
         opt -O3 merged.bc → merged.opt.bc
         llc -filetype=obj merged.opt.bc → merged.o
         cc -O2 merged.o → a.out
```

`opt -O3` on the merged module inlines `__print_int`'s `fprintf`, constant-folds FFI call arguments, and eliminates dead code across the Briv↔C boundary.

### Implementation (`src/main.rs`, `run_llvm_compile()`)
- Detect `clang` and `llvm-link` at pipeline start
- If available: `clang -c -emit-llvm -O2 briv_rt.c -o briv_rt.bc`
- Then: `llvm-link program.bc briv_rt.bc -o merged.bc`
- Then: `opt -O3 merged.bc -o merged.opt.bc`
- Then: `llc merged.opt.bc -filetype=obj -o program.o`
- Graceful fallback: if clang/llvm-link not installed, use current `cc` path
- No changes to `src/backend/llvm.rs`

### Tests
- Verify `print_loop` benchmark still compiles and runs
- Verify the merged IR contains inlined `fprintf`/`fputs` calls
- Verify graceful fallback when clang not installed

---

## Phase 2: MMIO Address Plumbing in LLVM Backend (~100 lines)

### Problem
`StateDecl.address: Option<u64>` is parsed but the LLVM backend only reads `s.name`, `s.ty`, and `s.expr` in `build_field_index()`. The `address` field is silently dropped. Every `@`-addressed field ends up in the `%State` struct — no MMIO semantics.

### Implementation

#### 2a. Index MMIO fields separately (`src/backend/llvm.rs`)
In `build_field_index()`, for each `StateDecl`:
- If `s.address.is_none()`: normal path (insert into `field_index_map` → `%State` struct member)
- If `s.address.is_some()`: store in new `mmio_fields: HashMap<String, u64>`, do NOT add to `%State`

New struct fields:
```rust
mmio_fields: HashMap<String, u64>,  // name → physical address
```

#### 2b. Emit MMIO reads
In `emit_expr` → `Expr::Identifier` path, BEFORE the `field_index_map` lookup:
```rust
if let Some(&addr) = self.mmio_fields.get(name) {
    let ptr_reg = format!("%i2p{}", self.txn_counter);
    let val_reg = format!("%ml{}", self.txn_counter);
    writeln!(out, "{} = inttoptr i64 {} to i64*", ptr_reg, addr);
    writeln!(out, "{} = load volatile i64, i64* {}, align 1", val_reg, ptr_reg);
    self.txn_counter += 1;
    return val_reg;
}
```

In SSA mode (`extractvalue` path): same check — MMIO fields are NOT in `%State`, always use inttoptr+load.

#### 2c. Emit MMIO writes
In `emit_stmt` → `Statement::Assignment` path, same precedence:
```rust
if let Some(&addr) = self.mmio_fields.get(&lhs) {
    // emit: %ptr = inttoptr i64 {addr} to i64*
    //        store volatile i64 {val}, i64* %ptr, align 1
    return;
}
```

In SSA mode: MMIO fields are NOT insertvalue candidates. Direct inttoptr+store.

#### 2d. Skip MMIO fields in emit_init_state()
Fields with addresses skip the normal `store` to `%State`. Their initial value is written to the MMIO address instead.

#### 2e. MMIO fields in exit condition evaluation
`emit_exit_expr` and precondition evaluation: same MMIO check — inttoptr+load instead of GEP.

### Tests
- `test_mmio_read`: `let reg: Int @ 0x1000 = 0; txn read [true][term == reg]` → verifies load volatile via inttoptr
- `test_mmio_write`: `let reg: Int @ 0x1000 = 0; txn write [true][reg == 5]` → verifies store volatile via inttoptr
- `test_mmio_init`: verify init_state writes to MMIO address
- `test_mmio_not_in_struct`: verify `%State` does not contain MMIO fields
- `test_non_mmio_unchanged`: normal `let x: Int = 0` still uses GEP

---

## Phase 3: Hardware Handoff Generator (~200 lines)

### 3a. Extend `c_analyzer.rs` for xparameters.h (~30 lines)
New function `extract_baseaddr_from_xparameters(content: &str) -> HashMap<String, u64>`:
- Scans lines for `#define XPAR_*_BASEADDR 0x...`
- Extracts: name = macro name minus `XPAR_` and `_BASEADDR`, lowercased; addr = hex value
- Ignores lines without `_BASEADDR`
- Returns name→address map

### 3b. `.xsa` XML parser (~60 lines + `zip` + `roxmltree` deps)
New function `extract_hw_from_xsa(xsa_path: &str) -> HashMap<String, (String, u64, u64)>`:
- Opens `.xsa` as zip archive
- Extracts `system.hwh` (XML)
- Parses `<MODULE INSTANCE="name"> → <MEMRANGES> → <MEMRANGE BASEVALUE="0x..." HIGHVALUE="0x..."/>`
- Returns map: cleaned_name → (type, base, high)

Graceful fallback if zip/xml crates not available: error message suggesting `cargo add`.

### 3c. DBVS schema generator (~50 lines)
New function `generate_dbvs_from_hw(hw: &HashMap) -> String`:
- For each register, emits:
```
register @0x{addr} as "{name}" { type: UInt; description: "Memory-mapped register"; };
```
- Optionally emits aliases:
```
alias {name}: UInt @0x{addr};
```

### 3d. Target `.dbv` generator (~50 lines)
New function `generate_dbv_from_hw(dbvs: &DbvsProgram, hw: &HashMap) -> String`:
- For each alias in the schema, looks up address from hw map
- If address found: emits matching record with address
- If address NOT found: warning — register in schema has no binding for this target
- Emits metadata comments with board/chip info

### 3e. CLI flags (`src/main.rs` ~60 lines)

| Flag | Effect |
|------|--------|
| `--hw-handoff system.xsa` | Extract → generate `.dbvs` + `.dbv` files |
| `--hw-handoff xparameters.h` | Extract from C header |
| `--target zcu4ev.dbv` | Bind schema aliases to this target's addresses |
| `--target sim` | Default — aliases are struct members, no MMIO |

When `--target <file>` is specified:
1. Parse the `.dbv` file
2. Parse the imported `.dbvs` schema(s)
3. For each alias in the schema, look up the resolved address in the `.dbv`
4. Feed resolved addresses into the LLVM backend's `mmio_fields` map
5. The proof engine validates contracts against logical alias names (unchanged)

### Tests
- `test_extract_xparameters`: parse sample header, verify name→addr map
- `test_generate_dbvs`: verify output contains register declarations
- `test_generate_dbv`: verify output maps aliases to addresses
- `test_hw_handoff_integration`: full pipeline: `.xsa` → `.dbvs` + `.dbv` → compile with `--target`

---

## Phase 4: DBVS Import → LLVM Alias Resolution (~80 lines)

### 4a. Schema import detection
In `resolve_imports()` or the LLVM pipeline entry point, detect `import "*.dbvs"`:
- Parse the schema into `DbvsProgram`
- Store in a new `schema_map: HashMap<String, DbvsProgram>`

### 4b. Target binding
When `--target <file>.dbv` is provided:
- Parse the `.dbv` into `DbrivProgram`
- For each schema referenced by the program, cross-reference aliases with the target bindings
- Resolve: alias_name → physical_address
- Feed into `mmio_fields`

### 4c. Error diagnostics
- **Error**: register in schema has no binding in target `.dbv`
- **Warning**: target `.dbv` binds a register not in any imported schema
- **Error**: alias type mismatch between schema and program usage

### Tests
- `test_schema_import_resolves`: import schema + target → mmio_fields populated
- `test_unbound_register_error`: schema alias without target binding → compiler error
- `test_type_mismatch_error`: schema says UInt, program uses Float → type error

---

## Execution Order

| # | Phase | Risk | Depends On | Lines |
|---|-------|------|-----------|-------|
| 1 | LTO Closure | Low | None | ~30 |
| 2 | MMIO Plumbing | Medium | None | ~100 |
| 3 | Hardware Handoff Generator | Medium | 2 | ~200 |
| 4 | DBVS Import Resolution | Medium | 2,3 | ~80 |

### Why LTO first
- Zero changes to codegen logic
- Establishes `llvm-link` pattern that Phase 3 can leverage if BSP code is compiled as LLVM bitcode
- Immediate benchmark benefit: `__print_int` inlined into `print_loop`

### Why MMIO before handoff
- The handoff generator produces addresses, but without Phase 2 they go nowhere
- Phase 2 can be tested standalone with hardcoded addresses before Phase 3 exists

## Non-Goals
- No COBOL backend changes (source-to-source transpiler, no LTO/linking story)
- No changes to proof engine (operates on logical names regardless of target)
- No changes to `@ link` trigger system (already correct in `emit_trg_load`)
- No changes to register/alias semantics in `src/dbriv/` (parser/AST/engine are correct)

## Success Criteria
- All 372 existing tests pass after each phase
- `print_loop` benchmark compiles and runs with LTO-merged IR
- `let x: Int @ 0x1000` emits `load volatile i64, i64* inttoptr (i64 4096 to i64*)` in LLVM IR
- `briv build --hw-handoff system.xsa --target zcu4ev.dbv` produces correct MMIO code
- Same program compiles for `--target sim` with GEP-based struct access
