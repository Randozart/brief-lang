// 2026-07-25: Briv VM interpreter implementation.
// Stack-based 64-bit virtual machine. All multi-byte reads use memcpy
// to guarantee correct behavior on strict-alignment architectures (ARM, RISC-V).

#include "interp.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

// ── Helpers: unaligned-safe reads ───────────────────────────────────────
// Every multi-byte read from the bytecode stream uses memcpy, NOT pointer
// casts. The instruction stream is inherently unaligned because opcode
// bytes push immediates off alignment boundaries.

static inline uint8_t  read_u8(const uint8_t* p)  { return *p; }

static inline uint16_t read_u16(const uint8_t* p) {
    uint16_t v; memcpy(&v, p, 2); return v;
}

static inline uint32_t read_u32(const uint8_t* p) {
    uint32_t v; memcpy(&v, p, 4); return v;
}

static inline uint64_t read_u64(const uint8_t* p) {
    uint64_t v; memcpy(&v, p, 8); return v;
}

// ── Stack helpers ────────────────────────────────────────────────────────

static int stack_push(VmState* vm, uint64_t val) {
    if (vm->stack_len >= vm->stack_cap) {
        snprintf(vm->error_buf, sizeof(vm->error_buf),
                 "VM: operand stack overflow (cap=%zu)", vm->stack_cap);
        vm->has_error = 1;
        return -1;
    }
    vm->stack[vm->stack_len++] = val;
    return 0;
}

static uint64_t stack_pop(VmState* vm) {
    if (vm->stack_len == 0) {
        snprintf(vm->error_buf, sizeof(vm->error_buf),
                 "VM: operand stack underflow");
        vm->has_error = 1;
        return 0;
    }
    return vm->stack[--vm->stack_len];
}

static uint64_t stack_peek(VmState* vm, size_t depth) {
    if (depth >= vm->stack_len) return 0;
    return vm->stack[vm->stack_len - 1 - depth];
}

// ── Frame helpers ────────────────────────────────────────────────────────

static int push_frame(VmState* vm, size_t local_count) {
    if (vm->frame_count >= vm->frame_cap) {
        snprintf(vm->error_buf, sizeof(vm->error_buf),
                 "VM: frame stack overflow (cap=%zu)", vm->frame_cap);
        vm->has_error = 1;
        return -1;
    }
    // Allocate space in flat local storage
    if (vm->locals_len + local_count > vm->locals_cap) {
        snprintf(vm->error_buf, sizeof(vm->error_buf),
                 "VM: local storage overflow");
        vm->has_error = 1;
        return -1;
    }
    Frame* f = &vm->frames[vm->frame_count++];
    f->locals = &vm->locals[vm->locals_len];
    f->local_count = local_count;
    f->return_pc = NULL;
    f->return_frame_idx = 0;
    vm->locals_len += local_count;
    // Zero-initialize new locals
    memset(f->locals, 0, local_count * sizeof(uint64_t));
    return 0;
}

static void pop_frame(VmState* vm) {
    if (vm->frame_count == 0) return;
    Frame* f = &vm->frames[vm->frame_count - 1];
    // Reclaim local storage
    vm->locals_len -= f->local_count;
    vm->frame_count--;
}

// ── .lair loading ────────────────────────────────────────────────────────

