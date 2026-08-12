# DBriev Universal Data Engine - Implementation Plan

**Date:** 2026-05-09
**Status:** Planning
**Version:** 1.0
**Related:** `docs/reference/DBRIEV_SPEC.md`, `docs/reference/BRIEV-DATALOG_RESEARCH.md`, `plans/active/DBRIEV_IMPLEMENTATION_PLAN.md`

---

## 1. Vision

DBriev (Data Briev) is a universal data language that unifies:

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

### Cross-Briev Interoperability

DBriev serves as the **data key** between all Briev variants:

| Briev Variant | DBriev Role |
|---------------|-------------|
| **Briev (.bv)** | FFI: `IMPORT "./data.dbv"` |
| **R-Briev** | Transpiles to SystemVerilog for hardware |
| **E-Briev (.ebv)** | Register binding via `.dbvs` |
| **RBV (.rbv)** | Reactive data binding to `.dbvl` |

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                            DBriev Universal Data Layer                          │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                              File Formats                                   │ │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐         │ │
│  │  │  .dbv   │  │  .dbvl  │  │  .dbvs  │  │  .bv    │  │  .ebv   │         │ │
│  │  │ Config  │  │ Database│  │ Schema  │  │  Briev  │  │  Briev  │         │ │
│  │  │(static) │  │(mutable)│  │(types)  │  │   Core  │  │  Hard.  │         │ │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘         │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                      │                                          │
│                                      ▼                                          │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                         DBriev Engine Core                                 │ │
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
│  │              SystemVerilog Transpiler (R-Briev → Hardware)                │ │
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
// hardware_registers.dbvs - Register map for R-Briev
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

**Location:** `src/dbriev/parser.rs` (existing)

| Parser | Purpose | Status |
|--------|---------|--------|
| `parse_dbriev` | Parse `.dbv` files | ✅ Implemented |
| `parse_dbvl` | Parse `.dbvl` files | ✅ Implemented |
| `parse_dbvs` | Parse `.dbvs` files | ✅ Implemented |

**Extensions needed:**
- RULE syntax parser for inference
- CHECK condition parser
- Import/resolution parser

### 4.2 Query Engine

**Location:** `src/dbriev/eval.rs` (existing, extend)

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

```dbriev
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

**Location:** `src/dbriev/contract.rs` (new)

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

**Location:** `src/dbriev/inference.rs` (new)

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

```dbriev
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

**Location:** `lib/dbriev-engine/src/storage/` (new)

```rust
pub trait StorageBackend {
    fn load(&mut self, path: &str) -> Result<MutableEngine, DbrievError>;
    fn save(&mut self, engine: &MutableEngine, path: &str) -> Result<(), DbrievError>;
    fn export_json(&self, engine: &DbrievEngine) -> Result<String, DbrievError>;
    fn export_jsonl(&self, engine: &DbrievEngine) -> Result<String, DbrievError>;
    fn import_json(&mut self, json: &str) -> Result<MutableEngine, DbrievError>;
    fn import_jsonl(&mut self, jsonl: &str) -> Result<MutableEngine, DbrievError>;
}
```

| Backend | File Type | Description |
|---------|-----------|-------------|
| `JsonStorage` | `.dbv.json` | JSON representation |
| `JsonlStorage` | `.dbvl` (native) | Line-oriented |
| `MemoryStorage` | (none) | In-memory |
| `SqliteStorage` | `.db` | Indexed queries |

---

## 5. Cross-Briev Integration

### 5.1 Briev (.bv) FFI

```briev
// auth.bv - Briev state machine using DBriev data
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

### 5.2 E-Briev Register Binding

```ebv
// firmware.ebv - Embedded Briev with hardware registers
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

// Interrupt handler bound to DBriev schema
INTERRUPT @0x3000[0] = timer_handler;
INTERRUPT @0x3000[1] = uart_handler;
```

### 5.3 R-Briev Hardware Transpilation

**Location:** `lib/dbriev-transpiler/src/sv.rs` (new)

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
module dbriev_registers (
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
// dashboard.rbv - Rendered Briev view
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

### 6.1 Server Binary: `dbrievd`

**Location:** `bin/dbrievd/` (new)

```
bin/dbrievd/
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
service DBrievService {
    rpc LoadSchema(LoadSchemaRequest) returns (LoadSchemaResponse);
    rpc Insert(InsertRequest) returns (InsertResponse);
    rpc Update(UpdateRequest) returns (UpdateResponse);
    rpc Delete(DeleteRequest) returns (DeleteResponse);
    rpc Query(QueryRequest) returns (QueryResponse);
    rpc Infer(InferRequest) returns (InferResponse);
    rpc Verify(VerifyRequest) returns (VerifyResponse);
}
```

### 6.2 CLI Tool: `dbriev`

**Location:** Extend `bin/main.rs` or new CLI

```bash
# Load and query
dbriev query data.dbvl --filter 'role == "admin"'
dbriev query data.dbvl --aggregate 'count BY role'
dbriev query data.dbvl --pipeline '@1->FILTER age > 25->SORT name'

# Datalog inference
dbriev infer data.dbvl --rule 'is_admin(X) :- role(X, "admin")'

# Import/Export
dbriev export data.dbv --format json --output data.json
dbriev export data.dbv --format jsonl --output data.jsonl
dbriev import data.jsonl --format jsonl --output data.dbvl

