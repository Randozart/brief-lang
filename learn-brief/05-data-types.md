# Data Types: Collections & Structures

Brief provides powerful built-in data types with O(1) operations.

## 1. HashMap<K, V>

Hash-based key-value storage with O(1) lookup. The current `std/collections.bv`
HashMap is a **flat open-addressing** obj (keys/vals Ptr arrays) constructed via
`op Init` and mutated through method calls:

```brief
import { HashMap } from "std/collections.bv";

// Construction via op Init
let map: HashMap<Int, Int> = 0;

txn fill [true][map.count > 0] {
    map.insert(1, 42);
    map.insert(2, 100);
    term;
};

// Lookup — the member returns the value for the key's hash slot
defn get_age(m: HashMap<Int, Int>) -> Int {
    term m.get(1);
};
```

### Example: Word Counter

```brief
defn count_words(text: String) -> HashMap<String, Int> {
    let counts = new_map<String, Int>();
    let words = text.split(" ");
    
    let i: Int = 0;
    [i < words .^Len] {
        let word = words[i];
        let current = counts.get(word);
        [current.is_some()] {
            counts = counts.insert(word, current.unwrap() + 1);
        };
        [current.is_none()] {
            counts = counts.insert(word, 1);
        };
        &i = i + 1;
    };
    
    term counts;
};
```

## 2. HashSet<T>

Hash-based set with O(1) membership testing.

```brief
// Construction
let set: HashSet<String> = new_set();

// Insert
set = set.insert("apple");
set = set.insert("banana");
set = set.insert("cherry");

// Check membership
[set.contains("apple")] {
    println("Has apple");
};

// Remove
set = set.remove("banana");

// Metadata
let len = set .^Len;
[set.is_empty()] {
    println("Set is empty");
};

// Iteration
let items = set.iter();
```

### Example: Unique Items

```brief
defn unique_items(list: List<String>) -> List<String> {
    let seen = new_set<String>();
    let result: List<String> = [];
    
    let i: Int = 0;
    [i < list .^Len] {
        let item = list[i];
        [!seen.contains(item)] {
            seen = seen.insert(item);
            result = result.append(item);
        };
        &i = i + 1;
    };
    
    term result;
};
```

## 3. Stack<T, N>

LIFO (Last-In-First-Out) structure. The current collections (`std/collections.bv`)
are **obj instances** driven by the `<-` operators: `op InsertAt` (push),
`op ExtractFrom` (pop), `op Init` (`let s: Stack<T, N> = 0` constructs).

```brief
import { Stack } from "std/collections.bv";

// Construction via op Init — allocates the instance + its data buffer
let stack: Stack<Int, 64> = 0;

// Push — `<-` dispatches op InsertAt → the self-bound push member
stack <- 1;
stack <- 2;
stack <- 3;

// Pop (discard the value) — `<-` dispatches op ExtractFrom → pop
<- stack;

// Destructive discard — pop and destroy the source's backing
~<- stack;

// Destructive extract into a binding (2026-08-01, Phase 3)
let v2: Int = 0;
v2 ~<- stack;

// Pop into a binding reads the member's returned value
let v: Int = stack.pop();
```

`obj Stack<T, N> { data: T[N]; len: Int; ... }` — a fixed-size, array-backed
stack (no heap in the hot path). `List<T>` is the heap-backed growable form
(`inner: ListBuffer<T>` with a `Malloc#`-allocated buffer); `RingBuffer<T>`
is the fixed-size circular buffer.

### Example: Expression Evaluator

```brief
defn evaluate_rpn(expr: List<String>) -> Int {
    let stack: Stack<Int> = new_stack();
    
    let i: Int = 0;
    [i < expr .^Len] {
        let token = expr[i];
        [token == "+"] {
            let (b, s) = stack.pop().unwrap();
            let (a, s2) = s.pop().unwrap();
            &stack = s2.push(a + b);
        };
        [token == "*"] {
            let (b, s) = stack.pop().unwrap();
            let (a, s2) = s.pop().unwrap();
            &stack = s2.push(a * b);
        };
        [!is_operator(token)] {
            let num = String(token).to_int();
            &stack = stack.push(num);
        };
        &i = i + 1;
    };
    
    term stack.pop().unwrap().0;
};
```

## 4. Queue<T>

FIFO (First-In-First-Out) structure.

