# DBrief Language Specification

**Version:** v0.1.0 **Date:** 2026-05-05 **Status:** Draft (unstable) **Language Variants:** Config (.dbv), Database (.dbvl), Schema (.dbvs)

---

## 1. Introduction and Philosophy

DBrief (Data Brief) is a declarative, contract-enforced data language that combines the best of TOML, Prolog/Datalog, and SQL into a unified verifiable data layer. It treats data as logical propositions that can be queried, validated, and synchronized across systems.

### 1.1 Core Design Principles

1. **Register-Addressable Data**: Data lives at addresses (`@`), not keys. Aligns with hardware thinking.
2. **Verification Before Trust**: Local data is fully verifiable; remote data requires explicit error handling.
3. **Dual Mode**: Works as both config language (`.dbv`) and database language (`.dbvl`).
4. **Backend Agnostic**: Pluggable storage backends with unified query interface.
5. ** Brief-Native**: Queries feel like Brief state machine syntax, not foreign SQL.

### 1.2 Relationship to Brief

| Brief Variant | Purpose | DBrief Role |
|-------------|--------|-------------|
| `.bv` | Core state machines | Can query DBrief data via FFI |
| `.rbv` | Web views | Binds to DBrief for reactive data |
| `.ebv` | Embedded hardware | Reads DBrief config for registers |
| `.dbv` | Config files | Static configuration |
| `.dbvl` | Database files | Mutable data store |

---

## 2. Language Variants

### 2.1 File Types

| Extension | Purpose | Mutable | Line-Based |
|----------|---------|---------|-----------|
| `.dbv` | Configuration | No | No |
| `.dbvl` | Database | Yes | Yes |
| `.dbvs` | Schema definition | No | No |

### 2.2 Usage Patterns

```brief
// config.dbv - Static configuration
[network.settings]
port: UInt[16] = 8080;
timeout: UInt[32] = 5000;

check valid_port {
    port > 1024;
    port < 65535;
}
```

```brief
// data.dbvl - Mutable database (each line is a record)
@1 { "Alice", 30, "Engineer" }
@1 { "Bob", 25, "Designer" }
@2 { "admin_write", true }
```

```brief
// schema.dbvs - Register map definition
REGISTER @1: Vector[Person]
REGISTER @2: Vector[Permission]

STRUCT Person {
    name: String,
    age: UInt[8],
    role: String
}

STRUCT Permission {
    name: String,
    write: Bool
}
```

---

## 3. Register Addressing System

### 3.1 Address Syntax

```bnf
address ::= "@" (register_id | variable_ref | remote_spec)
variable_ref ::= identifier
register_id ::= natural_number | hex_address
hex_address ::= "0x" hex_digit+
remote_spec ::= protocol ":" location ["/" register_id]
protocol ::= "tcp" | "https" | "postgres" | "mongo" | "redis"
location ::= (ip_address | hostname) [":" port]
```

### 3.2 Schema Import

```brief
// Import a schema to resolve registers
IMPORT "./schema.dbvs"

// Now @1 resolves to Vector[Person] from schema.dbvs
@1->FILTER age > 25
```

### 3.3 Variable Binding

```brief
// Bind address to variable for reuse
LET USERS = @1
LET API = @https://api.example.com/v1

// Use variable
@USERS->COUNT

// Override variable
LET USERS = @2  // Switch to different register
```

### 3.4 Remote Addresses

```brief
// TCP/IP backend
LET DB = @tcp:192.168.1.100:5432/1
LET DB = @tcp:server.example.com:5432/main

// HTTP/REST API
LET API = @https://api.example.com/v1

// Pluggable backends
@postgres:localhost/db/users
@mongo:mongodb://localhost:27017/users
@redis:localhost:6379/session
```

### 3.5 Access Modes

```brief
// Hot lookup - direct remote query each access
@hot:tcp:server/1 | FILTER role == "admin"

// Cached - local copy with sync interval
@cached:tcp:server/1 | SYNC interval: 5s

// Lazy - load on first access, cache with TTL
@lazy:tcp:server/1 | CACHE ttl: 60s
```

### 3.6 Auto-Allocation (@auto)