# Verification
dbriev verify data.dbv --schema schema.dbvs
dbriev check data.dbvl --contracts

# Schema operations
dbriev schema show schema.dbvs
dbriev schema validate data.dbv --schema schema.dbvs

# Hardware transpilation
dbriev transpile schema.dbvs --target systemverilog --output hardware.sv

# Server operations
dbriev serve --port 8080 --data data.dbvl
dbriev serve --schema schema.dbvs --mode read-only
```

---

## 7. Language Bindings

### 7.1 C FFI

**Location:** `lib/dbriev-ffi/` (new)

```c
// dbriev.h - C API
typedef struct dbriev_engine_s* dbriev_engine_t;
typedef struct dbriev_result_s* dbriev_result_t;
typedef struct dbriev_error_s* dbriev_error_t;

dbriev_engine_t* dbriev_engine_new();
void dbriev_engine_free(dbriev_engine_t*);

dbriev_error_t dbriev_load_dbvl(dbriev_engine_t*, const char* path);
dbriev_error_t dbriev_load_dbvs(dbriev_engine_t*, const char* path);
dbriev_error_t dbriev_load_dbv(dbriev_engine_t*, const char* path);

dbriev_result_t dbriev_insert(dbriev_engine_t*, const char* table, const char* json);
dbriev_result_t dbriev_update(dbriev_engine_t*, uint64_t addr, const char* json);
dbriev_result_t dbriev_delete(dbriev_engine_t*, uint64_t addr);
char* dbriev_read(dbriev_engine_t*, uint64_t addr);

char* dbriev_query(dbriev_engine_t*, const char* pipeline_json);
char* dbriev_infer(dbriev_engine_t*, const char* rule_json);

char* dbriev_export_json(dbriev_engine_t*);
char* dbriev_export_jsonl(dbriev_engine_t*);

char* dbriev_verify(dbriev_engine_t*, const char* schema_path);
```

### 7.2 Language SDKs

| Language | Method | Location |
|----------|--------|----------|
| **Python** | PyO3 | `lib/sdk/python/` |
| **JavaScript** | WASM | `lib/sdk/js/` |
| **Go** | cgo | `lib/sdk/go/` |
| **Ruby** | FFI | `lib/sdk/ruby/` |
| **Rust** | native | `lib/dbriev-engine/` |

**Python example:**

```python
import dbriev

engine = dbriev.load("users.dbvl")
result = engine.query('@users->FILTER age > 25')
for record in result.records:
    print(record["name"])

engine.insert({"name": "Dave", "age": 28, "role": "admin"})
engine.save()
```

**JavaScript example:**

```javascript
import { DbrievEngine } from "@briev/dbriev-js";

const engine = await DbrievEngine.load("users.dbvl");
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
| 1.1 | Create `dbriev-engine` crate | HIGH |
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
| 4.1 | Create dbrievd binary | HIGH |
| 4.2 | Implement REST API | HIGH |
| 4.3 | Add CRUD endpoints | HIGH |
| 4.4 | Add query endpoint | HIGH |
| 4.5 | Add import/export endpoints | HIGH |
| 4.6 | Build dbriev CLI | HIGH |

### Phase 5: Cross-Briev Integration (Month 6)

| Task | Description | Priority |
|------|-------------|----------|
| 5.1 | Briev → DBriev FFI | HIGH |
| 5.2 | E-Briev register binding | HIGH |
| 5.3 | RBV reactive binding | MEDIUM |
| 5.4 | R-Briev SystemVerilog transpiler | HIGH |

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
briev-compiler/
├── lib/
│   ├── dbriev-engine/           # Core engine (new)
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
│   ├── dbriev-transpiler/       # Hardware transpiler (new)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── sv.rs
│   │
│   ├── dbriev-ffi/              # C FFI bindings (new)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   └── ffi/                     # Existing FFI (keep)
│
├── bin/
│   ├── dbrievd/                 # Server (new)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       └── routes/
│   │
│   └── briev/                   # Existing CLI (extend)
│
├── src/
│   └── dbriev/                  # Existing (keep, refactor to use lib)
│       ├── parser.rs
│       ├── ast.rs
│       ├── eval.rs
│       ├── alloc.rs
│       └── mod.rs
│
├── docs/
│   └── reference/
│       └── DBRIEV_SPEC.md       # Update with v1 spec
│
└── plans/
    └── active/
        └── DBRIEV_UNIVERSAL_DATA_ENGINE_PLAN.md  # This file
```

---

## 10. Success Criteria

| Criteria | Verification |
|----------|-------------|
| Parse `.dbv` files | `dbriev load config.dbv` succeeds |
| Parse `.dbvl` files | `dbriev load data.dbvl` succeeds |
| Parse `.dbvs` files | `dbriev schema show schema.dbvs` works |
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

- `docs/reference/DBRIEV_SPEC.md` - Language specification
- `docs/reference/BRIEV-DATALOG_RESEARCH.md` - Datalog research
- `plans/active/DBRIEV_IMPLEMENTATION_PLAN.md` - Previous plan (superseded)
- `docs/EMBEDDED_BRIEV_2.2_SPEC.md` - E-Briev specification

---

**End of Plan**