```brief
// Construction
let queue: Queue<String> = new_queue();

// Enqueue
queue = queue.enqueue("first");
queue = queue.enqueue("second");
queue = queue.enqueue("third");

// Dequeue (returns Option<(T, Queue<T>)>)
let result = queue.dequeue();
[result.is_some()] {
    let (value, new_queue) = result.unwrap();
    // value = "first", new_queue has ["second", "third"]
};

// Front (returns Option<T>)
let front = queue.front();

// Metadata
let len = queue .^Len;
[queue.is_empty()] {
    println("Queue is empty");
};

// Clear
queue = queue.clear();
```

### Example: BFS Traversal

```brief
defn bfs(start: Node) -> List<Node> {
    let visited = new_set<Node>();
    let queue: Queue<Node> = new_queue();
    let result: List<Node> = [];
    
    queue = queue.enqueue(start);
    visited = visited.insert(start);
    
    [!queue.is_empty()] {
        let (node, q) = queue.dequeue().unwrap();
        &queue = q;
        result = result.append(node);
        
        let neighbors = node.get_neighbors();
        let i: Int = 0;
        [i < neighbors .^Len] {
            let neighbor = neighbors[i];
            [!visited.contains(neighbor)] {
                visited = visited.insert(neighbor);
                queue = queue.enqueue(neighbor);
            };
            &i = i + 1;
        };
    };
    
    term result;
};
```

## 5. StringBuilder

Efficient string concatenation (O(1) amortized append).

```brief
// Construction
let sb = new_builder();

// Append
sb = sb.append_char('H');
sb = sb.append_char('e');
sb = sb.append_str("llo");
sb = sb.append_int(42);
sb = sb.append_bool(true);
sb = sb.append_float(3.14);

// Convert to String
let result = sb.to_string();  // "Hello42true3.14"

// Metadata
let len = sb .^Len;
[sb.is_empty()] {
    println("Builder is empty");
};

// Clear
sb = sb.clear();
```

### Example: CSV Builder

```brief
defn build_csv(rows: List<List<String>>) -> String {
    let sb = new_builder();
    
    let i: Int = 0;
    [i < rows .^Len] {
        let row = rows[i];
        let j: Int = 0;
        [j < row .^Len] {
            [j > 0] {
                sb = sb.append_char(',');
            };
            sb = sb.append_str(row[j]);
            &j = j + 1;
        };
        sb = sb.append_char('\n');
        &i = i + 1;
    };
    
    term sb.to_string();
};
```

## 6. Structs: Pure Data Containers

`struct` declares pure data with fixed layout, C-compatible, no methods or contracts:

```brief
struct Point {
    x: Int;
    y: Int;
};

struct Contact {
    name: String;
    phone: String;
    email: String;
};

let p = Point { x: 10, y: 20 };
let name = p.name;  // field access
```

### Fixed-Size Arrays: `Type[N]` and Slice Views

Arrays of compile-time-known size are declared inline. Works for any type:

```brief
struct VMStack {
    data: Int[1024];    // [1024 x i64] in LLVM IR
    len: Int;
};

struct Cache {
    cells: Float[9];    // fixed-size, auto-vectorized
};
```

`Int[1024]` → `[1024 x i64]`, `Frame[256]` → `[256 x %Frame]`.
Bounds proven by contract: `[i >= 0 && i < stack .^Len]`.

#### Array Slices: `arr[start:end:stride]`

A slice is a **zero-copy view** into an existing array:

```brief
arr[:]         // Full view
arr[4:]        // From index 4 to end
arr[:8]        // From start to index 8
arr[2:8]       // Range [2, 8), stride 1
arr[2:8:2]     // Every other element
arr[i:j:k]     // Dynamic bounds
```

All components are optional: start defaults to 0, end to array length, stride to 1.

#### SIMD Operations

Element-wise arithmetic on array and slice types:

```brief
let a: Int[4] = ...;
let b: Int[4] = ...;
let sum = a + b;       // <4 x i64> vector add
let doubled = a * 2;   // Scalar broadcast
```

#### View Casts

The `as` operator produces zero-copy views between compatible array types:

```brief
let raw: Int[1024];
let bytes = raw as Byte[8192];  // type-punned: same bytes, different type
let evens = raw[0:1024:2] as Int[512];  // strided: slice recast as sized array
```

**Type-punned view:** `N * sizeof(T) == M * sizeof(U)` enforced at compile time.
Emits LLVM `bitcast`. **Strided view:** slice bounds compute element count,
validated against target size. Both are zero-copy.

#### Stdlib, Not Magic

`map`, `filter`, `fold` are regular txn functions in `lib/std/array.bv`:

```brief
txn array_map<T, U>(arr: Vector<T, N>, f: T -> U, i: Int)
    -> Vector<U, N>
    [i < N][i == N]
{
    result[i] = f(arr[i]);
    i = i + 1;
    term result;
};
```

