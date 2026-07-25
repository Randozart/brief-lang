# Phase 6: Write the Tamer in Brief — Self-Hosted Install-Time Compiler
## 2026-07-25

## Overview

The tamer system tool is currently implemented in C (`tamer/main.c`, `tamer/interp.c`).
Phase 6 rewrites the entire tamer in **Brief itself**, then compiles it to a native binary
via `briefc build --backend llvm`. This is the ultimate validation of the Bounty
architecture — Brief compiling a compiler tool, written in Brief, that compiles user
programs at install time.

### Architecture

```
lib/tamer/*.bv  (Brief source — the tamer)
    │
    ▼ briefc build --backend llvm tamer_rt.c
    │
tamer  (native binary, compiled via LLVM)
    │
    ▼ reads .bounty
    │
    ├── Parses .lair bytecode
    ├── Interprets .lair in embedded VM
    ├── Calls host FFI (LLVM, linker, CPUID)
    │
    ▼
Native binary (output, optimized for target CPU)
```

| Layer | Language | Role |
|-------|----------|------|
| `lib/tamer/*.bv` | Brief | VM interpreter, .bounty parser, orchestration |
| `tamer_rt.c` | C | Low-level runtime: stack operations, LLVM FFI, linker, CPUID |
| `tamer` | Native binary | Compiled output of the above two |

The tamer's **interpretation loop** (fetch-decode-execute of `.lair` bytecode) is
written in Brief using pattern matching. The **host FFI** (LLVM, linker, filesystem)
is accessed via `frgn` declarations backed by the tiny `tamer_rt.c` runtime.

The **VM instruction set and `.lair` format** remain unchanged from Phase 1. What
changes is WHO implements the interpreter — it moves from C to Brief.

### Why This Matters

1. **Self-hosting milestone**: Brief compiles a real-world systems tool written in
   itself. This validates the compiler's backend (LLVM codegen), its FFI model
   (`frgn` declarations), and its standard library (`Ptr<Int>`, `String`, match
   expressions, etc.).

2. **No more C in the critical path**: Eventually the `tamer_rt.c` can also be
   replaced with Brief + direct LLVM IR emission, but for Phase 6 it remains
   as a thin (~100 line) FFI layer.

3. **Demonstrates the VM backend's purpose**: The `briefc bounty` command produces
   `.lair` bytecode via `BackendKind::Vm`. The tamer interprets that exact format.
   They are two sides of the same coin.

---

## File Structure

### New Files

| File | Purpose |
|------|---------|
| `lib/tamer/main.bv` | Entry point: parse .bounty, extract sections, run pipeline |
| `lib/tamer/vm.bv` | VM interpreter: fetch-decode-execute loop |
| `lib/tamer/loader.bv` | .lair/.beastpack section loading and parsing |
| `lib/tamer/host_ffi.bv` | frgn declarations for LLVM, linker, CPUID, filesystem, stack |
| `lib/tamer/tamer_rt.c` | C runtime implementing the host FFI functions |

### Modified Files

| File | Change |
|------|--------|
| `tamer/Makefile` | Add rule to compile tamer from Brief: `briefc build --backend llvm lib/tamer/main.bv tamer_rt.c -o tamer` |
| `AGENTS.md` | Add Phase 6 as completed milestone |

---

## lib/tamer/main.bv — Entry Point

```brief
import host_ffi;

export defn main(argc: Int, argv: Ptr<Ptr<Int>>) -> Int {
    let bounty_path = load_arg(argv, 1);
    let data = host_ffi::read_file(bounty_path);
    let lair = parse_bounty_section(data, SECTION_LAIR);
    let beastpack = parse_bounty_section(data, SECTION_BEASTPACK);
    let manifest = parse_bounty_section(data, SECTION_MANIFEST);

    // Initialize VM
    let vm = VM::new();
    vm.load_lair(lair);

    // Register host functions
    vm.register_host(0, host_ffi::log);
    vm.register_host(1, host_ffi::cpuid);
    vm.register_host(2, host_ffi::os_abi);
    vm.register_host(3, host_ffi::llvm_create);
    // ... more host functions

    // Push beastpack data pointer as argument
    vm.push_arg(beastpack.data_ptr());
    vm.push_arg(beastpack.size());

    // Execute entry function
    let result = vm.execute(0);
    term result;
};
```

