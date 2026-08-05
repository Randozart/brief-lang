# Unified Implementation Plan: DBriv + VHDL Target

**Date:** 2026-05-05
**Status:** Implementation Complete
**Related:**
- `DBRIV_SPEC.md` - Language specification
- `VHDL_TARGET_RESEARCH.md` - VHDL research notes
- `BRIV_ADVENTURES_RESEARCH.md` - Game engine concept (research only)
- `../briv-adventures/IMPLEMENTATION_PLAN.md` - Adventure demo plan (separate)

---

## 1. Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│  Briv (.bv/.ebv) - Logic & State Machines             │
├─────────────────────────────────────────────────────────┤
│  DBriv (.dbv/.dbvs/.dbvl) - Data & Rules              │
│  - .dbv: Static configuration (ALIAS with addresses)  │
│  - .dbvs: Schema definitions (REGISTER, STRUCT, ENUM)  │
│  - .dbvl: Mutable line-based database                 │
├─────────────────────────────────────────────────────────┤
│  Transpile Targets: C | Rust | Verilog | VHDL | WASM  │
└─────────────────────────────────────────────────────────┘
```

---

## 2. Implementation Phases

### ✅ Phase 1: VHDL Backend

**Status:** Complete

- Add `--target vhdl` to CLI
- Type mapping (std_logic, signed, unsigned)
- Transaction → VHDL process translation

### ✅ Phase 2: DBriv Core (.dbv)

**Status:** Complete

- Parse `.dbv` files
- `@address` syntax resolution
- Type system (Briv types + Addr, RegOffset)
- `CHECK` contracts

### ✅ Phase 3: DBriv Query Engine

**Status:** Complete

- Arrow pipeline (`@1->FILTER|MAP|COUNT`)
- Bracket syntax (`@1[filter]`)
- ALIAS binding (schema `.dbvs` to config `.dbv`)
- `@auto` address allocation

### ✅ Phase 4: DBriv Rules (Basic)

**Status:** Partial - RULE syntax exists, inference not implemented

- RULE syntax (`RULE head :- body`)
- Non-recursive rules (basic Datalog)
- Full inference engine - deferred

### ✅ Phase 5: DBriv Database (.dbvl)

**Status:** Complete

- Line-based format (each line = record)
- Mutable operations (Insert, Update, Delete)
- DbvlEngine for runtime operations

---

## 3. File Types

| Extension | Purpose | Status |
|-----------|---------|--------|
| `.dbv` | Configuration with address bindings | ✅ Complete |
| `.dbvs` | Schema template definitions | ✅ Complete |
| `.dbvl` | Mutable line-based database | ✅ Complete |

---

## 4. Usage

```bash
# Verilog with hardware config
./briv-compiler verilog design.ebv --hw hardware.toml

# Verilog with DBriv config
./briv-compiler verilog design.ebv --hw config.dbv

# Verilog with DBriv schema
./briv-compiler verilog design.ebv --hw schema.dbvs

# VHDL compilation
./briv-compiler vhdl design.ebv --hw config.dbv
```

---

## 5. Success Criteria

| Phase | Criteria | Status |
|-------|----------|--------|
| VHDL | Valid .vhd output | ✅ Passes GHDL |
| DBriv Core | .dbv parses without error | ✅ Verified |
| DBriv Core | CHECK contracts verify | ✅ Verified |
| Query Engine | Queries return correct data | ✅ Tested |
| Query Engine | ALIAS binds correctly | ✅ Tested |
| DBriv .dbvl | Mutable operations work | ✅ Tested |
| DBriv .dbvs | Schema parse and import | ✅ Tested |

---

*Adventure research: ../briv-adventures/IMPLEMENTATION_PLAN.md*