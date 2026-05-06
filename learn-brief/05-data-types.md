# Data Types: Collections & Structures

Brief provides powerful built-in data types with O(1) operations.

## 1. HashMap<K, V>

Hash-based key-value storage with O(1) lookup.

```brief
// Construction
let map: HashMap<String, Int> = new_map();

// Insert
map = map.insert("age", 42);
map = map.insert("count", 100);

// Lookup (returns Option<V>)
let age = map.get("age");
[age.is_some()] {
    let val = age.unwrap();
    println("Age: " + String(val));
};

// Check existence
[map.contains_key("age")] {
    println("Has age key");
};

// Remove
map = map.remove("count");

// Metadata
let len = map.len();
[map.is_empty()] {
    println("Map is empty");
};

// Iteration
let keys = map.keys();
let values = map.values();
let pairs = map.iter();  // List<(K, V)>
```

### Example: Word Counter

```brief
defn count_words(text: String) -> HashMap<String, Int> {
    let counts = new_map<String, Int>();
    let words = text.split(" ");
    
    let i: Int = 0;
    [i < words.len()] {
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
let len = set.len();
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
    [i < list.len()] {
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

## 3. Stack<T>

LIFO (Last-In-First-Out) structure.

```brief
// Construction
let stack: Stack<Int> = new_stack();

// Push
stack = stack.push(1);
stack = stack.push(2);
stack = stack.push(3);

// Pop (returns Option<(T, Stack<T>)>)
let result = stack.pop();
[result.is_some()] {
    let (value, new_stack) = result.unwrap();
    // value = 3, new_stack has [1, 2]
};

// Peek (returns Option<T>)
let top = stack.peek();

// Metadata
let len = stack.len();
[stack.is_empty()] {
    println("Stack is empty");
};

// Clear
stack = stack.clear();
```

### Example: Expression Evaluator

```brief
defn evaluate_rpn(expr: List<String>) -> Int {
    let stack: Stack<Int> = new_stack();
    
    let i: Int = 0;
    [i < expr.len()] {
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
let len = queue.len();
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
        [i < neighbors.len()] {
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
let len = sb.len();
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
    [i < rows.len()] {
        let row = rows[i];
        let j: Int = 0;
        [j < row.len()] {
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

## 6. Complete Example: Contact Manager

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

txn lookup(name: String) [true][true] {
    let contact = contacts.get(name);
    [contact.is_some()] {
        let c = contact.unwrap();
        println("Name: " + c.name);
        println("Phone: " + c.phone);
        println("Email: " + c.email);
    };
    [contact.is_none()] {
        println("Contact not found");
    };
    term;
};

txn list_all() [true][true] {
    let names = contacts.keys();
    let i: Int = 0;
    [i < names.len()] {
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

*Next: [06-string.md](06-string.md) - String manipulation and operations*