The entry point uses `frgn` to read files, parse the `.bounty` format, and
orchestrate the VM. The heavy lifting (VM interpretation) happens in `vm.bv`.

---

## lib/tamer/vm.bv — VM Interpreter

The VM implementation is a Brief module that implements the fetch-decode-execute
loop for the `.lair` bytecode format. It uses `frgn` calls for stack operations
and memory management.

```brief
// lib/tamer/vm.bv — Stack-based VM interpreter
import host_ffi;

pub struct VM {
    stack: Ptr<Int>,
    stack_cap: Int,
    stack_len: Int,
    locals: Ptr<Int>,
    locals_len: Int,
    bytecode: Ptr<Int>,
    bytecode_len: Int,
    functions: Ptr<FunctionEntry>,
    function_count: Int,
    host_table: Ptr<HostFn>,
    host_count: Int,
    error: String,
};

pub defn VM::new() -> VM {
    let cap = 1024;
    VM {
        stack: host_ffi::alloc(cap * 8),
        stack_cap: cap,
        stack_len: 0,
        locals: host_ffi::alloc(4096 * 8),
        locals_len: 0,
        bytecode: 0,
        bytecode_len: 0,
        functions: 0,
        function_count: 0,
        host_table: host_ffi::alloc(64 * 8),
        host_count: 64,
        error: "",
    }
};

// ── Fetch-decode-execute ────────────────────────────────────────────────

pub defn VM::execute(self: Ptr<VM>, fn_idx: Int) -> Int {
    let fns = self.functions;
    let fn_entry = fns + fn_idx * 20;  // 20 bytes per entry

    // Read function entry fields
    let bc_offset = load_i64(fn_entry + 4);
    let bc_len = load_u32(fn_entry + 12);
    let local_count = load_u16(fn_entry + 16);
    let arg_count = load_u16(fn_entry + 18);

    // Push frame and copy arguments
    let saved_locals = self.locals_len;
    host_ffi::push_frame(self, local_count);
    for i in 0..arg_count {
        let arg = host_ffi::stack_pop(self);
        store_local(self, arg_count - 1 - i, arg);
    };

    let end_pc = bc_offset + bc_len;
    let pc = bc_offset;

    while pc < end_pc && self.error == "" {
        let op = load_u8(self.bytecode + pc);
        // Dispatch via match — one arm per opcode
        pc = dispatch_op(self, op, pc);
    };
    host_ffi::pop_frame_to(self, saved_locals);
    let result = host_ffi::stack_pop(self);
    term result;
};

// ── Opcode dispatch ─────────────────────────────────────────────────────

defn dispatch_op(vm: Ptr<VM>, op: Int, pc: Int) -> Int {
    match op {
        0x00 => { term pc + 1; }                      // nop
        0x01 => { host_ffi::stack_pop(vm); term pc + 1; }  // drop
        0x02 => {                                       // dup
            let a = host_ffi::stack_peek(vm, 0);
            host_ffi::stack_push(vm, a);
            term pc + 1;
        }
        0x06 => {                                       // add
            let a = host_ffi::stack_pop(vm);
            let b = host_ffi::stack_pop(vm);
            host_ffi::stack_push(vm, b + a);
            term pc + 1;
        }
        0x30 => {                                       // push_i8
            let val = load_i8(vm.bytecode + pc + 1);
            host_ffi::stack_push(vm, val);
            term pc + 2;
        }
        0x51 => {                                       // jmp
            let offset = load_i16(vm.bytecode + pc + 1);
            term pc + 3 + offset;
        }
        0x52 => {                                       // jz
            let offset = load_i16(vm.bytecode + pc + 1);
            let cond = host_ffi::stack_pop(vm);
            if cond == 0 { term pc + 3 + offset; }
            else { term pc + 3; };
        }
        0x71 => {                                       // hcall
            let host_id = load_u32(vm.bytecode + pc + 1);
            host_ffi::call_host(vm, host_id);
            term pc + 5;
        }
        0x19 => { term vm.bytecode_len; }               // ret (jump to end)
        // ... remaining ~25 opcodes follow the same pattern
        _ => {
            vm.error = "unknown opcode: " + op.to_string();
            term pc;
        }
    };
};
```

