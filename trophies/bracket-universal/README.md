## Universal Bracket Syntax: SIMD Protocol for All Types

**What**: Extended bracket syntax (`[]`) to work universally on every type.
Every value decomposes to visual `Char` fragments under bracket operations.
Added `@"pattern"` regex literals with DFA compilation at parse time.

**Why it matters**: Bracket operations are now a uniform SIMD protocol for
all data — Int, Float, Bool, Char, String, List, HashMap. `15561[;==5]`
returns `Int(161)` by decomposing the integer to chars, filtering, and
reconstructing. `@"[a-z]+"` compiles to a DFA at parse time for O(n)
runtime matching with zero allocation.

**How**: `decompose_atomic_to_chars()` converts Int/Float/Bool/Char to
`Vec<char>` of their visual representation. `reconstruct_from_chars()` parses
filtered chars back to the original type. `BracketOp::Mask` now handles
`Value::Bool`, `Value::Regex`, and `Value::String` — a string mask is
compiled to a DFA on the fly. Type-directed desugar: a bare string in
brackets on an atomic type (e.g., `15561["[15]"]`) becomes a per-element
regex filter.

**Before/After**: Previously, bracket ops only worked on List and String.
Now they decompose and reconstruct any type. The DFA compiler (`analysis/dfa.rs`)
was previously written but orphaned — now it is wired into every `@"..."`
literal evaluation.