int vm_load_lair(VmState* vm, const uint8_t* data, size_t size) {
    if (size < LAIR_HEADER_SIZE) {
        snprintf(vm->error_buf, sizeof(vm->error_buf),
                 "VM: .lair file too small (%zu bytes, need %d)", size, LAIR_HEADER_SIZE);
        vm->has_error = 1;
        return -1;
    }

    // Check magic
    if (memcmp(data, LAIR_MAGIC, 4) != 0) {
        snprintf(vm->error_buf, sizeof(vm->error_buf),
                 "VM: invalid .lair magic");
        vm->has_error = 1;
        return -1;
    }

    // Check version
    uint32_t version = read_u32(data + 4);
    if (version != 1) {
        snprintf(vm->error_buf, sizeof(vm->error_buf),
                 "VM: .lair version %u (expected 1)", (unsigned)version);
        vm->has_error = 1;
        return -1;
    }

    // Check endianness
    uint8_t endian = read_u8(data + 8);
    (void)endian; // For MVP: assume LE host. Future: check and reject.
    #if __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
    if (endian != LAIR_ENDIAN_BE) {
        snprintf(vm->error_buf, sizeof(vm->error_buf),
                 "VM: big-endian host cannot load LE .lair");
        vm->has_error = 1;
        return -1;
    }
    #endif

    // Read section table
    uint64_t str_off  = read_u64(data + 16);
    uint64_t str_size = read_u64(data + 24);
    uint64_t fn_off   = read_u64(data + 32);
    uint64_t fn_size  = read_u64(data + 40);
    uint64_t bc_off   = read_u64(data + 48);
    uint64_t bc_size  = read_u64(data + 56);
    uint64_t host_off = read_u64(data + 64);
    uint64_t host_sz  = read_u64(data + 72);

    // Validate offsets
    if (str_off + str_size > size || fn_off + fn_size > size ||
        bc_off + bc_size > size || host_off + host_sz > size) {
        snprintf(vm->error_buf, sizeof(vm->error_buf),
                 "VM: .lair section offsets out of range");
        vm->has_error = 1;
        return -1;
    }

    // Load string table
    vm->string_table = (const char*)(data + str_off);
    vm->string_table_size = str_size;

    // Load function table
    vm->function_count = fn_size / sizeof(LairFunction);
    vm->function_table = (LairFunction*)(data + fn_off);

    // Load bytecode
    vm->bytecode = data + bc_off;
    vm->bytecode_len = bc_size;

    // Load host function table (just store the raw data for now;
    // actual host fns are registered via vm_register_host)
    if (host_sz > 0) {
        // Host function table is an array of (name_idx, fn_id) pairs
        // 8 bytes each: 4 bytes name_idx + 4 bytes host_fn_id
        // We don't need to parse this at load time — it's metadata
        // for the assembler. Host functions are registered by ID.
    }

    return 0;
}

void vm_register_host(VmState* vm, uint32_t id, HostFn fn) {
    if ((size_t)id < vm->host_count) {
        vm->host_table[id] = fn;
    }
}

// 2026-07-30: Find a function by name. Returns index or -1.
int vm_find_function(VmState* vm, const char* name) {
    for (size_t i = 0; i < vm->function_count; i++) {
        LairFunction* fn = &vm->function_table[i];
        const char* fn_name = vm->string_table + fn->name_idx;
        if (strcmp(fn_name, name) == 0) {
            return (int)i;
        }
    }
    return -1;
}

// ── Fetch-decode-execute ─────────────────────────────────────────────────