### Key Design Decisions

**Stack operations are `frgn` calls**, not inline Brief operations. This is
because the VM stack lives in raw memory (`Ptr<Int>`) and needs bounds-checked
push/pop/peek. These are implemented in `tamer_rt.c`:

```c
// tamer_rt.c — stack operations
void tamer_stack_push(uint64_t* vm_state, uint64_t val) {
    VmState* vm = (VmState*)vm_state;
    if (vm->stack_len < vm->stack_cap) {
        vm->stack[vm->stack_len++] = val;
    }
}
```

**Bytecode reads use helper functions** (`load_u8`, `load_i16`, `load_u32`,
`load_i64`) which are inline in Brief and use Ptr arithmetic:

```brief
defn load_u8(ptr: Ptr<Int>) -> Int {
    term host_ffi::read_u8(ptr);
};
```

Or they could be direct `frgn` calls if Brief can't do pointer dereferencing
directly. The exact approach depends on what level of Ptr support is available.

---

## lib/tamer/loader.bv — Section Loading

```brief
// lib/tamer/loader.bv — Parse .bounty and .lair formats

pub defn parse_bounty_section(data: Ptr<Int>, section_type: Int) -> Ptr<Int> {
    // Parse .bounty header:
    //   magic(9) + version(4) + flags(4) + count(4) = 21 bytes header
    //   section table: entries of [type(1) + offset(8) + size(8)] = 17 bytes
    // Parse to find section with matching type, return its data pointer
    // For MVP, use host_ffi::find_section which does this in C.
    term host_ffi::find_section(data, section_type);
};

pub defn load_lair_header(data: Ptr<Int>) -> LairHeader {
    let version = load_u32(data + 4);
    let fn_offset = load_u64(data + 32);
    let fn_size = load_u64(data + 40);
    let bc_offset = load_u64(data + 48);
    let bc_size = load_u64(data + 56);
    LairHeader {
        version, fn_offset, fn_size,
        bc_offset, bc_size,
    }
};
```

---

## lib/tamer/host_ffi.bv — FFI Declarations

