# Hello World

This is the simplest possible Briev program. It demonstrates the basic structure of a Briev transaction.

## Run It

From the `briev-compiler` directory:

```bash
briev run examples/hello-world/src/main.bv
```

## What It Does

When you run this program, the `greet` transaction executes and returns the string `"Hello, World!"`.

The contract `[true][result == "Hello, World!"]` means:
- **Precondition**: `true` - Always run (no requirements)
- **Postcondition**: `result == "Hello, World!"` - The output must equal this string

## The Code Explained

```briev
txn greet         // Define a transaction called "greet"
[true]           // Precondition: always satisfied
[result == "Hello, World!"]  // Postcondition: result must be this string
{
    term "Hello, World!";  // Return this value
};
```

## Next Steps

Once you understand this, move on to `examples/simple-counter/` to learn about state variables.