The LLVM auto-vectorizer recognizes the `[i < N]` convergence contract and
vectorizes the load-apply-store loop without compiler magic.

### `type` vs `struct` vs `obj`

| Keyword | Purpose | Example |
|---------|---------|---------|
| `type` | Protocols, operator bindings, type system extensibility | `type Int: #Int { op Add(#Int); };` |
| `struct` | Pure data, fixed layout, C-compatible, no methods | `struct Point { x: Int; y: Int; };` |
| `obj` | Full-featured types with methods, contracts, generics | `obj Channel<T> { ... };` |

`type { field: T }` patterns are being migrated to `struct { field: T }`.

## 8. Complete Example: Contact Manager

```brief
// contacts.bv

struct Contact {
    name: String,
    phone: String,
    email: String
};

let contacts: HashMap<String, Contact> = new_map();

txn add_contact(name: String, phone: String, email: String)
    [!contacts.contains_key(name)]
    [contacts.contains_key(name)]
{
    let contact = Contact {
        name: name,
        phone: phone,
        email: email
    };
    &contacts = contacts.insert(name, contact);
    term;
};

txn remove_contact(name: String)
    [contacts.contains_key(name)]
    [!contacts.contains_key(name)]
{
    &contacts = contacts.remove(name);
    term;
};

txn lookup(name: String) [name != ""][contacts == @contacts] {
    let contact = contacts.get(name);
    when contact.is_some() {
        let c = contact.unwrap();
        println("Name: " + c.name);
        println("Phone: " + c.phone);
        println("Email: " + c.email);
    };
    when contact.is_none() {
        println("Contact not found");
    };
    term;
};

txn list_all() [true][contacts == @contacts] {
    let names = contacts.keys();
    let i: Int = 0;
    when i < names .^Len {
        println(names[i]);
        &i = i + 1;
    };
    term;
};
```

## Exercises

1. Implement a cache using HashMap with LRU eviction
2. Create a palindrome checker using Stack
3. Build a task scheduler using Queue with priorities
4. Implement a simple template engine using StringBuilder

---

## 9. Vectors: Multidimensional Arrays

Vectors are fixed-size, contiguous memory arrays optimized for hardware and SIMD operations.

### Declaration

```brief
// 1D vector
let vec: Vector<Int, 100>;

// 2D matrix
let mat: Vector<Int, 10, 20>;

// 3D tensor
let tensor: Vector<Float, 3, 32, 32>;

// Named dimensions (for readability)
let persons: Vector<Person, width:50, height:50, depth:40, time:10>;
```

### Syntax

- `Vector<T, dim1, dim2, ...>` - First argument is the element type, rest are dimensions
- Dimensions can be **anonymous** (just numbers) or **named** (`name:size`)
- Names are purely syntactic - they don't affect memory layout
- Total elements = product of all dimensions

### Accessing Elements

```brief
let mat: Vector<Int, 10, 20>;

// 2D access
let x = mat[5, 10];

// Slicing - get a row
let row: Vector<Int, 20> = mat[5, :];

// Range slicing
let sub: Vector<Int, 5, 5> = mat[0..5, 0..5];

// Named dimension slicing
let persons: Vector<Person, width:50, height:50, time:10>;
let frame: Vector<Person, 50, 50> = persons[time:5];
let slice: Vector<Person, 50> = persons[time:5, width:10];
```

### Slicing Syntax

Brief supports powerful slicing with commas for multiple dimensions:

```brief
let mat: Vector<Int, 10, 20>;

// Single index per dimension
mat[5, 10]           // Returns element at [5][10]

// Range slicing
mat[0..5, 0..10]     // Returns Vector<Int, 5, 10>
mat[5.., ..10]       // From 5 to end, from 0 to 10

// Striding
mat[::2, ::4]        // Every 2nd row, every 4th column
mat[0..10:2, ::3]    // Range with stride

// Named dimensions
persons[time:5, width:0..10]
persons[time::2, width:5]  // Every 2nd time step, at width=5
```

### Slicing Syntax

Brief supports powerful slicing with commas for multiple dimensions:

```brief
let mat: Vector<Int, 10, 20>;

// Single index per dimension
mat[5, 10]           // Returns element at [5][10]

// Range slicing
mat[0..5, 0..10]     // Returns Vector<Int, 5, 10>
mat[5.., ..10]       // From 5 to end, from 0 to 10

// Striding
mat[::2, ::4]        // Every 2nd row, every 4th column
mat[0..10:2, ::3]    // Range with stride

// Named dimensions
persons[time:5, width:0..10]
persons[time::2, width:5]  // Every 2nd time step, at width=5
```

