# DBrief Universal Data Engine - Implementation Plan

**Date:** 2026-05-09
**Status:** Planning
**Version:** 1.0
**Related:** `docs/reference/DBRIEF_SPEC.md`, `docs/reference/BRIEF-DATALOG_RESEARCH.md`, `plans/active/DBRIEF_IMPLEMENTATION_PLAN.md`

---

## 1. Vision

DBrief (Data Brief) is a universal data language that unifies:

| Replaces | How |
|----------|-----|
| **XML** | Clean structural syntax, no verbose tags |
| **JSON/JSONL** | Schema-validated, addressable, self-describing |
| **SQL** | Queryable with filters, aggregations, joins |
| **MongoDB** | Flexible document storage, nested structures |
| **Prolog/Datalog** | Logical inference rules, RULE syntax |

### Core Properties

- **Built-in contracts** - Data self-verifies at load time
- **Addressable** - Records live at `@address` (hardware-friendly)
- **Dual mode** - Static config (`.dbv`) and mutable database (`.dbvl`)
- **Schema-driven** - `.dbvs` defines types, constraints, aliases

### Cross-Brief Interoperability

DBrief serves as the **data key** between all Brief variants:

| Brief Variant | DBrief Role |
|---------------|-------------|
| **Brief (.bv)** | FFI: `IMPORT "./data.dbv"` |
| **R-Brief** | Transpiles to SystemVerilog for hardware |
| **E-Brief (.ebv)** | Register binding via `.dbvs` |
| **RBV (.rbv)** | Reactive data binding to `.dbvl` |

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                            DBrief Universal Data Layer                          │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                              File Formats                                   │ │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐         │ │
│  │  │  .dbv   │  │  .dbvl  │  │  .dbvs  │  │  .bv    │  │  .ebv   │         │ │
│  │  │ Config  │  │ Database│  │ Schema  │  │  Brief  │  │  Brief  │         │ │
│  │  │(static) │  │(mutable)│  │(types)  │  │   Core  │  │  Hard.  │         │ │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘         │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                      │                                          │
│                                      ▼                                          │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                         DBrief Engine Core                                 │ │
│  │                                                                             │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │ │
│  │  │   Parser    │  │ Query Engine │  │  Contract   │  │   Storage   │      │ │
│  │  │   (.dbv,    │  │  (filter,    │  │  Verifier   │  │   Backend   │      │ │
│  │  │  .dbvl,     │  │   aggregate, │  │ (pre/post,  │  │   (file,    │      │ │
│  │  │   .dbvs)    │  │   join,      │  │   type,     │  │   sqlite,   │      │ │
│  │  │             │  │   sort)      │  │   custom)   │  │   memory)   │      │ │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘      │ │
│  │        │                │                │                │              │ │
│  │        └────────────────┼────────────────┼────────────────┘              │ │
│  │                         ▼                                                  │ │
│  │              ┌─────────────────────┐                                      │ │
│  │              │  Inference Engine   │  ← Prolog/Datalog rules              │ │
│  │              │  (RULE syntax)       │  ← Stratified negation              │ │
│  │              └─────────────────────┘                                      │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                      │                                          │
│                                      ▼                                          │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │              SystemVerilog Transpiler (R-Brief → Hardware)                │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────────────────────────────┐
        │                    Multi-Language Interfaces                    │
        │                                                                  │
        │   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐   │
        │   │   REST   │   │   gRPC   │   │   WASM   │   │    C    │   │
        │   │   API    │   │   API    │   │   Module │   │   FFI   │   │
        │   └──────────┘   └──────────┘   └──────────┘   └──────────┘   │
        │        │              │              │              │        │
        │        ▼              ▼              ▼              ▼        │
        │   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐│
        │   │ Python   │   │   Go     │   │  Node.js │   │   Ruby   ││
        │   │   SDK    │   │   SDK    │   │    SDK   │   │   SDK    ││
        │   └──────────┘   └──────────┘   └──────────┘   └──────────┘│
        └─────────────────────────────────────────────────────────────────┘
```

---

## 3. File Formats

### 3.1 .dbv - Static Configuration

Immutable configuration files. Loaded once, never modified at runtime.

```dbv
// network.dbv - Network configuration
[network]
ip_address: String = "192.168.1.1";
port: UInt[16] = 8080;
timeout_ms: UInt[32] = 5000;
enabled: Bool = true;

[network.security]
tls_enabled: Bool = true;
cert_path: String = "/etc/certs/server.crt";

