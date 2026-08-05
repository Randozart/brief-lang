# Plan B: Auto-link with `--link-rt`

> Created: 2026-05-30T14:15Z
> Status: Draft — ready for implementation
> Depends on: Nothing

## Problem

`--link-rt` currently writes `briv_rt.c` to disk, compiles it to a `.o` file, and prints link instructions. The user then has to manually run `llc` and `cc`/`ld` to produce a final binary. This is friction — the flag should produce a ready-to-run executable.

## Goal

When `--link-rt` is passed, produce a final executable binary in the output directory:

1. Compile `.ll` → `.o` via `llc`
2. Compile `briv_rt.c` → `.o` via `cc` (already done)
3. Link both `.o` files + libc into a final binary via `cc`

If `llc` or `cc` is missing, fall back to printing instructions (current behavior).

## Implementation

### File: `src/main.rs`
### Function: `run_llvm_compile()` (line 1791)

### Step 1: Gather paths

After the existing C runtime compilation block (around line 1877), compute:

```rust
let out_base = out_dir.unwrap_or(std::path::Path::new("."));
let ll_o_path = out_base.join(format!("{}.o", stem));
let exe_path = out_base.join(stem);
```

### Step 2: Compile .ll → .o

Attempt `llc` execution. If it fails, print instructions and return early:

```rust
let llc_result = std::process::Command::new("llc")
    .args(["-filetype=obj", "-O2"])
    .arg("-o")
    .arg(&ll_o_path)
    .arg(&output_file)
    .status();

match llc_result {
    Ok(status) if status.success() => {
        println!("  Object: {}", ll_o_path.display());
    }
    _ => {
        eprintln!("  Warning: llc not found or failed. To compile manually:");
        eprintln!("    llc {} -filetype=obj -o {}", output_file.display(), ll_o_path.display());
        return Ok(output_file);
    }
}
```

### Step 3: Link into final executable

Link with `cc` (preferred over `ld` because it auto-links libc):

```rust
let mut link_cmd = std::process::Command::new("cc");
link_cmd.args(["-O2", "-o"])
    .arg(&exe_path)
    .arg(&ll_o_path)
    .arg(&rt_o_path);
if has_wake {
    link_cmd.args(["-lrt", "-lpthread"]);
}

let link_status = link_cmd.status();

match link_status {
    Ok(status) if status.success() => {
        println!("  Binary: {}", exe_path.display());
    }
    _ => {
        eprintln!("  Warning: linking failed. Link manually:");
        eprintln!("    cc {} {} -o {}", ll_o_path.display(), rt_o_path.display(), exe_path.display());
    }
}
```

### Edge Cases

- **No `llc` installed**: Fall back to printing instructions, return `.ll` path
- **No `cc` installed**: Fall back to printing instructions
- **`cc` compiles runtime but linking fails**: Print link instructions, return `.o` path
- **Wake triggers + no librt**: `-lrt` and `-lpthread` are added to the link step; if unavailable, the link will fail and print instructions
- **Output already exists**: `cc -o` overwrites; no special handling needed

### Test Updates (Plan D — extended)

The existing `llvm_backend_test.rs` can be extended to verify:

1. `.ll` output contains the expected `call` instructions (done in Plan D)
2. `--link-rt` at minimum doesn't crash (integration test runs the binary)

We should NOT require `llc` or `cc` in CI — the integration test should only verify `.ll` output.

## Future Extensions

- Support `--link-rt --target <triple>` for cross-compilation
- Support `--link-rt -static` for fully static binaries
- Support `--link-rt --strip` for stripped release binaries