### Filtering with Semicolon

The semicolon separates coordinates from filter conditions:

```brief
let persons: Vector<Person, 50, 50>;

// Filter: all persons where age > 18
persons[: age > 18].adult = true;

// Slice + filter: at row 5, where age > 18
persons[5, ; age > 18].processed = true;

// Range + stride + filter
persons[0..50:2; city == "NYC"].region = "East";
```

### Why Vectors Instead of Lists

| Feature | List<T> | Vector<T, dims> |
|---------|---------|--------------|
| Size | Dynamic | Fixed at compile time |
| Memory | Heap-allocated | Contiguous buffer |
| Access | O(n) scan | O(1) direct |
| Hardware | Limited | Full SIMD support |
| Use case | Runtime data | Buffers, tensors |

### Example: Image Processing

```brief
struct Pixel {
    r: UInt,
    g: UInt,
    b: UInt
}

let frame: Vector<Pixel, width:1920, height:1080>;

// Brighten all red pixels > 200
frame[: r > 200].r = 255;

// Process every 4th pixel
frame[width::4, height::4].r = frame[width::4, height::4].r / 2;
```

---

## 10. Pointer Types (`Ptr<T>`)

`Ptr<T>` is a verified pointer whose safety is proven at compile time.
Creation is via the **address-of** operator `&` — the compiler tracks
provenance, so there is no way to forge a `Ptr<T>` without the compiler
knowing its origin.

### Creating Pointers

```brief
// Verified pointer (compiler tracks bounds, guarantees non-null)
let p: Ptr<Int> = &x;

// From a collection's first element
let list_ptr: Ptr<Int> = &my_list[0];

// Reflection form (same meaning as &x)
let p2: Ptr<Int> = x.^Ptr;
```

### Dereferencing

Use bracket indexing — `ptr[i]` — or the `*` dereference operator:

```brief
let val: Int = p[0];          // Read element 0 — bounds-checked at compile time
p[0] = 42;                    // Write element 0 — bounds-checked
let first: Int = *p;          // Dereference — first element
```

The compiler emits the same raw `load`/`store` instructions as C, but only
after proving the access is within bounds.

### Safety Guarantees

| Property | Guaranteed by |
|----------|---------------|
| Bounds | `i * sizeof(T) < ptr.^^Bytes` is proven by the SMT solver |
| Non-null | `Ptr<T>` from `&x` or `&list[0]` is always valid |
| Alignment | Address is always aligned to `T.^^Alignment` |
| No use-after-free | Brief has no `free` — global state lives forever |

### Standard Library

`std/ptr.bv` provides convenient wrappers with explicit contracts:

```brief
import { read_i64, write_i64, copy, address } from "std/ptr.bv";

// Safe read — precondition: i >= 0 && (i+1)*8 <= p .^^Bytes
let v = read_i64(p, 0);

// Safe write — same precondition
write_i64(p, 0, 99);

// Block copy — precondition: non-overlapping ranges → @llvm.memcpy
copy(dest, src, count);

// Get raw address
let addr = address(p);
```

Every function has a contract that the `PointerVerifier` pass checks at
compile time. If the caller cannot prove the precondition, compilation fails
with a `ProofError`.

---

## 11. Type/Metadata Checks: `is`, `from`, `like`

Brief provides three infix operators for inspecting types and structure at runtime.

### `is` — Type or Variant Check

```brief
let x: Int = 42;
let is_int = x is Int;    // → true

let y: Option[Int] = some(42);
let is_some = y is some;  // → true
let is_none = y is none;  // → false
```

The RHS of `is` can be a type name (`Int`, `String`) or a variant keyword (`some`, `none`, `ok`, `err`).

### `from` — Derivation Check

```brief
struct Foo { x: Int; }
struct Bar : Foo { y: Int; }

let obj = Bar { x: 1, y: 2 };
let is_from_foo = obj from Foo;   // → true
```

Checks whether the value's type is or derives from the target type.

### `like` — Structural Equality

```brief
42 like 42             // → true
[1, 2] like [1, 2]     // → true (recursive comparison)
"hi" like "hi"         // → true
42 like 1              // → false
```

Compares structural layout, not nominal type. Two structs with different names
but identical fields can be `like` each other.

### Precedence

```brief
!x is Some      → !(x is Some)
x is Some == true → (x is Some) == true
```

Binds tighter than `==`/`!=` but looser than unary `!`.

---

*Next: [06-string.md](06-string.md) - String manipulation and operations*