check valid_port {
    port > 1024;
    port < 65535;
}

check valid_timeout {
    timeout_ms > 0;
    timeout_ms < 300000;
}
```

### 3.2 .dbvl - Mutable Database

Line-oriented mutable records. Each line is a self-contained record.

```dbvl
// users.dbvl - User database
@1 { "Alice", 30, "Engineer", active: true }
@1 { "Bob", 25, "Designer", active: false }
@1 { "Charlie", 35, "Manager", active: true }
@2 { "admin", true, permissions: ["read", "write", "delete"] }
@2 { "user", false, permissions: ["read"] }
@3 { "session_abc", user_id: 1, expires: 1699999999 }
@3 { "session_xyz", user_id: 2, expires: 1700000000 }

// With inline validation
@4 CHECK age > 0 AND age < 150 { "David", 45, "CTO" }
```

### 3.3 .dbvs - Schema Definition

Type definitions, register maps, aliases, and contracts.

```dbvs
// hardware_registers.dbvs - Register map for R-Brief
REGISTER @0x1000: Vector[Register]
REGISTER @0x2000: Vector[MemoryBlock]
REGISTER @0x3000: Vector[Interrupt]

STRUCT Register {
    name: String,
    address: UInt[32],
    width: UInt[8],
    access: Enum[ReadOnly, WriteOnly, ReadWrite],
    reset_value: UInt[32] = 0,
    description: Option[String]
}

STRUCT MemoryBlock {
    name: String,
    base: UInt[32],
    size: UInt[32],
    attributes: Vector[String],  // "cacheable", "executable", etc.
    description: Option[String]
}

STRUCT Interrupt {
    number: UInt[8],
    name: String,
    handler: UInt[32],
    priority: UInt[4],
    enabled: Bool
}

ALIAS stack_pointer: Addr = @0x1000;
ALIAS reset_vector: Addr = @0x1004;
ALIAS? optional_clock: Addr;  // Optional - may be unbound
```

---

## 4. Core Components

### 4.1 Parser Layer

**Location:** `src/dbrief/parser.rs` (existing)

| Parser | Purpose | Status |
|--------|---------|--------|
| `parse_dbrief` | Parse `.dbv` files | ✅ Implemented |
| `parse_dbvl` | Parse `.dbvl` files | ✅ Implemented |
| `parse_dbvs` | Parse `.dbvs` files | ✅ Implemented |

**Extensions needed:**
- RULE syntax parser for inference
- CHECK condition parser
- Import/resolution parser

### 4.2 Query Engine

**Location:** `src/dbrief/eval.rs` (existing, extend)

**Current capabilities:**
- Filter, Map, Sort, Limit, Skip, Unique
- Count, Sum, Avg, Min, Max, First, Last

**Add capabilities:**

| Operation | Description | Datalog Analogy |
|-----------|-------------|-----------------|
| `Join` | Cross-address joins | `R(x,y), S(y,z)` |
| `LeftJoin` | Left outer join | LEFT JOIN |
| `GroupBy` | Group and aggregate | SQL GROUP BY |
| `Having` | Filter groups | SQL HAVING |
| `Exists` | Subquery existence | ` EXISTS ` |
| `Not` | Stratified negation | `NOT R(x)` |
| `Recursive` | Recursive rules | Datalog recursive |

**Query syntax:**

```dbrief
// Pipeline syntax
@users->FILTER age > 25->SORT name->LIMIT 10

// Bracket syntax
@users[age > 25 AND role == "admin"]

// Full QUERY syntax
QUERY @users
  | FILTER role == "admin"
  | SORT age DESC
  | MAP name, email
  | LIMIT 5

// Aggregation
QUERY @orders | AGGREGATE SUM total BY customer_id

// Datalog-style rules
RULE is_admin(user) :- @users[user, role == "admin"]
RULE can_access(resource, user) :- @permissions[resource, user, access == true]
```

### 4.3 Contract Verification

**Location:** `src/dbrief/contract.rs` (new)

```rust
pub trait Contract {
    fn pre_validate(&self, record: &Record) -> Result<(), Vec<Violation>>;
    fn post_validate(&self, record: &Record) -> Result<(), Vec<Violation>>;
    fn type_check(&self, value: &Value, expected: &Type) -> Result<(), TypeError>;
}

pub struct Check {
    pub name: String,
    pub conditions: Vec<Expr>,
}

