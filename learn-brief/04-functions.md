# Functions with Contracts

Functions (`defn`) are pure computations with contracts. Unlike transactions, they don't mutate state.

## 1. Basic Functions

```brief
defn add(a: Int, b: Int) -> Int {
    term a + b;
};

defn is_positive(n: Int) -> Bool {
    term n > 0;
};
```

**Parts:**
- `defn` - Keyword
- `add` - Function name
- `(a: Int, b: Int)` - Parameters with types
- `-> Int` - Return type (contracts are optional but recommended)
- `term a + b` - Return value

**Note:** Contract syntax has changed. Preconditions/postconditions like `[a >= 0]` are now warnings/errors if trivial. Functions can now omit contracts entirely.

## 2. Contracts (Optional but Recommended)

**Contracts are now optional but trivial ones like `[true][true]` produce warnings:**

```brief
// ✅ Works - no contracts needed
defn divide(a: Int, b: Int) -> Int {
    term a / b;
};

// ✅ Better - add contracts for verification
defn safe_divide(a: Int, b: Int) [b != 0][true] -> Int {
    term a / b;
};
```

## 3. Multiple Return Values

```brief
defn div_mod(a: Int, b: Int) [b != 0][quotient * b + remainder == a] -> (Int, Int) {
    term (a / b, a % b);
};

defn get_coords() -> (x: Int, y: Int) {
    term (10, 20);
};

// Usage
let (q, r) = div_mod(10, 3);  // q = 3, r = 1
let (_, remainder) = div_mod(42, 5);  // _ discards the quotient
let coords = get_coords();
let x = coords.0;
let y = coords.1;
```

**Note:** Tuple destructuring works with `_` as a discard placeholder.
For nested generics like `Ptr<Ptr<Int>>`, add a space: `Ptr<Ptr<Int> >`
to avoid the `>>` shift-right token.
```

## 4. Named Return Values

```brief
defn get_person() -> (name: String, age: Int) {
    term ("Alice", 30);
};

let person = get_person();
println(person.name);  // "Alice"
println(person.age);   // 30
```

## 5. Guards in Functions

```brief
defn abs(n: Int) [true][result >= 0] -> Int {
    [n < 0] {
        term -n;
    };
    term n;
};

defn max(a: Int, b: Int) [true][result == a || result == b] -> Int {
    [a >= b] {
        term a;
    };
    term b;
};

defn clamp(val: Int, min_val: Int, max_val: Int) 
    [min_val <= max_val]
    [result >= min_val && result <= max_val] 
    -> Int 
{
    [val < min_val] {
        term min_val;
    };
    [val > max_val] {
        term max_val;
    };
    term val;
};
```

## 6. Recursive Functions

```brief
defn factorial(n: Int) [n >= 0 && n <= 20][result >= 1] -> Int {
    [n == 0 || n == 1] {
        term 1;
    };
    term n * factorial(n - 1);
};

defn fibonacci(n: Int) [n >= 0][result >= 0] -> Int {
    [n == 0] {
        term 0;
    };
    [n == 1] {
        term 1;
    };
    term fibonacci(n - 1) + fibonacci(n - 2);
};

defn gcd(a: Int, b: Int) [a >= 0 && b >= 0][result >= 0] -> Int {
    [b == 0] {
        term a;
    };
    term gcd(b, a % b);
};
```

## 7. Functions with Generics

```brief
defn identity<T>(x: T) [true][result == x] -> T {
    term x;
};

defn swap<T, U>(a: T, b: U) [true][result.0 == b && result.1 == a] -> (U, T) {
    term (b, a);
};

defn first<T, U>(pair: (T, U)) [[true] -> T {
    term pair.0;
};

// Note: `[true][true]` is rejected. Use `[[true]` (post-only) or `[true]]` (pre-only).
// The `[[post]` and `[pre]]` sugar fill the omitted side as `[true]`.
```

## 8. Complete Example

```brief
// math_utils.bv

defn power(base: Int, exp: Int) 
    [exp >= 0]
    [result >= 1]
    -> Int 
{
    [exp == 0] {
        term 1;
    };
    [exp == 1] {
        term base;
    };
    let half = power(base, exp / 2);
    [exp % 2 == 0] {
        term half * half;
    };
    term base * half * half;
};

defn is_prime(n: Int) 
    [n >= 0]
    [[true]
    -> Bool 
{
    [n < 2] {
        term false;
    };
    [n == 2] {
        term true;
    };
    [n % 2 == 0] {
        term false;
    };
    
    let i: Int = 3;
    [i * i <= n] {
        [n % i == 0] {
            term false;
        };
        i = i + 2;
    };
    term true;
};

defn sum_range(start: Int, end: Int) 
    [start <= end]
    [result >= 0]
    -> Int 
{
    let n: Int = end - start + 1;
    term (start + end) * n / 2;
};

txn main() [[true] {
    let p = power(2, 10);  // 1024
    let prime = is_prime(17);  // true
    let sum = sum_range(1, 100);  // 5050
    term;
};
```

## Exercises

1. Write a `min` function that returns the smaller of two values
2. Implement `is_even` and `is_odd` with contracts
3. Create a `fibonacci_fast` function using tail recursion
4. Write a `quadratic_formula` function that returns both roots

---

*Next: [05-data-types.md](05-data-types.md) - HashMap, HashSet, Stack, Queue*