The `@auto` keyword allows the compiler to automatically assign unclaimed addresses:

```brief
ALIAS heap_start: Addr = @auto   // Compiler picks free address
ALIAS stack_top: Addr = @auto    // Compiler picks free address

// In .dbv - any unclaimed address is available
REGISTER @auto: Vector[UInt[8], 4096]  // Auto-allocate 4KB
```

**Compiler behavior:**
- Scans claimed address ranges
- Finds first contiguous free block of requested size
- Reserves and marks as used
- Fails if insufficient space available

---

## 4. Type System

### 4.1 Primitive Types

| Type | Description | Range |
|------|------------|-------|
| `Bool` | Boolean | `true` / `false` |
| `Int[N]` | Signed N-bit integer | `-2^(N-1)` to `2^(N-1)-1` |
| `UInt[N]` | Unsigned N-bit integer | `0` to `2^N-1` |
| `Float` | 64-bit float | IEEE 754 |
| `String` | UTF-8 string | Variable length |
| `Data` | Raw bytes | Variable length |

### 4.2 Register-Specific Types

| Type | Description |
|------|-------------|
| `Addr` | Memory/register address |
| `RegOffset` | Offset from base address |
| `Vector[T]` | Dynamically-sized array |
| `Option[T]` | Optional value |
| `Result[T, E]` | Error-aware value |

### 4.3 Composite Types

```brief
STRUCT Person {
    name: String,
    age: UInt[8],
    occupation: String
}

ENUM Role {
    ADMIN,
    EDITOR,
    GUEST
}

TYPE User = {
    id: UInt[32],
    name: String,
    role: Role
}
```

---

## 5. Schema Definition

### 5.1 REGISTER Keyword

```bnf
register_def ::= "REGISTER" address ":" type ["check" contract]
```

### 5.2 Examples

```brief
// Simple register
REGISTER @1: Vector[Person]

// Register with contract
REGISTER @0xA000: {
    baud_rate: UInt[32],
    enabled: Bool
} CHECK [
    baud_rate <= 115200
]
```

### 5.4 ALIAS - Symbolic Name Binding

The ALIAS keyword provides **logical to physical address separation**, enabling the same Brief code to target different hardware by swapping the `.dbv` config file.

```bnf
alias_def ::= "ALIAS" identifier ":" type
alias_def ::= "ALIAS" identifier ":" type "=" address
alias_def ::= "ALIAS?" identifier ":" type "=" address  // Optional - fallback allowed
```

### 5.4.1 Schema Declaration (.dbvs)

In `.dbvs` files, ALIAS declares symbolic names without addresses:

```brief
// schema.dbvs - declares symbolic names (LOGICAL)
// No addresses assigned - resolved at compile time
ALIAS led_on: Bool
ALIAS counter: UInt[32]
ALIAS buffer: Vector[UInt[8], 1024]
ALIAS? optional_value: UInt[32]  // Optional - fallback allowed
```

### 5.4.2 Config Binding (.dbv)

In `.dbv` files, ALIAS binds symbolic names to physical addresses:

```brief
// KV260.dbv - bind to specific addresses (CONCRETE)
ALIAS led_on: Bool = @0xFF5E0000
ALIAS counter: UInt[32] = @0xFF5E0004
ALIAS buffer: Vector[UInt[8], 1024] = @0xA0000000

// Or auto-allocate (compiler picks free space)
ALIAS heap_start: Addr = @auto
ALIAS stack_top: Addr = @auto
```

### 5.4.3 Different Target (Same Logic)

Same `.ebv` code with a different `.dbv` for a different board:

```brief
// ZCU102.dbv - different board, different addresses
ALIAS led_on: Bool = @0xFC010000
ALIAS counter: UInt[32] = @0xFC010004
ALIAS buffer: Vector[UInt[8], 1024] = @0x80000000
```

### 5.4.4 Compile-Time Validation

The compiler validates:
1. **All required aliases bound**: Non-optional aliases must have addresses
2. **No conflicts**: Two aliases cannot overlap in address space
3. **Type compatibility**: Bound address type must match declared type
4. **Auto-allocation**: Unclaimed addresses are available for `@auto`

