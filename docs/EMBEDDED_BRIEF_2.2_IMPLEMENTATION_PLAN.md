# Embedded Brief 2.2 Implementation Plan

## 1. Goal
Implement the features for Embedded Brief 2.2, unifying logic across Core (.bv), Rendered (.rbv), and Embedded (.ebv) variants with high-velocity hardware primitives.

---

## 2. Shared Core Infrastructure (Phases 1-2)

### Phase 1: Lexer & AST Update
**Target Files:** `src/lexer.rs`, `src/ast.rs`
- [ ] **Lexer:** Add keywords `within`, `trg`, `forall`, `exists`, `within`. Add time units: `cycles`, `ms`, `s`, `min`.
- [ ] **AST:** 
    - Add `Vector(Box<Type>, Vec<usize>)` to `Type` enum.
    - Add `VectorIndex` and `VectorSlice` variants to `Expr`.
    - Support `Type | Type` (Union) in `StateDecl` and `Local` variables.
- [ ] **Parser:** Update `parse_type` (recursive `Type[N][M]` and `A | B`) and `parse_statement` (assignment `within <time> <unit>`).

### Phase 2: Typechecker & Logic Engine
**Target File:** `src/typechecker.rs`
- [ ] **Vector Lifting:** Implement logic allowing `Vector op Scalar` and `Vector op Vector`.
- [ ] **Geometry Validation:** Enforce dimensions match for parallel operations (`Int[100] + Int[100]` OK, `Int[100] + Int[50]` Error).
- [ ] **Mandatory Error Handling:** Detect `Union` type in assignment; block `term` if `Error` variant path is unhandled by guards (`[result is Error]`).
- [ ] **Unit Validation:** Block `.ebv` files from using non-supported time units (e.g., `cycles` in `.rbv`).

---

## 3. Variant-Specific Codegen (Phases 3-4)

### Phase 3: Embedded Backend (`src/embedded/`)
- [ ] **Memory Mapping:** Load and parse memory TOML, perform address/reserved-range validation.
- [ ] **Vector Codegen:** Map vector operations to hardware wide-buses/SIMD (e.g., `&bus[::3] = 1`).
- [ ] **Timeout Logic:** Map `within` to hardware watchdog timers/interrupts.
- [ ] **Reactor Gen:** Map `rct txn` to static jumps for hardware interrupts.

### Phase 4: Rendered Backend (`src/wasm_gen.rs`)
- [ ] **TypedArrays:** Map `Int[N]` to `Int32Array` or `Float32Array`.
- [ ] **Slicing:** Map `vec[start..end]` to TypedArray `.subarray()`.
- [ ] **Timeout Logic:** Map `within` to `Promise.race` and `setTimeout`.

---

## 4. Cross-Variant Integration & Build (Phase 5)

**Target Files:** `src/main.rs`, `src/import_resolver.rs`
- [ ] **Import Resolver:** Allow `.ebv`, `.rbv`, and `.bv` to share logic via `import "shared.bv"`.
- [ ] **Target System:** Add `--target embedded` CLI flag.
- [ ] **Preset Library:** Configure `chip_maps/` folder for `lwip.toml`, `tinyusb.toml`, and platform defaults.

---

## 5. Success Criteria
1.  Compiler parses `let frame: UInt[3][1920][1080] @/address;`.
2.  Geometry Mismatch error fires for mismatched slices.
3.  `let res: String | Error = f() within 5s;` forces explicit error checking.
4.  `.ebv` backend generates `no_std` Rust with mapped registers.
5.  `.rbv` backend generates JS using TypedArrays for performance.

---

*Status: Plan Finalized. Ready to commence implementation upon user command.*