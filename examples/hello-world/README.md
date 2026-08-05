# Hello World

This is the simplest possible Briv program. It demonstrates the basic structure of a Briv transaction.

## Run It

From the `briv-compiler` directory:

```bash
briv run examples/hello-world/src/main.bv
```

## What It Does

When you run this program, the `greet` transaction executes and returns the string `"Hello, World!"`.

The contract `[true][result == "Hello, World!"]` means:
- **Precondition**: `true` - Always run (no requirements)
- **Postcondition**: `result == "Hello, World!"` - The output must equal this string

## The Code Explained

```briv
txn greet         // Define a transaction called "greet"
[true]           // Precondition: always satisfied
[result == "Hello, World!"]  // Postcondition: result must be this string
{
    term "Hello, World!";  // Return this value
};
```

## Next Steps

Once you understand this, move on to `examples/simple-counter/` to learn about state variables.