```brief
// Error if required alias missing
// Error: Alias 'led_on' not bound in config

// Error if address conflict
// Error: Alias 'counter' overlaps with 'buffer' at 0xFF5E0000

// Success if all required aliases bound
// Compiling ebv->kv260.dbv: OK
```

### 5.4.5 Optional Aliases

The `ALIAS?` syntax marks an alias as optional. If not bound in `.dbv`, the compiler:
- Uses a default/zero value at compile time
- Logs a warning but doesn't fail

```brief
// schema.dbvs
ALIAS? debug_led: Bool  // Optional

// KV260.dbv
// debug_led not bound - uses false

// ZCU102.dbv  
ALIAS debug_led: Bool = @0xFC010010  // Bound - uses this
```

---

## 6. Contracts on Data

### 6.1 CHECK Syntax

```bnf
check_def ::= "CHECK" "[" expression "]"
```

### 6.2 Examples

```brief
// Single constraint
CHECK age > 18

// Multiple constraints
CHECK [
    age > 0;
    age < 150;
    name .#Size > 0
]

// Cross-field validation
CHECK balance >= 0
CHECK [
    credit_limit > 0 -> balance <= credit_limit
]
```

### 6.3 Contract Modes

```brief
// compile: Verify at compile/build time (default for .dbv)
// runtime: Verify at access time (default for remote)
// observe: Log violations but allow access
// ignore: Skip all verification
```

---

## 7. Inference Rules (Prolog Layer)

### 7.1 RULE Syntax

```bnf
rule_def ::= "RULE" head ":"-" body
head ::= identifier "(" parameters ")"
body ::= conjunction ("," conjunction)*
conjunction ::= predicate | negated_predicate
negated_predicate ::= "NOT" predicate
```

### 7.2 Examples

```brief
// Basic rule
RULE can_write(U) :- user{ id: U, role: "admin" }
RULE can_write(U) :- user{ id: U, role: "editor" }

// With constraints
RULE adult(U) :- user{ id: U, age: A }, A >= 18
RULE verified(U) :- user{ id: U, verified: true }

// Recursive rule
RULE reachable(A, B) :- edge{ from: A, to: B }
RULE reachable(A, B) :- edge{ from: A, to: C }, reachable(C, B)
```

### 7.3 Query with Rules

```brief
// Query using rule - returns proofs
?can_write(1)

// With guard - logical filter
[@1->?can_write(user_id)] {
    user = @1->GET user_id
}
```

---

## 8. Query Syntax

### 8.1 Unified Query Operators

DBrief supports three complementary query styles that don't conflict with Brief keywords:

#### Style A: Arrow (->) Pipeline

```brief
@1->FILTER role == "admin"
@1->COUNT
@1->MAP name, age * 2
@1->SORT age DESC
@1->LIMIT 10
```

#### Style B: Bracket ([]) Filter

```brief
@1[role == "admin"]                    // Filter
@1[age > 30]                       // Filter
@1[*]                              // All
@1[?, role == "admin"]              // Logical query (returns proofs)
@1[age > 30, COUNT]              // Filter + aggregation
```

#### Style C: QUERY Keyword

```brief
QUERY @1 | FILTER role == "admin" | COUNT
QUERY @1 | WHERE age > 30 | MAP name | SORT
QUERY @1 | JOIN @2 ON user_id
QUERY @1 | GROUP BY role | AGG COUNT
```

### 8.2 Aggregations

| Operator | Returns | Description |
|----------|--------|------------|
| `COUNT` | UInt | Number of records |
| `SUM(field)` | Number | Sum of field |
| `AVG(field)` | Float | Average of field |
| `MIN(field)` | Number | Minimum |
| `MAX(field)` | Number | Maximum |
| `FIRST` | Record | First matching |
| `LAST` | Record | Last matching |

### 8.3 Transformations

| Operator | Description |
|----------|------------|
| `MAP expr` | Transform each record |
| `FILTER pred` | Keep matching records |
| `SORT field [ASC\|DESC]` | Order records |
| `LIMIT n` | Take first n records |
| `SKIP n` | Skip first n records |
| `UNIQUE` | Remove duplicates |

