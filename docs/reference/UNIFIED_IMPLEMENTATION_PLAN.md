# Unified Implementation Plan: DBriev + VHDL Target

**Date:** 2026-05-05
**Status:** Implementation Complete
**Related:**
- `DBRIEV_SPEC.md` - Language specification
- `VHDL_TARGET_RESEARCH.md` - VHDL research notes
- `BRIEV_ADVENTURES_RESEARCH.md` - Game engine concept (research only)
- `../briev-adventures/IMPLEMENTATION_PLAN.md` - Adventure demo plan (separate)

---

## 1. Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│  Briev (.bv/.ebv) - Logic & State Machines             │
├─────────────────────────────────────────────────────────┤
│  DBriev (.dbv/.dbvs/.dbvl) - Data & Rules              │
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

### ✅ Phase 2: DBriev Core (.dbv)

**Status:** Complete

- Parse `.dbv` files
- `@address` syntax resolution
- Type system (Briev types + Addr, RegOffset)
- `CHECK` contracts

### ✅ Phase 3: DBriev Query Engine

**Status:** Complete

- Arrow pipeline (`@1->FILTER|MAP|COUNT`)
- Bracket syntax (`@1[filter]`)
- ALIAS binding (schema `.dbvs` to config `.dbv`)
- `@auto` address allocation

### ✅ Phase 4: DBriev Rules (Basic)

**Status:** Partial - RULE syntax exists, inference not implemented

- RULE syntax (`RULE head :- body`)
- Non-recursive rules (basic Datalog)
- Full inference engine - deferred

### ✅ Phase 5: DBriev Database (.dbvl)

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
./briev-compiler verilog design.ebv --hw hardware.toml

# Verilog with DBriev config
./briev-compiler verilog design.ebv --hw config.dbv

# Verilog with DBriev schema
./briev-compiler verilog design.ebv --hw schema.dbvs

# VHDL compilation
./briev-compiler vhdl design.ebv --hw config.dbv
```

---

## 5. Success Criteria

| Phase | Criteria | Status |
|-------|----------|--------|
| VHDL | Valid .vhd output | ✅ Passes GHDL |
| DBriev Core | .dbv parses without error | ✅ Verified |
| DBriev Core | CHECK contracts verify | ✅ Verified |
| Query Engine | Queries return correct data | ✅ Tested |
| Query Engine | ALIAS binds correctly | ✅ Tested |
| DBriev .dbvl | Mutable operations work | ✅ Tested |
| DBriev .dbvs | Schema parse and import | ✅ Tested |

---

*Adventure research: ../briev-adventures/IMPLEMENTATION_PLAN.md*