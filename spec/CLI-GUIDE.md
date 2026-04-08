# Brief CLI Guide

**Version:** 1.0  
**Purpose:** Complete command reference for the Brief compiler CLI  

---

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [Commands Overview](#commands-overview)
4. [Development Commands](#development-commands)
   - [check](#check)
   - [build](#build)
   - [run](#run)
5. [RBV Commands](#rbv-commands)
   - [rbv](#rbv)
   - [serve](#serve)
6. [Project Commands](#project-commands)
   - [init](#init)
   - [import](#import)
7. [FFI Commands](#ffi-commands)
   - [wrap](#wrap)
   - [map](#map)
8. [System Commands](#system-commands)
   - [install](#install)
   - [lsp](#lsp)
9. [Global Options](#global-options)
10. [Exit Codes](#exit-codes)
11. [Configuration](#configuration)
12. [Examples](#examples)

---

## Installation

### From Source

```bash
cargo build --release
./target/release/brief install
```

### From Binary

Download the latest release from GitHub and add to your PATH.

---

## Quick Start

```bash
# Create a new project
brief init my-project
cd my-project

# Run development server
brief run src/main.bv

# Build for production
brief build src/main.bv
```

---

## Commands Overview

| Command | Description |
|---------|-------------|
| `check <file>` | Type check without execution |
| `build <file>` | Full compilation |
| `run <file>` | Build, serve, and open browser |
| `rbv <file>` | Compile RBV to browser-ready files |
| `serve [dir]` | Serve static files |
| `init [name]` | Create new project |
| `import <name>` | Add dependency |
| `wrap <lib>` | Generate FFI bindings |
| `map <lib>` | Preview FFI bindings (dry-run) |
| `install` | Install CLI to ~/.local/bin |
| `lsp` | Start Language Server |

---

## Development Commands

### check

Type check a Brief file without executing it. Fastest way to validate code.

```bash
brief check <file.bv>
```

**Options:**
- `-a, --annotate` - Generate path annotations
- `--skip-proof` - Skip proof verification
- `-v, --verbose` - Verbose output
- `--quiet, --whisper` - Minimal output

**Example:**
```bash
brief check src/main.bv
```

**Output:**
```
✓ Type checking passed
✓ Proof verification passed
```

---

### build

Full compilation of a Brief file. Runs type checking, proof verification, and generates output.

```bash
brief build <file.bv>
```

**Options:**
- Same as `check`

**Example:**
```bash
brief build src/main.bv
```

**Output:**
```
Compiling main.bv...
✓ Type checking passed
✓ Proof verification passed
✓ Build complete
```

---

### run

Compile, build WASM, serve locally, and open browser.

```bash
brief run <file>
```

**Options:**
- `--out <dir>` - Output directory
- `--port <port>` - Port for server (default: 8080)
- `--no-open` - Don't open browser
- `--watch, -w` - Watch for changes and rebuild
- `--skip-proof` - Skip proof verification

**Example:**
```bash
# Run with defaults
brief run src/main.bv

# Custom port, watch mode
brief run src/main.bv --port 3000 --watch

# No browser open
brief run src/main.bv --no-open
```

---

## RBV Commands

### rbv

Compile Rendered Brief (RBV) to browser-ready files. RBV files combine Brief code with HTML views.

```bash
brief rbv <file.rbv>
```

**Options:**
- `--out <dir>` - Output directory (default: `<name>-build`)
- `--no-build` - Skip wasm-pack build

**Example:**
```bash
brief rbv src/view.rbv
```

---

### serve

Serve static files locally.

```bash
brief serve [dir]
```

**Options:**
- `--port <port>` - Port for server (default: 8080)
- `--no-open` - Don't open browser

**Example:**
```bash
# Serve current directory
brief serve

# Serve specific directory
brief serve dist

# Custom port
brief serve --port 9000
```

---

## Project Commands

### init

Create a new Brief project.

```bash
brief init [name]
```

**Example:**
```bash
# Create project in current directory
brief init

# Create project in subdirectory
brief init my-project
```

**Creates:**
```
my-project/
├── src/
│   └── main.bv
├── Cargo.toml
└── brief.toml
```

---

### import

Add a dependency to the current project.

```bash
brief import <name>
```

**Example:**
```bash
brief import std/io
brief import my-library
```

---

## FFI Commands

### wrap

Generate FFI bindings for a library. Writes generated files to disk.

```bash
brief wrap <lib> [options]
```

**Options:**
- `--mapper <name>` - Specify mapper (c, rust, wasm, js, python)
- `--out <dir>` - Output directory
- `--force` - Overwrite existing files

**Supported Libraries:**
- C headers (.h, .c)
- Rust crates (.rs)
- WebAssembly (.wasm, .wat)
- JavaScript/TypeScript (.js, .ts, .d.ts)
- Python (.py, .pyi)

**Example:**
```bash
# Auto-detect language
brief wrap lib/my_library.h

# Specify mapper explicitly
brief wrap lib/my_module.py --mapper python

# Output to custom directory
brief wrap lib/crypto --out src/ffi

# Force overwrite
brief wrap lib/my_lib.rs --force
```

**Generated Files:**
```
lib/ffi/generated/my_lib/
├── lib.bv          # Brief declarations
└── bindings.toml   # TOML metadata
```

---

### map

Analyze a library and preview generated bindings (dry-run). Does not write files.

```bash
brief map <lib> [options]
```

**Options:**
- `--mapper <name>` - Specify mapper

**Example:**
```bash
# Preview C library bindings
brief map lib/my_lib.h

# Preview Python library
brief map lib/utils.py --mapper python

# Preview TypeScript
brief map types/api.d.ts --mapper js
```

**Output:**
```bash
Library: my_lib
Mapper: c
Analyzed 15 functions

=== lib.bv (preview) ===

frgn sig read_file: String -> String;

defn read_file(path: String) -> String [
  true
][
  result.valid()
] {
  __raw_read_file(path)
};

=== bindings.toml (preview) ===

[[functions]]
name = "read_file"
location = "my_lib::read_file"
target = "native"
mapper = "c"
...
```

---

## System Commands

### install

Install the Brief CLI to `~/.local/bin`.

```bash
brief install
```

**Example:**
```bash
$ brief install
Installed 'brief' to /home/user/.local/bin/brief

Add to your PATH if needed:
  export PATH="$PATH:/home/user/.local/bin"

Add this line to your ~/.bashrc or ~/.zshrc to make it permanent.
```

---

### lsp

Start the Brief Language Server for IDE integration.

```bash
brief lsp
```

**Example:**
```bash
# Start LSP server
brief lsp

# LSP clients:
# - VSCode: Brief extension
# - Neovim: nvim-lspconfig
# - Emacs: eglans-lsp-mode
```

---

## Global Options

These options work with all commands:

| Option | Description |
|--------|-------------|
| `-h, --help` | Show help |
| `-v, --verbose` | Verbose output |
| `--quiet, --whisper` | Minimal output (for CI) |

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Type error |
| 3 | Proof verification failed |
| 4 | Parse error |
| 5 | FFI error |

---

## Configuration

### brief.toml

Project configuration file:

```toml
[project]
name = "my-project"
version = "0.1.0"

[build]
out_dir = "dist"
port = 8080

[proof]
enabled = true
timeout = 30

[ffi]
search_paths = ["lib", "vendor"]
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `BRIEF_HOME` | Brief home directory |
| `BRIEF_OUT` | Default output directory |
| `BRIEF_PORT` | Default server port |

---

## Examples

### Development Workflow

```bash
# 1. Create project
brief init my-app
cd my-app

# 2. Run with watch mode
brief run src/main.bv --watch

# 3. Type check while editing
brief check src/main.bv -v

# 4. Build for production
brief build src/main.bv
```

### FFI Workflow

```bash
# 1. Preview bindings for a C library
brief map libsqlite.h

# 2. Generate bindings
brief wrap libsqlite.h --out src/ffi

# 3. Use in code
echo 'frgn sig sqlite_open: String -> Int from "src/ffi/bindings.toml";'
```

### Multi-file Project

```bash
# Project structure
# src/
#   main.bv
#   lib/
#     math.bv
#     io.bv

# Run with multiple files
brief run src/main.bv

# Check all files
brief check src/lib/*.bv src/main.bv
```

### CI/CD Usage

```bash
# Minimal output for CI
brief check src/main.bv --quiet

# Skip expensive proof verification
brief check src/main.bv --skip-proof

# Exit code based
brief check src/main.bv || exit 1
```

---

## See Also

- [SPEC.md](SPEC.md) - Language specification
- [QUICK-REFERENCE.md](QUICK-REFERENCE.md) - Syntax quick reference
- [LANGUAGE-REFERENCE.md](LANGUAGE-REFERENCE.md) - Full language reference
- [FFI-GUIDE.md](FFI-GUIDE.md) - FFI documentation
- [MAPPER-GUIDE.md](MAPPER-GUIDE.md) - Creating language mappers