pub struct Violation {
    pub check_name: String,
    pub message: String,
    pub address: Address,
}
```

**Contract types:**
- **Pre-condition** - Valid before insert/update
- **Post-condition** - Valid after insert/update
- **Type checking** - Value matches declared type
- **Custom rules** - User-defined expressions

### 4.4 Inference Engine (Datalog-style)

**Location:** `src/dbrief/inference.rs` (new)

```rust
pub struct Rule {
    pub head: RuleHead,
    pub body: Vec<RuleBody>,
}

pub struct RuleHead {
    pub name: String,
    pub variables: Vec<String>,
}

pub enum RuleBody {
    Fact(String, Vec<(String, Value)>),
    Not(Box<RuleBody>),
    And(Box<RuleBody>, Box<RuleBody>),
    Or(Box<RuleBody>, Box<RuleBody>),
    Compare(Expr, CompareOp, Expr),
}

pub struct InferenceEngine {
    facts: HashMap<String, Vec<Fact>>,
    rules: Vec<Rule>,
}
```

**Example rules:**

```dbrief
// Simple rule
RULE is_adult(person) :- @persons[person, age >= 18]

// Conjunctive rule
RULE can_buy(user, product) :-
    @users[user, status == "active"]
    @cart[user, product]
    @inventory[product, stock > 0]

// Recursive rule (transitive closure)
RULE ancestor(ancestor, descendant) :- @parent[child, parent == ancestor]
RULE ancestor(ancestor, descendant) :- @parent[child, parent == mid] ancestor(mid, descendant)

// Stratified negation
RULE no_access(user) :- @users[user, suspended == true] NOT @access[user, active == true]
```

**Evaluation strategy:**
1. Stratification for negation
2. Semi-naive evaluation for recursion
3. Magic set rewriting for efficient evaluation

### 4.5 Storage Backend

**Location:** `lib/dbrief-engine/src/storage/` (new)

```rust
pub trait StorageBackend {
    fn load(&mut self, path: &str) -> Result<MutableEngine, DbriefError>;
    fn save(&mut self, engine: &MutableEngine, path: &str) -> Result<(), DbriefError>;
    fn export_json(&self, engine: &DbriefEngine) -> Result<String, DbriefError>;
    fn export_jsonl(&self, engine: &DbriefEngine) -> Result<String, DbriefError>;
    fn import_json(&mut self, json: &str) -> Result<MutableEngine, DbriefError>;
    fn import_jsonl(&mut self, jsonl: &str) -> Result<MutableEngine, DbriefError>;
}
```

| Backend | File Type | Description |
|---------|-----------|-------------|
| `JsonStorage` | `.dbv.json` | JSON representation |
| `JsonlStorage` | `.dbvl` (native) | Line-oriented |
| `MemoryStorage` | (none) | In-memory |
| `SqliteStorage` | `.db` | Indexed queries |

---

## 5. Cross-Brief Integration

### 5.1 Brief (.bv) FFI

```brief
// auth.bv - Brief state machine using DBrief data
IMPORT "./users.dbvl" AS users
IMPORT "./permissions.dbvs" AS perms

MACHINE auth {
    INITIAL state: unauthenticated
    
    state authenticated {
        entry {
            let user = users[@current_user_id];
            if user.active == false {
                transition_to(unauthenticated);
            }
        }
    }
    
    action has_permission(perm: String) -> Bool {
        let perms = @permissions[@current_user_id];
        return perms.contains(perm);
    }
}
```

### 5.2 E-Brief Register Binding

```ebv
// firmware.ebv - Embedded Brief with hardware registers
IMPORT "../hardware_registers.dbvs"

// Direct register access
LET gpio_port: Register = @0x1000;
LET uart0: Register = @0x2000;

// Memory-mapped I/O
FUNCTION init_uart() {
    gpio_port[0x00] = 0x80;  // Set divisor
    gpio_port[0x04] = 52;    // 115200 baud
    gpio_port[0x08] = 0x03;  // 8N1
}