### 8.4 Joins

```brief
@1->JOIN @2 ON user_id
@1->LEFT_JOIN @2 ON user_id
@1->CROSS_JOIN @2
```

---

## 9. Transactions

### 9.1 Transaction Syntax

```bnf
txn_def ::= "TXN" identifier "(" parameters? ")" contract "{" body "}"
```

### 9.2 Examples

```brief
// Add record
TXN add_person(name, age, role) [true][added == true] {
    @1->PUSH { name, age, role }
    added = true
}

// Update record
TXN update_age(id, new_age) [
    @1->GET id != null
][age == new_age] {
    @1->SET id: { age: new_age }
}

// Delete record
TXN delete_user(id) [
    @1->GET id != null
][deleted == true] {
    @1->DELETE id
    deleted = true
}
```

### 9.3 Atomicity

```brief
TXN transfer(from, to, amount) [
    @1->GET from: balance >= amount
][
    from.balance == @from.balance - amount;
    to.balance == @to.balance + amount
] {
    @1->SET from: { balance: @from.balance - amount }
    @1->SET to: { balance: @to.balance + amount }
}
```

---

## 10. Topology and Integrity

### 10.1 FORALL/EXISTS Constraints

```brief
// Every post must have a valid author
CHECK FORALL p IN @posts: 
    EXISTS u IN @users WHERE p.author_id == u.id

// No orphaned records
CHECK FORALL u IN @users:
    u.role IN ["admin", "editor", "guest"]
```

### 10.2 Foreign Key Relationships

```brief
REGISTER @posts: Vector[Post]
REGISTER @users: Vector[User]

CHECK [
    FORALL p IN @posts: 
        EXISTS u IN @users WHERE p.author_id == u.id
]
```

---

## 11. Remote API Integration

### 11.1 Data Trust Model

| Source | Verification | Handling |
|--------|-------------|----------|
| Local (`.dbv`/`.dbvl`) | Full - SMT | Safe by default |
| Import (`.dbvs`) | Schema check | Trusted schema |
| Remote API | None initially | Result/Option required |

### 11.2 Explicit Error Handling

```brief
// FRN-style must handle errors
FRN DEFN fetch_user(id: UInt[32]) -> RESULT[User, ApiError]

// Usage with Result unwrap
TXN load_user(id) [true][user != null] {
    RESULT = fetch_user(id) !  // ! unwraps Result
    user = RESULT.ok
    [RESULT.err] term FETCH_FAILED
}
```

### 11.3 Standard Library API Handler

```brief
ENUM ApiBackend {
    TCP,
    HTTP_REST,
    GRAPHQL,
    GRPC,
    WEB_SOCKET
}

// Generic fetch with backend type
DEFN api_fetch(
    backend: ApiBackend, 
    endpoint: String, 
    params: Vector[(String, String)]
) -> RESULT[Data, ApiError]

// Convenience overloads
DEFN http_get(url: String) -> RESULT[Data, HttpError]
DEFN tcp_query(addr: Addr, query: String) -> RESULT[Data, DbError]
```

---

## 12. Web Integration

### 12.1 RBV View Binding

```brief
// In .rbv file
IMPORT "./userdb.dbvs"

STATE users: Vector[User] = @users->ALL

RCTV txn refresh_users [true][users == @users->ALL] {
    users = @users->ALL
}

// View uses users state
RENDER user_list {
    DIV users {
        FOR u IN users {
            DIV user { u.name }
        }
    }
}
```

### 12.2 REST Endpoint Generation

```brief
// Auto-generate REST from DBrief schema
REST @users {
    GET /users           // -> @users->ALL
    GET /users/:id       // -> @users->GET id
    POST /users          // -> add_user txn
    PUT /users/:id       // -> update_user txn
    DELETE /users/:id    // -> delete_user txn
}
```

---

## 13. Performance and Optimization

### 13.1 Index Strategies

```brief
// Hash index on field
INDEX @1 ON name HASH

// B-tree index on range queries
INDEX @1 ON age BTree

// Composite index
INDEX @1 ON (role, department) COMPOSITE
```

### 13.2 Storage Layouts