```brief
// lib/tamer/host_ffi.bv — Host function declarations for the tamer runtime

// ── Memory ────────────────────────────────────────────────────
frgn alloc(size: Int) -> Ptr<Int> as _tamer_alloc from "tamer_rt";

// ── Stack operations ──────────────────────────────────────────
frgn stack_push(vm: Ptr<Int>, val: Int) as _tamer_stack_push from "tamer_rt";
frgn stack_pop(vm: Ptr<Int>) -> Int as _tamer_stack_pop from "tamer_rt";
frgn stack_peek(vm: Ptr<Int>, depth: Int) -> Int as _tamer_stack_peek from "tamer_rt";
frgn push_frame(vm: Ptr<Int>, slot_count: Int) as _tamer_push_frame from "tamer_rt";
frgn pop_frame_to(vm: Ptr<Int>, saved_locals: Int) as _tamer_pop_frame_to from "tamer_rt";

// ── Bytecode reads ────────────────────────────────────────────
frgn read_u8(ptr: Ptr<Int>) -> Int as _tamer_read_u8 from "tamer_rt";
frgn read_i16(ptr: Ptr<Int>) -> Int as _tamer_read_i16 from "tamer_rt";
frgn read_u32(ptr: Ptr<Int>) -> Int as _tamer_read_u32 from "tamer_rt";
frgn read_i64(ptr: Ptr<Int>) -> Int as _tamer_read_i64 from "tamer_rt";

// ── File I/O ──────────────────────────────────────────────────
frgn read_file(path: String) -> Ptr<Int> as _tamer_read_file from "tamer_rt";
frgn get_data_size(ptr: Ptr<Int>) -> Int as _tamer_get_data_size from "tamer_rt";

// ── Section extraction (bounty format) ────────────────────────
frgn find_section(data: Ptr<Int>, type_id: Int) -> Ptr<Int> as _tamer_find_section from "tamer_rt";

// ── Target detection ──────────────────────────────────────────
frgn cpuid() -> Int as _tamer_cpuid from "tamer_rt";
frgn os_abi() -> Int as _tamer_os_abi from "tamer_rt";

// ── LLVM codegen ──────────────────────────────────────────────
frgn llvm_create() -> Int as _tamer_llvm_create from "tamer_rt";
frgn llvm_emit(mod: Int, ir: String) -> Int as _tamer_llvm_emit from "tamer_rt";
frgn llvm_optimize(mod: Int, level: Int) -> Int as _tamer_llvm_optimize from "tamer_rt";
frgn llvm_write(mod: Int, path: String) -> Int as _tamer_llvm_write from "tamer_rt";

// ── Linking ───────────────────────────────────────────────────
frgn link(objects: Ptr<Int>, count: Int, output: String) -> Int as _tamer_link from "tamer_rt";

// ── Logging ───────────────────────────────────────────────────
frgn log(msg: String) as _tamer_log from "tamer_rt";

// ── Dispatch helper ───────────────────────────────────────────
frgn call_host(vm: Ptr<Int>, host_id: Int) as _tamer_call_host from "tamer_rt";
```

---

## tamer_rt.c — C Runtime

This replaces `tamer/interp.c` and `tamer/main.c`. It's smaller because the VM
interpreter loop moves to Brief.

```c
// tamer_rt.c — Low-level runtime for the Brief-compiled tamer.
// Provides memory, stack, bytecode I/O, and LLVM FFI via frgn bindings.

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

// ── VM state (kept in C, accessed by Brief via Ptr<Int>) ────────────────
typedef struct {
    uint64_t* stack;
    size_t stack_cap, stack_len;
    uint64_t* locals;
    size_t locals_cap, locals_len;
    const uint8_t* bytecode;
    size_t bc_len;
    void* sections[3];
    size_t section_sizes[3];
    char error[256];
} TamerState;

// ── Stack operations ─────────────────────────────────────────────────────
void tamer_stack_push(TamerState* vm, uint64_t val) {
    if (vm->stack_len < vm->stack_cap)
        vm->stack[vm->stack_len++] = val;
}

uint64_t tamer_stack_pop(TamerState* vm) {
    if (vm->stack_len > 0)
        return vm->stack[--vm->stack_len];
    return 0;
}

// ── Bytecode reads (unaligned-safe) ─────────────────────────────────────
uint64_t tamer_read_u8(const uint8_t* p)  { return *p; }
uint64_t tamer_read_i16(const uint8_t* p) { int16_t v; memcpy(&v, p, 2); return v; }
uint64_t tamer_read_u32(const uint8_t* p) { uint32_t v; memcpy(&v, p, 4); return v; }
uint64_t tamer_read_i64(const uint8_t* p) { int64_t v; memcpy(&v, p, 8); return v; }

// ── LLVM FFI (MVP: shell out to clang) ──────────────────────────────────
void tamer_llvm_emit(int mod_handle, const char* ir_text) {
    FILE* f = fopen("/tmp/tamer_mod.ll", "a");
    if (f) { fputs(ir_text, f); fclose(f); }
}
```

---

## Implementation Order