// Interrupt handler bound to DBrief schema
INTERRUPT @0x3000[0] = timer_handler;
INTERRUPT @0x3000[1] = uart_handler;
```

### 5.3 R-Brief Hardware Transpilation

**Location:** `lib/dbrief-transpiler/src/sv.rs` (new)

```rust
pub fn transpile_to_systemverilog(schema: &DbvsProgram) -> String {
    // Generate module declaration
    // Generate register definitions
    // Generate memory maps
    // Generate AXI/APB interfaces
}
```

**Output example:**

```systemverilog
// Generated from hardware_registers.dbvs
module dbrief_registers (
    input  wire         clk,
    input  wire         rst_n,
    input  wire [31:0]  addr,
    input  wire [31:0]  wdata,
    input  wire         we,
    input  wire         re,
    output reg  [31:0]  rdata
);

    // Register 0x1000 - Register array
    logic [31:0] registers_0x1000 [0:255];
    
    // Register 0x2000 - Memory blocks
    logic [31:0] memory_0x2000 [0:4095];
    
    // Register 0x3000 - Interrupt controller
    logic [31:0] interrupt_enable;
    logic [31:0] interrupt_status;
    logic [31:0] interrupt_priority [16];
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            registers_0x1000 <= '0;
            memory_0x2000 <= '0;
            interrupt_enable <= '0;
            interrupt_status <= '0;
        end else begin
            if (we && addr >= 32'h1000 && addr < 32'h1100)
                registers_0x1000[addr[7:2]] <= wdata;
            // ... more logic
        end
    end
endmodule
```

### 5.4 RBV Reactive Data Binding

```rbv
// dashboard.rbv - Rendered Brief view
IMPORT "../users.dbvl" AS users

VIEW dashboard {
    STATE user_list: List[User]
    
    // Reactive query - updates when .dbvl changes
    QUERY active_users <- @users->FILTER active == true->MAP name, email
    
    RENDER {
        <div>
            <h1>Active Users: {active_users.length}</h1>
            <table>
                {FOR user IN active_users}
                <tr><td>{user.name}</td><td>{user.email}</td></tr>
                {END}
            </table>
        </div>
    }
}
```

---

## 6. Server & CLI

### 6.1 Server Binary: `dbriefd`

**Location:** `bin/dbriefd/` (new)

```
bin/dbriefd/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── server.rs       # axum server setup
│   ├── routes/
│   │   ├── schema.rs   # /schema/*
│   │   ├── data.rs     # /data/*
│   │   ├── query.rs    # /query/*
│   │   └── infer.rs    # /infer/*
│   └── handlers.rs
```

**REST API:**

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Health check |
| `POST` | `/schema/load` | Load `.dbvs` |
| `POST` | `/schema/validate` | Validate against schema |
| `GET` | `/data/:address` | Read record |
| `POST` | `/data` | Insert record |
| `PUT` | `/data/:address` | Update record |
| `DELETE` | `/data/:address` | Delete record |
| `POST` | `/query` | Execute pipeline |
| `POST` | `/infer` | Execute rule |
| `POST` | `/verify` | Verify contracts |
| `GET` | `/export/json` | Export JSON |
| `GET` | `/export/jsonl` | Export JSONL |
| `POST` | `/import/json` | Import JSON |
| `POST` | `/import/jsonl` | Import JSONL |

**gRPC API** (optional):

```protobuf
service DBriefService {
    rpc LoadSchema(LoadSchemaRequest) returns (LoadSchemaResponse);
    rpc Insert(InsertRequest) returns (InsertResponse);
    rpc Update(UpdateRequest) returns (UpdateResponse);
    rpc Delete(DeleteRequest) returns (DeleteResponse);
    rpc Query(QueryRequest) returns (QueryResponse);
    rpc Infer(InferRequest) returns (InferResponse);
    rpc Verify(VerifyRequest) returns (VerifyResponse);
}
```

### 6.2 CLI Tool: `dbrief`

**Location:** Extend `bin/main.rs` or new CLI

```bash
# Load and query
dbrief query data.dbvl --filter 'role == "admin"'
dbrief query data.dbvl --aggregate 'count BY role'
dbrief query data.dbvl --pipeline '@1->FILTER age > 25->SORT name'

# Datalog inference
dbrief infer data.dbvl --rule 'is_admin(X) :- role(X, "admin")'

# Import/Export
dbrief export data.dbv --format json --output data.json
dbrief export data.dbv --format jsonl --output data.jsonl
dbrief import data.jsonl --format jsonl --output data.dbvl

# Verification
dbrief verify data.dbv --schema schema.dbvs
dbrief check data.dbvl --contracts

# Schema operations
dbrief schema show schema.dbvs
dbrief schema validate data.dbv --schema schema.dbvs

# Hardware transpilation
dbrief transpile schema.dbvs --target systemverilog --output hardware.sv