| Layout | Best For | Access Pattern |
|--------|---------|-------------|
| Row-oriented (`.dbv`) | Writes, config | Random access |
| Column-oriented (`.dbvl`) | Aggregations | Scan-heavy |

### 13.3 Query Planning

```brief
// Hint for query optimizer
@1->FILTER role == "admin" | USE INDEX role_idx

// Materialized view
VIEW admin_list = @1->FILTER role == "admin"
```

---

## 14. Pluggable Backends

### 14.1 Backend Selection

```brief
// Embeddable
BACKEND sled

// In-memory (hot paths, testing)
BACKEND memory

// SQL-backed
BACKEND sqlite

// Remote server
BACKEND remote
```

### 14.2 Configuration

```brief
// Sled (embedded KV)
BACKEND sled "./data.db"

// SQLite
BACKEND sqlite "./data.db"

// In-memory
BACKEND memory
```

### 14.3 ACID Levels

| Level | Guarantees | Performance |
|-------|-----------|------------|
| `atomic` | All ACID | Medium |
| `eventual` | Eventual consistency | High |
| `none` | No guarantees | Highest |

---

## 15. Verification Modes

### 15.1 Compile-Time Verification (SMT)

```brief
// Verify contracts at compile time
VERIFY compile

// .dbv files always verify at compile time
```

### 15.2 Runtime Verification

```brief
// Verify at access time
VERIFY runtime

// For remote data, always runtime
```

### 15.3 Observe Mode

```brief
// Log violations, allow access
VERIFY observe
```

### 15.4 Ignore Mode

```brief
// Skip verification
VERIFY ignore
```

---

## 16. Output Targets

### 16.1 SystemVerilog (Hardware)

```brief
// Generate SV package
TARGET systemverilog "./kv260_pkg.sv"

// Output:
// package kv260_regs;
//   logic [31:0] CRL_APB = 32'hFF5E0000;
//   logic [31:0] RESERVED = 32'hFF5E0238;
// endpackage
```

### 16.2 Rust

```brief
TARGET rust "./config.rs"

// Output:
// pub const CRL_APB: u32 = 0xFF5E0000;
// pub const RESERVED: u32 = 0xFF5E0238;
```

### 16.3 C Headers

```brief
TARGET c "./config.h"

// Output:
// #define CRL_APB 0xFF5E0000
// #define RESERVED 0xFF5E0238
```

---

## 17. Grammar Summary

```bnf
program ::= (import | registration | alias_def | struct_def | enum_def | rule_def | query | txn)*

import ::= "IMPORT" string_literal ("AS" identifier)?

registration ::= "REGISTER" address ":" type ["CHECK" contract]

alias_def ::= "ALIAS" identifier ":" type ("=" address)?
alias_def ::= "ALIAS?" identifier ":" type ("=" address)?

struct_def ::= "STRUCT" identifier "{" field* "}"
field ::= identifier ":" type ";"

enum_def ::= "ENUM" identifier "{" variant ("," variant)* "}"

rule_def ::= "RULE" head ":"-" body

query ::= address "->" operator
        | address "[" expression "]"
        | "QUERY" address "|" pipeline

txn ::= "TXN" identifier "(" params? ")" contract "{" body "}"

address ::= "@" (number | "auto" | identifier | remote_spec)

type ::= primitive_type 
       | "Vector" "[" type "]"
       | "Option" "[" type "]"
       | "Result" "[" type "," type "]"
       | identifier
```

---

## 18. Migration Notes

### From TOML
- Replace `[section]` with `REGISTER @N:`
- Replace `key = value` with struct fields
- Add `CHECK` contracts for validation

### From JSON
- Replace `{ "key": value }` with `{ field: value }`
- Replace `array.map()` with `@addr->MAP`
- Replace `array.filter()` with `@addr->FILTER`

### From MongoDB
- Replace `.find()` with `@addr->FILTER`
- Replace aggregations with pipeline syntax
- Add contracts for schema enforcement

---

## 9. Future Considerations

- [ ] GraphQL integration
- [ ] Streaming queries (continuous)
- [ ] Multi-master replication
- [ ] Time-travel queries
- [ ] Blockchain anchoring