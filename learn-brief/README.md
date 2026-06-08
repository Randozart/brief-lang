# Learn Brief - Complete Tutorial

**Version:** 0.16.0  
**Last Updated:** 2026-06-08  
**Status:** Complete ✅

---

## Learning Path

### Beginner (Start Here)

1. **[00-welcome.md](00-welcome.md)** - What is Brief?
   - Introduction to declarative programming
   - Key features overview
   - Quick start guide

2. **[00a-base-design.md](00a-base-design.md)** - The Brief Mindset
   - How to read Brief code
   - Symbol meanings and heuristics
   - The "feel" of the language

3. **[01-basics.md](01-basics.md)** - Variables, Types, Transactions
   - State declarations (`let`)
   - Basic types (Int, String, Bool, Char)
   - Transaction syntax
   - Guards and escape

3. **[02-contracts.md](02-contracts.md)** - Preconditions & Postconditions
   - Writing meaningful contracts
   - The `@` prior state operator
   - Contract verification
   - Common patterns

4. **[03-reactive.md](03-reactive.md)** - Reactive Transactions
   - The `rct` keyword
   - Termination verification
   - Reactive chains
   - Async reactive transactions

### Intermediate

5. **[04-functions.md](04-functions.md)** - Functions with Contracts
   - Function syntax (`defn`)
   - Non-trivial contracts (required!)
   - Multiple return values
   - Recursive functions
   - Generics

6. **[05-data-types.md](05-data-types.md)** - Collections & Structures
   - HashMap<K,V> (O(1) lookup)
   - HashSet<T> (O(1) membership)
   - Stack<T> (LIFO)
   - Queue<T> (FIFO)
   - StringBuilder (O(n) concatenation)

7. **[06-string.md](06-string.md)** - String Manipulation
   - Basic operations (len, concat, substr)
   - Search operations (contains, find)
   - Case conversion (to_upper, to_lower)
   - Trimming and padding
   - Split and join

8. **[07-ffi.md](07-ffi.md)** - Foreign Function Interface
   - FFI signatures (`frgn`)
   - Error handling
   - Metropolitan FFI (zero-copy)
   - Type mapping
   - Complete examples

### Advanced

9. **[08-examples.md](08-examples.md)** - Complete Examples
   - Counter application
   - Bank account system
   - Shopping cart
   - Todo list
   - Traffic light system
   - Producer-consumer pattern

10. **[09-patterns.md](09-patterns.md)** - Common Patterns
    - State machine pattern
    - Observer pattern
    - CQRS
    - Repository pattern
    - Builder pattern
    - Strategy pattern
    - Circuit breaker
    - Retry with backoff
    - Rate limiting
    - Caching

11. **[10-best-practices.md](10-best-practices.md)** - Best Practices
    - Performance tips
    - Code organization
    - Testing strategies
    - Debugging techniques
    - Security best practices
    - Common pitfalls

### Reference

13. **[13-projections.md](13-projections.md)** - Projections (`:>` and `<:`)
    - Metadata projections (Size, Bytes, Ptr, Alignment, Range)
    - Bit manipulation projections (Popcount, LeadingZeros, TrailingZeros, Absolute, BitReverse)
    - Reflection (Type, Ptr!, Keys, Values, Contains, Pop, Index, Get, Top, Front, Elements)
    - `<:` subtype projections (FILTER, MAP, SORT, GROUP, aggregates)
    - String match via `<:[...]`

---

## Quick Reference

### Syntax Cheat Sheet

```brief
// State
let counter: Int = 0;
const MAX_SIZE: Int = 100;

// Transaction (no parentheses if no params)
txn increment [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};

// Reactive transaction
rct txn auto_reset [counter >= 100][counter == 0] {
    &counter = 0;
    term;
};

// Function (no contracts required, but meaningful ones are recommended)
defn add(a: Int, b: Int) -> Int {
    term a + b;
};

// Guard
[x > 0] {
    &result = x * 2;
};

// FFI
frgn sqrt(x: Float) -> Result<Float, MathError>;
```

### Type System

| Type | Description | Example |
|------|-------------|---------|
| `Int` | 64-bit integer | `42` |
| `UInt` | Unsigned 64-bit | `42u` |
| `Float` | 32-bit float | `3.14` |
| `Bool` | Boolean | `true`, `false` |
| `String` | UTF-8 string | `"hello"` |
| `Char` | Unicode codepoint | `'a'` |
| `List<T>` | Dynamic array | `[1, 2, 3]` |
| `Option<T>` | Nullable | `Some(42)`, `None` |
| `Result<T,E>` | Error handling | `Ok(42)`, `Err(e)` |
| `Ptr<T>` | Verified pointer | `&x :> Ptr` |

### Collections

```brief
// HashMap
let map = new_map<String, Int>();
map = map.insert("key", 42);
let val = map.get("key");

// HashSet
let set = new_set<String>();
set = set.insert("item");
let has = set.contains("item");

// Stack
let stack = new_stack();
stack = stack.push(1);
let (val, stack) = stack.pop();

// Queue
let queue = new_queue();
queue = queue.enqueue("first");
let (val, queue) = queue.dequeue();
```

---

## Additional Resources

### Documentation
- [SPEC.md](../spec/SPEC.md) - Complete language specification
- [QUICK-REFERENCE.md](../spec/QUICK-REFERENCE.md) - Syntax cheat sheet
- [LANGUAGE-TUTORIAL.md](../spec/LANGUAGE-TUTORIAL.md) - Detailed tutorial
- [METROPOLITAN_FFI.md](../METROPOLITAN_FFI.md) - FFI guide
- [DATABRIEF_GUIDE.md](../DATABRIEF_GUIDE.md) - Configuration guide
- [OPTIMIZATIONS.md](../OPTIMIZATIONS.md) - Performance guide

### Examples
- [examples/](../examples/) - Example programs
- [tests/](../tests/) - Test cases
- [08-examples.md](08-examples.md) - Complete examples

### Tools
- Brief Compiler - `brief` command
- Language Server - `brief lsp`
- Syntax Highlighting - VS Code extension

---

## Getting Help

- **GitHub Issues:** Report bugs and request features
- **Discussions:** Ask questions and share ideas
- **Discord:** Chat with other Brief developers

---

## Next Steps

After completing this tutorial:

1. **Build a Project** - Apply what you've learned
2. **Read the Spec** - Deep dive into language details
3. **Explore Examples** - Study real-world code
4. **Contribute** - Help improve Brief
5. **Teach Others** - Share your knowledge

---

*Happy coding! 🚀*

*Last updated: 2026-06-08*  
*Version: Brief v0.16.0*
