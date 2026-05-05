# Unified Implementation Plan: DBrief + VHDL Target

**Date:** 2026-05-05
**Status:** Planning → Implementation Started
**Related:**
- `DBRIEF_SPEC.md` - Language specification
- `VHDL_TARGET_RESEARCH.md` - VHDL research notes
- `BRIEF_ADVENTURES_RESEARCH.md` - Game engine concept (research only)
- `../brief-adventures/IMPLEMENTATION_PLAN.md` - Adventure demo plan (separate)

---

## 1. Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│  Brief (.bv/.ebv) - Logic & State Machines             │
├─────────────────────────────────────────────────────────┤
│  DBrief (.dbv/.dbvs/.dbvl) - Data & Rules              │
│  - .dbv: Static configuration                          │
│  - .dbvs: Schema definitions                            │
│  - .dbvl: Mutable line-based database                  │
├─────────────────────────────────────────────────────────┤
│  Transpile Targets: C | Rust | Verilog | VHDL | WASM  │
└─────────────────────────────────────────────────────────┘
```

---

## 2. Implementation Phases

### Phase 1: VHDL Backend (Quick Win)

**Purpose:** Add second transpile target for `.ebv` files alongside SystemVerilog.

**Scope:**
- Add `--target vhdl` to CLI
- Type mapping (std_logic, signed, unsigned)
- Transaction → VHDL process translation
- Brief contracts → PSL assertions

**Files to Create:**
- `src/backend/vhdl.rs` - VHDL code generation (~1500 lines)

**Files to Modify:**
- `src/backend/mod.rs` - Register VHDL module
- `src/main.rs` - Add VHDL target option

**Deliverable:** Generate valid `.vhd` files that compile in GHDL

---

### Phase 2: DBrief Core (Data Layer)

**Purpose:** Implement `.dbv` (static config) parser and type system.

**Scope:**
- Parse `.dbv` files
- `@address` syntax resolution
- Type system (Brief types + Addr, RegOffset)
- `CHECK` contracts with compile-time verification

**Files to Create:**
- `src/dbrief/ast.rs` - DBrief AST types
- `src/dbrief/parser.rs` - Parse `.dbv` files
- `src/dbrief/mod.rs` - Module entry point

**Deliverable:** Parse a `.dbv` file and validate against spec

---

### Phase 3: DBrief Query Engine

**Purpose:** Build query evaluation for data extraction.

**Scope:**
- Arrow pipeline (`@1->FILTER|MAP|COUNT`)
- Bracket syntax (`@1[filter]`)
- Aggregations (COUNT, SUM, AVG, MIN, MAX)
- ALIAS binding (schema `.dbvs` to config `.dbv`)
- `@auto` address allocation

**Files to Create:**
- `src/dbrief/eval.rs` - Query evaluation engine
- `src/dbrief/alloc.rs` - Address allocation

**Deliverable:** Query `.dbv` data and return filtered/mapped results

---

### Phase 4: DBrief Rules (Universal Logic)

**Purpose:** Add inference rules for universal logic across domains.

**Scope:**
- RULE syntax (`RULE head :- body`)
- Non-recursive rules (basic Datalog)
- Stratified negation support (later)
- Hardware: "Can signal A reach register B?"
- Game: "Can player go from A to B?"

**Files to Create:**
- `src/dbrief/rules.rs` - Rule evaluation engine

**Deliverable:** Define and query rules that work across hardware and game domains

---

### Phase 5: DBrief Database (.dbvl)

**Purpose:** Add mutable line-based database support.

**Scope:**
- Line-based format (each line = record)
- Mutation operations (INSERT, UPDATE, DELETE)
- Transaction support
- Backend selection (Sled, SQLite, in-memory)

**Files to Create:**
- `src/dbrief/db.rs` - Database operations
- `src/dbrief/backends/` - Pluggable storage

**Deliverable:** Mutable database with persistent storage

---

## 3. VHDL Backend Details

### 3.1 Type Mapping

| Brief Type | VHDL Type |
|------------|-----------|
| `Bool` | `std_logic` |
| `UInt[N]` | `std_logic_vector(N-1 downto 0)` |
| `Int[N]` | `signed(N-1 downto 0)` |
| `Float` | `real` |
| `Vector[T, N]` | `array(0 to N-1) of T` |

### 3.2 Contract → PSL

| Brief Contract | PSL Property |
|----------------|--------------|
| `pre[condition]` | `assert never condition` |
| `post[condition]` | `assert always condition -> next` |

### 3.3 Output

- Target VHDL-2008
- PSL IEEE 1850-2005
- File extension: `.vhd`

---

## 4. DBrief Specification Summary

### 4.1 File Types

| Extension | Purpose | Mutable |
|-----------|---------|---------|
| `.dbv` | Configuration | No |
| `.dbvs` | Schema definition | No |
| `.dbvl` | Database | Yes |

### 4.2 Core Syntax

```brief
// Address
@1, @0xFF5E0000, @auto

// Register definition
REGISTER @1: Vector[Person]

// Alias (schema)
ALIAS led: Bool

// Alias binding (config)
ALIAS led: Bool = @0xFF5E0000

// Query
@1->FILTER age > 25

// Rule
RULE can_write(U) :- user{ id: U, role: "admin" }

// Contract
CHECK age > 18
```

---

## 5. Integration Points

| Integration | Description |
|-------------|-------------|
| Brief import | `.bv` can import `.dbv` via FFI |
| EBV binding | `.ebv` can bind to `.dbv` config |
| RBV data | `.rbv` views bind to `.dbvl` data |
| Verilog/SV | DBrief transpiles to SystemVerilog |
| VHDL | DBrief transpiles to VHDL |

---

## 6. Success Criteria

| Phase | Criteria | Verification |
|-------|----------|--------------|
| VHDL | Valid .vhd output | Compiles in GHDL |
| DBrief Core | .dbv parses without error | `brief compile config.dbv` |
| DBrief Core | CHECK contracts verify | Invalid data rejected |
| Query Engine | Queries return correct data | `@1->FILTER x > 0` works |
| Query Engine | ALIAS binds correctly | Address resolution works |
| Rules | Non-recursive rules eval | RULE returns proofs |

---

## 7. Open Questions (deferred)

1. **Recursive rules**: Include basic recursion or defer to later?
2. **Remote backends**: TCP/HTTP after or before rules?
3. **Adventure demo**: Build after Phase 4 complete?

---

*Plan references: DBRIEF_SPEC.md, VHDL_TARGET_RESEARCH.md*
*Adventure research: ../brief-adventures/IMPLEMENTATION_PLAN.md*