| Step | What | Files | Depends On |
|------|------|-------|------------|
| 1 | Create `tamer_rt.c` | `tamer/tamer_rt.c` | None |
| 2 | Create `host_ffi.bv` | `lib/tamer/host_ffi.bv` | Step 1 |
| 3 | Create `loader.bv` | `lib/tamer/loader.bv` | Step 2 |
| 4 | Create `vm.bv` | `lib/tamer/vm.bv` | Step 2 |
| 5 | Create `main.bv` | `lib/tamer/main.bv` | Steps 3, 4 |
| 6 | Compile: `briefc build --backend llvm lib/tamer/main.bv tamer_rt.c -o tamer` | Makefile | Steps 1-5 |
| 7 | Test: `tamer /tmp/test_bounty.bounty` | — | Step 6 |
| 8 | Clean up old C VM files | Remove `tamer/interp.c`, `tamer/interp.h`, `tamer/main.c` | Step 7 |

---

## Testing Strategy

```bash
# 1. Build the native tamer from Brief source
cd lib/tamer
briefc build --backend llvm main.bv ../../tamer/tamer_rt.c -o ../../tamer/brief_tamer

# 2. Create a test bounty
briefc bounty /tmp/test.bv -o /tmp/test.bounty

# 3. Process with the Brief-compiled tamer
./tamer/brief_tamer /tmp/test.bounty

# 4. Run the output binary
/tmp/test_binary
```

### Comparison Tests

Run each phase's end-to-end test with both the C tamer and the Brief tamer,
comparing outputs:

```bash
# C tamer
./tamer/tamer /tmp/test.bounty -o /tmp/out_c
/tmp/out_c > /tmp/result_c.txt

# Brief tamer
./tamer/brief_tamer /tmp/test.bounty -o /tmp/out_brief
/tmp/out_brief > /tmp/result_brief.txt

# Compare
diff /tmp/result_c.txt /tmp/result_brief.txt  # should be empty
```

### Regression Guard

- All existing Rust tests must pass (`cargo test --lib`)
- All existing C tests must pass until step 8 (when they're removed)
- The Brief tamer must produce identical output to the C tamer for the same input

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| **Brief's Ptr<Int> support isn't low-level enough** | All pointer operations go through `frgn` calls to the C runtime. Brief code only does integers, booleans, structs, and match expressions — no unsafe pointer work in Brief itself. |
| **`frgn` calling convention doesn't support Ptr<Int>** | Pass pointers as `Int` (they're just 64-bit addresses). The C runtime casts them back. |
| **Match on 30+ opcodes causes compile-time slowdown** | Acceptable for a build-once tool. The tamer is compiled once per platform, not per program. |
| **Brief compiler doesn't support all patterns used** | The tamer's code uses only a restricted subset of Brief: `defn`, `if/match`, `for` loops, `fn` calls, `let` bindings, arithmetic. No generics beyond the needed scope. |
| **Old C tamer breaks during transition** | Keep making `tamer/tamer` as the C version. Create `tamer/brief_tamer` as the Brief version alongside it. Remove the C version only after the Brief version is verified identical. |

---

## Documentation Updates

| Document | Change |
|----------|--------|
| `docs/plans/2026-07-25-bounty-architecture.md` | Update Phase 6 section with brief impl details |
| `docs/architecture/bounty-architecture.md` | Update tamer section to describe Brief compilation |
| `AGENTS.md` | Add Phase 6 completion milestone |

### Rationale Comments

All new `.bv` files get header comments:

```brief
// 2026-07-25: Tamer VM interpreter in Brief.
// Implements the fetch-decode-execute loop for .lair bytecode.
// Compiled natively via briefc --backend llvm.
// Replaces the C interpreter (tamer/interp.c) with Brief source.
```

Every opcode dispatch arm in `vm.bv` gets a one-line comment referencing
the opcode's C equivalent in `tamer/interp.c` for cross-referencing.