uint64_t vm_execute(VmState* vm, uint32_t fn_idx) {
    vm->has_error = 0;

    // Validate function index
    if (fn_idx >= vm->function_count) {
        snprintf(vm->error_buf, sizeof(vm->error_buf),
                 "VM: function index %u out of range (count=%zu)",
                 (unsigned)fn_idx, vm->function_count);
        vm->has_error = 1;
        return 0;
    }

    LairFunction* fn = &vm->function_table[fn_idx];

    // Set up initial frame
    vm->locals_len = 0;
    vm->frame_count = 0;
    if (push_frame(vm, fn->local_count) != 0) return 0;

    // Copy arguments from stack into local slots
    // Arguments are pushed right-to-left; first arg is deepest on stack
    for (uint16_t i = 0; i < fn->arg_count; i++) {
        uint64_t arg = stack_pop(vm);
        vm->locals[fn->arg_count - 1 - i] = arg;
    }

    const uint8_t* pc = vm->bytecode + fn->bytecode_offset;
    const uint8_t* end = pc + fn->bytecode_len;

    while (pc < end && !vm->has_error) {
        uint8_t op = read_u8(pc);
        switch (op) {
            // ── Stack operations ────────────────────────────────────────
            case OP_NOP:
                pc += 1; break;

            case OP_DROP:
                stack_pop(vm); pc += 1; break;

            case OP_DUP: {
                uint64_t a = stack_peek(vm, 0);
                if (!vm->has_error) stack_push(vm, a);
                pc += 1; break;
            }

            case OP_SWAP: {
                uint64_t a = stack_pop(vm);
                uint64_t b = stack_pop(vm);
                if (vm->has_error) return 0;
                stack_push(vm, a);
                stack_push(vm, b);
                pc += 1; break;
            }

            case OP_OVER: {
                uint64_t a = stack_pop(vm);
                uint64_t b = stack_peek(vm, 0);
                if (vm->has_error) return 0;
                stack_push(vm, a);
                stack_push(vm, b);
                pc += 1; break;
            }

            case OP_ROT: {
                uint64_t a = stack_pop(vm);
                uint64_t b = stack_pop(vm);
                uint64_t c = stack_pop(vm);
                if (vm->has_error) return 0;
                stack_push(vm, b);
                stack_push(vm, a);
                stack_push(vm, c);
                pc += 1; break;
            }

            // ── Arithmetic ──────────────────────────────────────────────
            case OP_ADD: {
                uint64_t a = stack_pop(vm);
                uint64_t b = stack_pop(vm);
                if (!vm->has_error) stack_push(vm, b + a);
                pc += 1; break;
            }

            case OP_SUB: {
                uint64_t a = stack_pop(vm);
                uint64_t b = stack_pop(vm);
                if (!vm->has_error) stack_push(vm, b - a);
                pc += 1; break;
            }

            case OP_MUL: {
                uint64_t a = stack_pop(vm);
                uint64_t b = stack_pop(vm);
                if (!vm->has_error) stack_push(vm, b * a);
                pc += 1; break;
            }

            case OP_DIV_S: {
                int64_t a = (int64_t)stack_pop(vm);
                int64_t b = (int64_t)stack_pop(vm);
                if (a == 0) {
                    snprintf(vm->error_buf, sizeof(vm->error_buf),
                             "VM: division by zero"); vm->has_error = 1;
                    return 0;
                }
                if (!vm->has_error) stack_push(vm, (uint64_t)(b / a));
                pc += 1; break;
            }

            case OP_REM_S: {
                int64_t a = (int64_t)stack_pop(vm);
                int64_t b = (int64_t)stack_pop(vm);
                if (a == 0) {
                    snprintf(vm->error_buf, sizeof(vm->error_buf),
                             "VM: division by zero"); vm->has_error = 1;
                    return 0;
                }
                if (!vm->has_error) stack_push(vm, (uint64_t)(b % a));
                pc += 1; break;
            }

            case OP_AND: {
                uint64_t a = stack_pop(vm); uint64_t b = stack_pop(vm);
                if (!vm->has_error) { stack_push(vm, b & a); }
                pc += 1; break;
            }

            case OP_OR: {
                uint64_t a = stack_pop(vm); uint64_t b = stack_pop(vm);
                if (!vm->has_error) { stack_push(vm, b | a); }
                pc += 1; break;
            }

            case OP_XOR: {
                uint64_t a = stack_pop(vm); uint64_t b = stack_pop(vm);
                if (!vm->has_error) { stack_push(vm, b ^ a); }
                pc += 1; break;
            }

            case OP_NOT: {
                uint64_t a = stack_pop(vm);
                // 2026-07-26: Logical NOT (0→1, else→0), not bitwise.
                // Boolean inversion for jz/jnz. Bitwise NOT is OP_BNOT.
                if (!vm->has_error) { stack_push(vm, a == 0 ? 1 : 0); }
                pc += 1; break;
            }

            case OP_BNOT: {
                uint64_t a = stack_pop(vm);
                // 2026-07-26: Bitwise NOT (~a). Separate from OP_NOT which
                // is logical. Added so the VM can express both operations.
                if (!vm->has_error) { stack_push(vm, ~a); }
                pc += 1; break;
            }

            case OP_SHL: {
                uint64_t a = stack_pop(vm); uint64_t b = stack_pop(vm);
                if (!vm->has_error) { stack_push(vm, b << (a & 63)); }
                pc += 1; break;
            }

            case OP_SHR_S: {
                uint64_t a = stack_pop(vm);
                int64_t b = (int64_t)stack_pop(vm);
                if (!vm->has_error) { stack_push(vm, (uint64_t)(b >> (a & 63))); }
                pc += 1; break;
            }

            // ── Comparison ──────────────────────────────────────────────
            case OP_EQ: {
                uint64_t a = stack_pop(vm); uint64_t b = stack_pop(vm);
                if (!vm->has_error) { stack_push(vm, b == a ? 1 : 0); }
                pc += 1; break;
            }

            case OP_NE: {
                uint64_t a = stack_pop(vm); uint64_t b = stack_pop(vm);
                if (!vm->has_error) { stack_push(vm, b != a ? 1 : 0); }
                pc += 1; break;
            }

            case OP_LT_S: {
                int64_t a = (int64_t)stack_pop(vm);
                int64_t b = (int64_t)stack_pop(vm);
                if (!vm->has_error) { stack_push(vm, b < a ? 1 : 0); }
                pc += 1; break;
            }

            case OP_LE_S: {
                int64_t a = (int64_t)stack_pop(vm);
                int64_t b = (int64_t)stack_pop(vm);
                if (!vm->has_error) { stack_push(vm, b <= a ? 1 : 0); }
                pc += 1; break;
            }

            case OP_GT_S: {
                int64_t a = (int64_t)stack_pop(vm);
                int64_t b = (int64_t)stack_pop(vm);
                if (!vm->has_error) { stack_push(vm, b > a ? 1 : 0); }
                pc += 1; break;
            }

            case OP_GE_S: {
                int64_t a = (int64_t)stack_pop(vm);
                int64_t b = (int64_t)stack_pop(vm);
                if (!vm->has_error) { stack_push(vm, b >= a ? 1 : 0); }
                pc += 1; break;
            }

            // ── Memory operations ───────────────────────────────────────
            case OP_LOAD: {
                uint64_t addr = stack_pop(vm);
                if (!vm->has_error) {
                    // 2026-07-25: Bounds check to prevent segfaults in MVP.
                    if (addr < 4096 || addr > 0x7FFFFFFFFFFF) {
                        // Suspicious address — push 0 instead of crashing
                        stack_push(vm, 0);
                    } else {
                        uint64_t val; memcpy(&val, (void*)(uintptr_t)addr, 8);
                        stack_push(vm, val);
                    }
                }
                pc += 1; break;
            }

            case OP_STORE: {
                uint64_t val = stack_pop(vm);
                uint64_t addr = stack_pop(vm);
                if (!vm->has_error) {
                    memcpy((void*)(uintptr_t)addr, &val, 8);
                }
                pc += 1; break;
            }

            case OP_LOAD_OFF: {
                uint64_t offset = read_u64(pc + 1);
                uint64_t base = stack_pop(vm);
                if (!vm->has_error) {
                    uint64_t val;
                    memcpy(&val, (void*)(uintptr_t)(base + offset), 8);
                    stack_push(vm, val);
                }
                pc += 9; break;
            }

            case OP_STORE_OFF: {
                uint64_t offset = read_u64(pc + 1);
                uint64_t val = stack_pop(vm);
                uint64_t base = stack_pop(vm);
                if (!vm->has_error) {
                    memcpy((void*)(uintptr_t)(base + offset), &val, 8);
                }
                pc += 9; break;
            }

            case OP_ALLOC: {
                uint64_t size = read_u64(pc + 1);
                if (!vm->has_error) {
                    void* ptr = malloc((size_t)size);
                    memset(ptr, 0, (size_t)size);
                    stack_push(vm, (uint64_t)(uintptr_t)ptr);
                }
                pc += 9; break;
            }

            // ── Local storage ───────────────────────────────────────────
            case OP_LOAD_LOCAL: {
                uint8_t idx = read_u8(pc + 1);
                if (vm->frame_count > 0) {
                    Frame* f = &vm->frames[vm->frame_count - 1];
                    if ((size_t)idx < f->local_count) {
                        if (!vm->has_error) stack_push(vm, f->locals[idx]);
                    } else {
                        snprintf(vm->error_buf, sizeof(vm->error_buf),
                                 "VM: local slot %u out of range (count=%zu)",
                                 (unsigned)idx, f->local_count);
                        vm->has_error = 1; return 0;
                    }
                }
                pc += 2; break;
            }

            case OP_STORE_LOCAL: {
                uint8_t idx = read_u8(pc + 1);
                uint64_t val = stack_pop(vm);
                if (vm->has_error) return 0;
                if (vm->frame_count > 0) {
                    Frame* f = &vm->frames[vm->frame_count - 1];
                    if ((size_t)idx < f->local_count) {
                        f->locals[idx] = val;
                    } else {
                        snprintf(vm->error_buf, sizeof(vm->error_buf),
                                 "VM: local slot %u out of range (count=%zu)",
                                 (unsigned)idx, f->local_count);
                        vm->has_error = 1; return 0;
                    }
                }
                pc += 2; break;
            }

            case OP_PUSH_FRAME: {
                uint8_t slots = read_u8(pc + 1);
                if (push_frame(vm, slots) != 0) return 0;
                pc += 2; break;
            }

            case OP_POP_FRAME:
                pop_frame(vm); pc += 1; break;

            // ── Immediate values ────────────────────────────────────────
            case OP_PUSH_I8: {
                int64_t val = (int8_t)read_u8(pc + 1);
                if (!vm->has_error) stack_push(vm, (uint64_t)val);
                pc += 2; break;
            }

            case OP_PUSH_I16: {
                int64_t val = (int16_t)read_u16(pc + 1);
                if (!vm->has_error) stack_push(vm, (uint64_t)val);
                pc += 3; break;
            }

            case OP_PUSH_I32: {
                int64_t val = (int32_t)read_u32(pc + 1);
                if (!vm->has_error) stack_push(vm, (uint64_t)val);
                pc += 5; break;
            }

            case OP_PUSH_I64: {
                uint64_t val = read_u64(pc + 1);
                if (!vm->has_error) stack_push(vm, val);
                pc += 9; break;
            }

            case OP_PUSH_STR: {
                // push_str <table_idx: u16> — pushes pointer to string
                // data from the string table
                uint16_t idx = read_u16(pc + 1);
                // For MVP: just push the index (actual string data
                // is accessed by the host FFI or by load instructions)
                if (!vm->has_error) stack_push(vm, idx);
                pc += 3; break;
            }

            // ── Control flow ────────────────────────────────────────────
            case OP_JMP: {
                int16_t offset = (int16_t)read_u16(pc + 1);
                pc += offset + 3; break;
            }

            case OP_JZ: {
                int16_t offset = (int16_t)read_u16(pc + 1);
                uint64_t cond = stack_pop(vm);
                if (vm->has_error) return 0;
                pc += 3;
                if (cond == 0) pc += offset;
                break;
            }

            case OP_JNZ: {
                int16_t offset = (int16_t)read_u16(pc + 1);
                uint64_t cond = stack_pop(vm);
                if (vm->has_error) return 0;
                pc += 3;
                if (cond != 0) pc += offset;
                break;
            }

            case OP_CALL: {
                uint16_t callee_idx = read_u16(pc + 1);
                if (callee_idx >= vm->function_count) {
                    snprintf(vm->error_buf, sizeof(vm->error_buf),
                             "VM: call to invalid function %u", (unsigned)callee_idx);
                    vm->has_error = 1; return 0;
                }
                LairFunction* callee = &vm->function_table[callee_idx];
                // Push new frame for callee first
                if (push_frame(vm, callee->local_count) != 0) return 0;
                // 2026-07-26: Save return state on callee frame, not caller.
                // Before: saved on frame_count-1 before push_frame (caller),
                // then push_frame created callee at frame_count-1 with NULL
                // return_pc. OP_RET read from callee and got NULL — every
                // call returned from vm_execute. After push_frame, frame_count-1
                // IS the callee. See docs/plans/2026-07-26-tamer-zero-c-and-static-memory.md.
                Frame* cur = &vm->frames[vm->frame_count - 1];
                cur->return_pc = pc + 3;
                cur->return_frame_idx = vm->frame_count - 1;

                // Copy arguments from stack into callee's local slots
                for (uint16_t i = 0; i < callee->arg_count; i++) {
                    uint64_t arg = stack_pop(vm);
                    if (vm->has_error) return 0;
                    vm->locals[vm->locals_len - callee->local_count
                               + callee->arg_count - 1 - i] = arg;
                }

                // Jump to callee
                pc = vm->bytecode + callee->bytecode_offset;
                break;
            }

            case OP_RET: {
                // Pop return value (if any)
                uint64_t retval = 0;
                if (vm->stack_len > 0) retval = stack_pop(vm);
                // Restore previous frame
                Frame* cur = &vm->frames[vm->frame_count - 1];
                const uint8_t* return_pc = cur->return_pc;
                pop_frame(vm);
                // Push return value on caller's stack
                if (!vm->has_error) stack_push(vm, retval);
                // If no return PC, we're returning from the entry function
                if (return_pc == NULL) return retval;
                pc = return_pc;
                break;
            }

            // ── Host FFI ────────────────────────────────────────────────
            case OP_HCALL: {
                uint32_t host_id = read_u32(pc + 1);
                if ((size_t)host_id >= vm->host_count || vm->host_table[host_id] == NULL) {
                    snprintf(vm->error_buf, sizeof(vm->error_buf),
                             "VM: host function %u not registered", (unsigned)host_id);
                    vm->has_error = 1; return 0;
                }
                // Collect arguments from stack
                // For MVP, we use a convention where the arg count is
                // encoded in the immediate or agreed upon by convention.
                // Simple approach: host function knows its own arity.
                // We pass all stack contents? No — that's too many.
                // For now, host functions receive (argc, argv) where
                // argv[0..argc-1] are the top N stack values.
                // The caller pushes args right-to-left.
                // The host fn writes return value to args[0].
                // We need to know how many args the host fn expects.
                // Convention: host functions with ID < 32 take 1 arg,
                // ID >= 32 take 0 args. This is a placeholder for MVP.
                // The real arity is encoded in the .lair host function table.
                // For now, host functions take 1 argument from the top of stack.
                // 2026-07-25: Copy top N args to a temp buffer so host_fn gets them.
                uint64_t host_args[8];
                int host_n = 0;
                if (vm->stack_len > 0) {
                    host_args[0] = vm->stack[vm->stack_len - 1];
                    host_n = 1;
                }
                vm->host_table[host_id](host_args, host_n);
                // Push return value (host writes to host_args[0])
                // For now, replace the top of stack with the return value
                if (vm->stack_len > 0) {
                    vm->stack[vm->stack_len - 1] = host_args[0];
                }
                pc += 5; break;
            }

            case OP_CALL_PTR: {
                uint64_t fn_addr = stack_pop(vm);
                if (vm->has_error) return 0;
                // call_ptr is for calling function pointers from heap.
                // For MVP, push the address back + a marker for the host.
                stack_push(vm, fn_addr);
                stack_push(vm, 0xCA11CA11); // marker
                pc += 5; break;
            }

            // ── Debug ───────────────────────────────────────────────────
            case OP_TRACE: {
                fprintf(stderr, "[VM trace] pc=%zu stack_len=%zu frame_count=%zu\n",
                        (size_t)(pc - vm->bytecode), vm->stack_len, vm->frame_count);
                pc += 1; break;
            }

            case OP_TRAP: {
                snprintf(vm->error_buf, sizeof(vm->error_buf),
                         "VM: TRAP at pc=%zu", (size_t)(pc - vm->bytecode));
                vm->has_error = 1;
                return 0;
            }

            default: {
                snprintf(vm->error_buf, sizeof(vm->error_buf),
                         "VM: unknown opcode 0x%02X at pc=%zu",
                         (unsigned)op, (size_t)(pc - vm->bytecode));
                vm->has_error = 1;
                return 0;
            }
        }
    }

    // When we fall off the end, treat as return with TOS
    if (vm->stack_len > 0) return stack_pop(vm);
    return 0;
}