# Server operations
dbrief serve --port 8080 --data data.dbvl
dbrief serve --schema schema.dbvs --mode read-only
```

---

## 7. Language Bindings

### 7.1 C FFI

**Location:** `lib/dbrief-ffi/` (new)

```c
// dbrief.h - C API
typedef struct dbrief_engine_s* dbrief_engine_t;
typedef struct dbrief_result_s* dbrief_result_t;
typedef struct dbrief_error_s* dbrief_error_t;

dbrief_engine_t* dbrief_engine_new();
void dbrief_engine_free(dbrief_engine_t*);

dbrief_error_t dbrief_load_dbvl(dbrief_engine_t*, const char* path);
dbrief_error_t dbrief_load_dbvs(dbrief_engine_t*, const char* path);
dbrief_error_t dbrief_load_dbv(dbrief_engine_t*, const char* path);

dbrief_result_t dbrief_insert(dbrief_engine_t*, const char* table, const char* json);
dbrief_result_t dbrief_update(dbrief_engine_t*, uint64_t addr, const char* json);
dbrief_result_t dbrief_delete(dbrief_engine_t*, uint64_t addr);
char* dbrief_read(dbrief_engine_t*, uint64_t addr);

char* dbrief_query(dbrief_engine_t*, const char* pipeline_json);
char* dbrief_infer(dbrief_engine_t*, const char* rule_json);

char* dbrief_export_json(dbrief_engine_t*);
char* dbrief_export_jsonl(dbrief_engine_t*);

char* dbrief_verify(dbrief_engine_t*, const char* schema_path);
```

### 7.2 Language SDKs

| Language | Method | Location |
|----------|--------|----------|
| **Python** | PyO3 | `lib/sdk/python/` |
| **JavaScript** | WASM | `lib/sdk/js/` |
| **Go** | cgo | `lib/sdk/go/` |
| **Ruby** | FFI | `lib/sdk/ruby/` |
| **Rust** | native | `lib/dbrief-engine/` |

**Python example:**

```python
import dbrief

engine = dbrief.load("users.dbvl")
result = engine.query('@users->FILTER age > 25')
for record in result.records:
    print(record["name"])

engine.insert({"name": "Dave", "age": 28, "role": "admin"})
engine.save()
```

**JavaScript example:**

```javascript
import { DbriefEngine } from "@brief/dbrief-js";

const engine = await DbriefEngine.load("users.dbvl");
const result = engine.query('@users->FILTER age > 25');
console.log(result.records);

