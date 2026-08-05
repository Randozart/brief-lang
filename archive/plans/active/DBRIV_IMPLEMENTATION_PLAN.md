# DBriv Implementation Plan

**Date:** 2026-05-05
**Status:** Planning
**Related:** `docs/reference/DBRIV_SPEC.md`, `docs/reference/BRIV-DATALOG_RESEARCH.md`

---

## 1. Scope

Implement DBriv (Data Briv) as a new language variant for configuration and database files, with Briv integration.

---

## 2. Required Components

### 2.1 Data Layer

| Feature | Description | Priority |
|---------|-------------|----------|
| `.dbv` files | Static configuration format | HIGH |
| `.dbvl` files | Mutable line-based database | HIGH |
| `.dbvs` files | Schema/register definition | HIGH |
| `@address` syntax | Register/address addressing | HIGH |
| Type system | Briv types + Addr, RegOffset | HIGH |

### 2.2 ALIAS System

| Feature | Description | Priority |
|---------|-------------|----------|
| Symbol declaration | `ALIAS name: Type` in .dbvs | HIGH |
| Address binding | `ALIAS name: Type = @addr` in .dbv | HIGH |
| Optional aliases | `ALIAS? name: Type` | MEDIUM |
| @auto allocation | Auto-assign free addresses | HIGH |

### 2.3 Query System

| Feature | Description | Priority |
|---------|-------------|----------|
| Arrow syntax | `@1->FILTER\|COUNT\|MAP` | HIGH |
| Bracket syntax | `@1[filter]` | HIGH |
| QUERY pipeline | `QUERY @1 \| FILTER...` | MEDIUM |
| Aggregations | COUNT, SUM, AVG, MIN, MAX | HIGH |

### 2.4 Inference Rules

| Feature | Description | Priority |
|---------|-------------|----------|
| RULE syntax | `RULE head :- body` | MEDIUM |
| Basic rules | Non-recursive queries | HIGH |
| Recursive rules | Path finding, etc. | LOW |
| Stratified negation | NOT support | LOW |

### 2.5 Contracts

| Feature | Description | Priority |
|---------|-------------|----------|
| CHECK keyword | Data validation | HIGH |
| Compile-time verify | SMT checking | HIGH |
| Runtime verify | Value checking | MEDIUM |

---

## 3. Backend Targets

| Target | Description | Priority |
|--------|-------------|----------|
| Sled | Embedded KV store | MEDIUM |
| SQLite | SQL backend | MEDIUM |
| In-memory | Hot path / testing | HIGH |
| Remote | TCP/HTTP APIs | LOW |

---

## 4. Transpilation Targets

| Target | Description | Priority |
|--------|-------------|----------|
| SystemVerilog | Hardware config | HIGH |
| Rust | Host code | HIGH |
| C headers | Embedded C | MEDIUM |

---

## 5. Implementation Phases

### Phase 1: Core (.dbv)
- Parser for .dbv format
- @address resolution
- Type checking
- Compile-time contracts

### Phase 2: Query Engine
- Filter/map/count operators
- Bracket and arrow syntax
- Aggregations

### Phase 3: Schema (.dbvs)
- ALIAS declarations
- Validation against .dbvs
- @auto allocation

### Phase 4: Rules (Optional)
- RULE syntax
- Basic inference
- Recursive queries

### Phase 5: Database (.dbvl)
- Line-based format
- Mutable operations
- Transactions

---

## 6. Files to Create

| File | Purpose |
|------|---------|
| `src/dbriv/parser.rs` | Parse .dbv/.dbvl/.dbvs files |
| `src/dbriv/eval.rs` | Query evaluation engine |
| `src/dbriv/alloc.rs` | Address allocation |
| `src/dbriv/mod.rs` | Module entry |
| `src/dbriv/ast.rs` | DBriv AST |

---

## 7. Integration Points

| Integration | Description |
|--------------|-------------|
| Briv import | `.bv` can import .dbv via FFI |
| EBV binding | `.ebv` can bind to .dbv config |
| RBV data | `.rbv` views bind to .dbvl data |

---

## 8. Success Criteria

After implementation:

| Criteria | Verification |
|-----------|--------------|
| `.dbv` parses without error | `briv compile config.dbv` |
| `.dbv` contracts verify | Invalid data rejected at compile |
| Queries work | `@1->FILTER x > 0` returns correct |
| ALIAS binds correctly | Address resolution works |
| Transpiles to SV | Generates valid SystemVerilog |

---

## 9. Open Questions

1. **Storage backend** - Default to Sled or in-memory for first iteration?
2. **Remote support** - TCP/HTTP after or before rules?
3. **Scope for v1** - Just .dbv (config) or include .dbvl (database)?