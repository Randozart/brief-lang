# Getting Started with Brief

**Version:** 1.0  
**Purpose:** Your first steps learning Brief programming  

---

## Table of Contents

1. [What is Brief?](#what-is-brief)
2. [Prerequisites](#prerequisites)
3. [Installing Brief](#installing-brief)
4. [Your First Project: Hello World](#your-first-project-hello-world)
5. [Understanding What You Wrote](#understanding-what-you-wrote)
6. [Your Second Project: A Counter](#your-second-project-a-counter)
7. [Key Concepts in Brief](#key-concepts-in-brief)
8. [Where to Go Next](#where-to-go-next)

---

## What is Brief?

Brief is a programming language designed to help you write **bug-free code** from the start.

Most programming languages let you write anything, even things that don't make sense, and then you spend hours debugging later. Brief takes a different approach: **you write a contract first, then write the code**.

Think of it like writing a recipe with quality checks built in. Before you cook, you specify what ingredients you need and what the finished dish should look like. If something goes wrong during cooking, you know immediately because the contract wasn't met.

### The Key Idea: Contracts

A **contract** in Brief has two parts:

1. **Precondition** - What must be true before your code runs
2. **Postcondition** - What must be true after your code runs

If the preconditions aren't met, Brief won't let the code run. If the postconditions aren't met, Brief tells you something went wrong. This catches bugs at the moment they happen, not hours later when you're trying to figure out why your program crashed.

### What Can You Build with Brief?

Brief is designed to work in multiple contexts:

- **Web applications** - Brief compiles to WebAssembly, so it runs in browsers
- **Backend services** - Can handle business logic with contracts ensuring correctness
- **FFI libraries** - Brief can call functions from other languages (Rust, C, Python, JavaScript, Go, and more)

### Why Learn Brief?

- **Catch bugs early** - Contracts tell you immediately when something goes wrong
- **Self-documenting code** - Contracts explain what each piece of code expects and guarantees
- **Symbolic verification** - Brief can mathematically prove your contracts are achievable
- **Simple syntax** - No complex type system to learn upfront

---

## Prerequisites

Before learning Brief, you should have:

### Basic Programming Knowledge

- Understanding of what variables are (storing values like numbers or text)
- Understanding of functions (reusable pieces of code)
- Basic understanding of programming concepts like "if statements" and "loops"

If you have experience in any programming language—JavaScript, Python, C, Rust, or even BASIC—you have enough background for Brief.

### Software You Need

1. **A text editor** - We recommend [Visual Studio Code](https://code.visualstudio.com/) because it's free and works well with Brief
2. **A terminal/command line** - Brief runs from the command line. On Windows, this is PowerShell or Command Prompt. On Mac/Linux, it's called Terminal.
3. **Rust** (for building Brief from source) - Installation instructions below

### Understanding Your Terminal

You'll need to know a few basic commands:

| Command | What it does |
|---------|--------------|
| `cd folder` | Change into a folder (directory) |
| `ls` | List files in current folder |
| `mkdir folder` | Create a new folder |
| `pwd` | Print current folder path |

Don't worry if these are new to you—they're straightforward and we'll explain them as we use them.

---

## Installing Brief

Brief is currently distributed as source code that you compile yourself. This sounds intimidating but is actually quite simple.

### Step 1: Install Rust

Brief is written in Rust, so you need Rust installed first.

1. Go to [https://rustup.rs](https://rustup.rs)
2. Follow the instructions (it mostly just involves running one command)
3. Verify Rust installed by opening a new terminal and typing:

```bash
rustc --version
```

You should see something like `rustc 1.75.0` (version numbers change over time).

### Step 2: Build Brief

Open your terminal and run these commands:

```bash
# Clone the Brief repository (or navigate to where you downloaded it)
git clone https://github.com/your-username/brief-compiler.git
cd brief-compiler

# Build Brief in release mode (this takes a few minutes the first time)
cargo build --release
```

The `--release` flag makes the compiled program run faster. The first build might take 5-10 minutes. Future builds will be faster.

### Step 3: Add Brief to Your PATH

After building, you can either:

**Option A: Add to PATH temporarily (for one terminal session)**

```bash
export PATH="$PATH:/path/to/brief-compiler/target/release"
```

**Option B: Install system-wide**

```bash
# From the brief-compiler directory
./target/release/brief install
```

This installs Brief to `~/.local/bin/brief`.

### Step 4: Verify Installation

```bash
brief --help
```

You should see:

```
Brief Compiler v0.1.0

Usage: brief <command> [options] [file]

Commands:
  check <file>     Type check without execution (fast)
  build <file>     Full compilation
  run <file>       Compile, build WASM, serve, and open browser
  ...
```

If you see this, you're ready to start programming!

### Troubleshooting

**"brief: command not found"**

- Make sure the PATH includes where Brief is installed (`~/.local/bin` or the target directory)
- Try opening a new terminal window

**"cargo: command not found"**

- Rust isn't installed correctly. Run the installer again from [rustup.rs](https://rustup.rs)

**Build fails with errors**

- Make sure you have the latest Rust: `rustup update`
- Check the error messages—they usually tell you what's wrong

---

## Your First Project: Hello World

Let's create your first Brief program. We'll make a simple program that says "Hello, World!"

### Step 1: Create a New Project

```bash
# Create a new folder for your project
mkdir hello-brief
cd hello-brief

# Initialize with Brief
brief init
```

This creates a basic project structure:

```
hello-brief/
├── src/
│   └── main.bv      # Your Brief code lives here
├── Cargo.toml        # Rust project settings (for compiling to WebAssembly)
└── brief.toml       # Brief project settings
```

### Step 2: Open and Edit main.bv

Open `src/main.bv` in your text editor. You'll see some default content. Replace everything with this:

```brief
txn greet [true][result == "Hello, World!"] {
    term "Hello, World!";
};
```

### Step 3: Run Your Program

Back in your terminal, from the `hello-brief` folder:

```bash
brief run src/main.bv
```

You should see your browser open (or a message saying the server started). Your "Hello, World!" program is running!

### Congratulations!

You've just written and run your first Brief program. Let's break down exactly what you wrote.

---

## Understanding What You Wrote

Here's your first program again, with each part labeled:

```brief
txn greet         // 1. Transaction named "greet"
[                 // 2. Contract starts with bracket
  true            // 3. Precondition: always runs (no requirements)
][                // 4. Separator
  result == "Hello, World!"  // 5. Postcondition: result must equal this
]                 // 6. Contract ends with bracket
{                 // 7. Body starts
    term "Hello, World!";  // 8. Terminate with this value
}                 // 9. Body ends
```

### Breaking It Down

**1. `txn greet`**

The word `txn` is short for "transaction." A transaction is a unit of work in Brief—it's how you define something your program can *do*.

The name `greet` is what you call it. You can use this name to trigger this work from elsewhere in your program.

**2-6. The Contract `[true][result == "Hello, World!"]`**

The contract is where Brief's magic happens. It has two parts:

- **Precondition** (`true`) - What must be true *before* this code runs
- **Postcondition** (`result == "Hello, World!"`) - What must be true *after* this code runs

The preconditions are in the first set of brackets `[ ]`. The postconditions are in the second set.

**3. Precondition: `true`**

`true` means "no requirements." This transaction can run under any circumstances. We don't need anything specific to be true before it runs.

**5. Postcondition: `result == "Hello, World!"`**

After the transaction runs, the result should equal `"Hello, World!"`. The variable `result` is special in Brief—it holds the value that the transaction produces.

**8. `term "Hello, World!";`**

The keyword `term` means "terminate and return this value." It ends the transaction and produces the output value.

**Understanding `result`**

When you see `result == "Hello, World!"`, read it as: "The output should equal Hello World."

When Brief verifies your program, it checks that `result` will indeed equal `"Hello, World!"` when the transaction completes. If it can't prove this mathematically, it will give you an error.

---

## Your Second Project: A Counter

Let's build something more interesting: a counter that can go up, down, and reset to zero. This will teach you about **state variables**—values that persist between transactions.

### Step 1: Create the Project

```bash
cd ..              # Go back to parent folder
mkdir simple-counter
cd simple-counter
brief init
```

### Step 2: Write the Counter Code

Open `src/main.bv` and replace the content with:

```brief
// A simple counter that tracks a number
// State variable: the current count
let count: Int = 0;

// Increment the counter by 1
// Precondition: count is less than 100
// Postcondition: count increases by 1
txn increment [count < 100][count == @count + 1] {
    &count = count + 1;
    term;
};

// Decrement the counter by 1
// Precondition: count is greater than 0
// Postcondition: count decreases by 1
txn decrement [count > 0][count == @count - 1] {
    &count = count - 1;
    term;
};

// Reset counter to zero
// Precondition: count is not already 0
// Postcondition: count equals 0
txn reset [~/count][count == 0] {
    &count = 0;
    term;
};
```

### Step 3: Understand the New Concepts

Let's examine the new parts of this code:

**State Variables: `let count: Int = 0;`**

The word `let` creates a variable. The `: Int` says it's an integer (whole number). The `= 0` sets its initial value.

Unlike local variables inside a transaction, a variable at the top level of your file is **persistent**. It remembers its value between transactions.

**The `&` Symbol: `&count = count + 1;`**

The ampersand `&` means "modify the state variable." So `&count = count + 1` means:
1. Read the current `count`
2. Add 1 to it
3. Save the result back to `count`

Without the `&`, like `count + 1`, it would just be a calculation that doesn't change anything.

**The `@` Symbol: `count == @count + 1`**

The `@` means "the previous value." In the postcondition, `count == @count + 1` says:
- The new `count` equals the old `count` plus 1

This lets you express "count increases by 1" as a mathematical relationship.

**The `~/` Shorthand: `[~/count]`**

Instead of writing `[count != 0][count == 0]`, you can write `[~/count]` for short. It expands to two conditions:
- Precondition: `!count` (count is not true/false... wait, that's for booleans)

Actually, for our counter, `[~/count]` means the postcondition of the *previous* transaction must have been satisfied. It ensures operations happen in sequence.

Let's use the explicit form for clarity in our transactions. Update the `reset` transaction:

```brief
txn reset [count != 0][count == 0] {
    &count = 0;
    term;
};
```

### Step 4: Run the Program

```bash
brief run src/main.bv
```

You'll see the counter running in your browser. You can click buttons to increment, decrement, and reset.

### The Power of Contracts

Here's what makes Brief special: try changing `[count < 100]` to `[true]` and running:

```brief
txn increment [true][count == @count + 1] {
    &count = count + 1;
    term;
};
```

Brief will warn you:

```
error[P009]: trivial precondition
  |
  = transaction 'increment' has precondition '[true]' which is always satisfied
```

Brief noticed that `[true]` accepts any state, and that's suspicious. If increment can run *always*, what's stopping it from running when count is already 100?

This is Brief catching a potential bug before you even run the code.

---

## Key Concepts in Brief

Now that you've written two programs, let's formalize the key concepts.

### Transactions

A **transaction** (`txn`) is a named unit of work. It has:

1. A name (how you refer to it)
2. A contract (precondition and postcondition)
3. A body (the code that runs)

```brief
txn transaction_name [precondition][postcondition] {
    // code
};
```

Transactions are the main building blocks of Brief programs. When something needs to happen, you write a transaction for it.

### Contracts

A **contract** defines the rules for a transaction:

- **Precondition** - What must be true before the transaction runs
- **Postcondition** - What must be true after the transaction completes

Contracts are written in brackets `[pre][post]`.

**Preconditions:**
- `count < 100` - count must be less than 100
- `count > 0` - count must be greater than 0
- `result != 0` - the result must not be zero

**Postconditions:**
- `count == @count + 1` - count increases by 1
- `result == true` - the result is true
- `count == 0` - count equals zero

**Logical NOT with `~`:**
- `[~active]` means the transaction requires active to be false (for toggle behavior)

**The `~/` shorthand:**
- `[~/count]` is a shortcut for `[count != 0][count == 0]`
- It means "the previous state existed, and now we're resetting"

### State Variables

State variables are defined at the top level with `let`:

```brief
let name: Type = initial_value;
```

They persist across transactions. To modify them, use `&`:

```brief
&count = count + 1;  // Increment
&count = count - 1;  // Decrement
&count = 0;         // Reset
```

### Definitions

A **definition** (`defn`) is like a function or subroutine. It computes a value without changing state:

```brief
defn double(x: Int) -> Int [true][result == x * 2] {
    term x * 2;
};
```

Unlike transactions, definitions:
- Cannot modify state variables
- Take parameters
- Return a value
- Are called for their return value

### Structs and Instances

A **struct** defines a structure with named fields:

```brief
struct Counter {
    count: Int;
    
    txn increment [count < 100][count == @count + 1] {
        &count = count + 1;
        term;
    };
};
```

An **instance** is a specific Counter with its own fields:

```brief
let my_counter = Counter {};           // Default values
let big_counter = Counter { count: 50 };  // Custom values

big_counter.increment();  // Call its method
big_counter.count;       // Access its field
```

### The `term` Keyword

`term` ends a transaction or definition and specifies its result:

```brief
term "hello";           // Return the string "hello"
term count + 1;        // Return count plus 1
term;                   // Return nothing (Void)
```

For transactions with postconditions about state (like `count == @count + 1`), use `term;` without a value.

### Boolean Values and Conditions

Brief uses these boolean values:

| Value | Meaning |
|-------|---------|
| `true` | Always satisfied (no requirements) |
| `false` | Never satisfied (impossible) |

Conditions in contracts use comparison operators:

| Operator | Meaning |
|----------|---------|
| `==` | Equals |
| `!=` | Not equals |
| `<` | Less than |
| `>` | Greater than |
| `<=` | Less than or equal |
| `>=` | Greater than or equal |
| `&&` | And (both must be true) |
| `\|\|` | Or (either must be true) |
| `!` | Not (inverts true/false) |

---

## Where to Go Next

You've learned the basics of Brief! Here's where to go from here.

### Learn More

- **[QUICK-REFERENCE.md](../spec/QUICK-REFERENCE.md)** - Short syntax reference for when you need to look something up
- **[SPEC.md](../spec/SPEC.md)** - Complete language specification for deep understanding
- **[LANGUAGE-REFERENCE.md](../spec/LANGUAGE-REFERENCE.md)** - Detailed reference for all features

### Example Projects

- **[hello-world](../examples/hello-world/)** - The simplest possible Brief project
- **[simple-counter](../examples/simple-counter/)** - A counter with state variables (what you just built)
- **[counter.rbv](../examples/counter.rbv)** - A more complete counter with a visual display

### FFI (Calling Other Languages)

Brief can call functions from other languages:

- **[FFI-GUIDE.md](../spec/FFI-GUIDE.md)** - How to use external libraries
- **[MAPPER-GUIDE.md](../spec/MAPPER-GUIDE.md)** - How to add support for new languages

### CLI Reference

- **[CLI-GUIDE.md](../spec/CLI-GUIDE.md)** - Complete command reference for the Brief compiler

---

## Glossary

**Body** - The code inside a transaction or definition, between `{` and `}`.

**Contract** - Two conditions that specify what must be true before (precondition) and after (postcondition) a transaction runs.

**Definition (`defn`)** - A named computation that returns a value without modifying state.

**FFI (Foreign Function Interface)** - How Brief calls functions written in other languages.

**Instance** - A specific struct with its own field values.

**Lambda-style termination** - Using `term;` without a value when the postcondition describes state changes.

**Mapper** - A component that translates between Brief types and foreign language types.

**Postcondition** - The second part of a contract; what must be true after the code runs.

**Precondition** - The first part of a contract; what must be true before the code runs.

**Result** - A special variable holding the return value of a transaction or definition.

**State variable** - A variable defined at the top level that persists across transactions.

**Struct** - A named structure with fields and methods, defined with the `struct` keyword.

**Symbolic verification** - Brief's ability to mathematically prove that contracts can be satisfied.

**Transaction (`txn`)** - A named unit of work with a contract and a body.

---

*Welcome to Brief! We're glad you're here.*