// ── Init / Free / Error ──────────────────────────────────────────────────

int vm_init(VmState* vm) {
    memset(vm, 0, sizeof(*vm));
    vm->stack_cap = 1024;
    vm->stack = (uint64_t*)calloc(vm->stack_cap, sizeof(uint64_t));
    if (!vm->stack) return -1;
    vm->locals_cap = 4096;
    vm->locals = (uint64_t*)calloc(vm->locals_cap, sizeof(uint64_t));
    if (!vm->locals) { free(vm->stack); return -1; }
    vm->frame_cap = 256;
    vm->frames = (Frame*)calloc(vm->frame_cap, sizeof(Frame));
    if (!vm->frames) { free(vm->stack); free(vm->locals); return -1; }
    vm->host_count = 64;
    vm->host_table = (HostFn*)calloc(vm->host_count, sizeof(HostFn));
    if (!vm->host_table) { free(vm->stack); free(vm->locals); free(vm->frames); return -1; }
    return 0;
}

void vm_free(VmState* vm) {
    free(vm->stack);
    free(vm->locals);
    free(vm->frames);
    free(vm->host_table);
    memset(vm, 0, sizeof(*vm));
}

const char* vm_error(VmState* vm) {
    return vm->has_error ? vm->error_buf : NULL;
}