await engine.insert({ name: "Dave", age: 28 });
await engine.save();
```

---

## 8. Implementation Phases

### Phase 1: Core Engine (Months 1-2)

| Task | Description | Priority |
|------|-------------|----------|
| 1.1 | Create `dbrief-engine` crate | HIGH |
| 1.2 | Refactor existing parser/eval/ast | HIGH |
| 1.3 | Implement unified Engine trait | HIGH |
| 1.4 | Add full query operations (join, group, having) | HIGH |
| 1.5 | Implement contract verification | HIGH |
| 1.6 | Add FromJson/ToJson for all types | HIGH |
| 1.7 | Pass existing tests | HIGH |

### Phase 2: Storage & Contracts (Month 3)

| Task | Description | Priority |
|------|-------------|----------|
| 2.1 | Define Storage trait | HIGH |
| 2.2 | Implement JsonStorage | HIGH |
| 2.3 | Implement JsonlStorage | HIGH |
| 2.4 | Implement contract checker | HIGH |
| 2.5 | Add type validation | MEDIUM |
| 2.6 | Add address allocation | MEDIUM |

### Phase 3: Inference Engine (Month 4)

| Task | Description | Priority |
|------|-------------|----------|
| 3.1 | Implement RULE parser | HIGH |
| 3.2 | Implement fact storage | HIGH |
| 3.3 | Implement rule evaluation | HIGH |
| 3.4 | Implement stratified negation | MEDIUM |
| 3.5 | Implement recursive rules | MEDIUM |

### Phase 4: Server & CLI (Month 5)

| Task | Description | Priority |
|------|-------------|----------|
| 4.1 | Create dbriefd binary | HIGH |
| 4.2 | Implement REST API | HIGH |
| 4.3 | Add CRUD endpoints | HIGH |
| 4.4 | Add query endpoint | HIGH |
| 4.5 | Add import/export endpoints | HIGH |
| 4.6 | Build dbrief CLI | HIGH |

### Phase 5: Cross-Brief Integration (Month 6)

| Task | Description | Priority |
|------|-------------|----------|
| 5.1 | Brief → DBrief FFI | HIGH |
| 5.2 | E-Brief register binding | HIGH |
| 5.3 | RBV reactive binding | MEDIUM |
| 5.4 | R-Brief SystemVerilog transpiler | HIGH |

### Phase 6: Language Bindings (Month 7)

| Task | Description | Priority |
|------|-------------|----------|
| 6.1 | Implement C FFI | HIGH |
| 6.2 | Generate C headers | HIGH |
| 6.3 | Create Python SDK | HIGH |
| 6.4 | Create WASM module | HIGH |
| 6.5 | Test cross-language integration | HIGH |

### Phase 7: Polish & Docs (Month 8)

| Task | Description | Priority |
|------|-------------|----------|
| 7.1 | Performance optimization | MEDIUM |
| 7.2 | Error message improvements | MEDIUM |
| 7.3 | Complete documentation | HIGH |
| 7.4 | Tutorial & examples | HIGH |
| 7.5 | Benchmarks | LOW |

---

## 9. File Structure

```
brief-compiler/
├── lib/
│   ├── dbrief-engine/           # Core engine (new)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs
│   │       ├── address.rs
│   │       ├── value.rs
│   │       ├── record.rs
│   │       ├── schema.rs
│   │       ├── static.rs
│   │       ├── mutable.rs
│   │       ├── query/
│   │       │   ├── mod.rs
│   │       │   ├── pipeline.rs
│   │       │   ├── filter.rs
│   │       │   ├── aggregate.rs
│   │       │   └── join.rs
│   │       ├── contract/
│   │       │   ├── mod.rs
│   │       │   ├── pre.rs
│   │       │   ├── post.rs
│   │       │   └── type_check.rs
│   │       ├── inference/
│   │       │   ├── mod.rs
│   │       │   ├── rules.rs
│   │       │   └── eval.rs
│   │       └── storage/
│   │           ├── mod.rs
│   │           ├── json.rs
│   │           ├── jsonl.rs
│   │           ├── memory.rs
│   │           └── sqlite.rs
│   │
│   ├── dbrief-transpiler/       # Hardware transpiler (new)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── sv.rs
│   │
│   ├── dbrief-ffi/              # C FFI bindings (new)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   └── ffi/                     # Existing FFI (keep)
│
├── bin/
│   ├── dbriefd/                 # Server (new)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       └── routes/
│   │
│   └── brief/                   # Existing CLI (extend)
│
├── src/
│   └── dbrief/                  # Existing (keep, refactor to use lib)
│       ├── parser.rs
│       ├── ast.rs
│       ├── eval.rs
│       ├── alloc.rs
│       └── mod.rs
│
├── docs/
│   └── reference/
│       └── DBRIEF_SPEC.md       # Update with v1 spec
│
└── plans/
    └── active/
        └── DBRIEF_UNIVERSAL_DATA_ENGINE_PLAN.md  # This file
```

---

## 10. Success Criteria

| Criteria | Verification |
|----------|-------------|
| Parse `.dbv` files | `dbrief load config.dbv` succeeds |
| Parse `.dbvl` files | `dbrief load data.dbvl` succeeds |
| Parse `.dbvs` files | `dbrief schema show schema.dbvs` works |
| Query works | `@users->FILTER age > 25` returns correct results |
| Aggregations work | `AGGREGATE COUNT BY role` works |
| Joins work | Cross-address joins return correct results |
| Contracts verify | Invalid data rejected with clear errors |
| Inference works | RULE queries return correct deductions |
| Import/Export JSON | Round-trip preserves data |
| Import/Export JSONL | Line-based format works |
| SystemVerilog output | Generates valid SV from `.dbvs` |
| REST API works | All endpoints respond correctly |
| Python SDK works | Can load/query from Python |
| WASM works | Can run in browser |

---

## 11. Open Questions

1. **Storage priority** - Start with JSON/JSONL, add SQLite later?
2. **Web framework** - Use axum, actix-web, or warp for server?
3. **Inference scope** - Full recursion + negation, or basic rules first?
4. **Transpilation format** - SystemVerilog only, or also VHDL, Chisel?

---

## 12. Related Documents

- `docs/reference/DBRIEF_SPEC.md` - Language specification
- `docs/reference/BRIEF-DATALOG_RESEARCH.md` - Datalog research
- `plans/active/DBRIEF_IMPLEMENTATION_PLAN.md` - Previous plan (superseded)
- `docs/EMBEDDED_BRIEF_2.2_SPEC.md` - E-Brief specification

---

**End of Plan**