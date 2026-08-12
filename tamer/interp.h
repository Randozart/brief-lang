// 2026-07-25: Briev VM interpreter — stack-based 64-bit virtual machine.
// Executes .lair bytecode for the install-time compilation pipeline.
// Hosted in the tamer system tool (tamer/main.c). No dependencies beyond C99.

#ifndef BRIEV_VM_H
#define BRIEV_VM_H

#include <stdint.h>
#include <stddef.h>

// ── Opcodes ──────────────────────────────────────────────────────────────
// Each instruction is 1-9 bytes: opcode + optional immediate(s).
// Immediates are little-endian. Use memcpy to read — NOT pointer casts.

// No immediate (1 byte)
#define OP_NOP      0x00
#define OP_DROP     0x01
#define OP_DUP      0x02
#define OP_SWAP     0x03
#define OP_OVER     0x04
#define OP_ROT      0x05
#define OP_ADD      0x06
#define OP_SUB      0x07
#define OP_MUL      0x08
#define OP_DIV_S    0x09
#define OP_REM_S    0x0A
#define OP_AND      0x0B
#define OP_OR       0x0C
#define OP_XOR      0x0D
#define OP_NOT      0x0E
#define OP_SHL      0x0F
#define OP_SHR_S    0x10
#define OP_EQ       0x11
#define OP_NE       0x12
#define OP_LT_S     0x13
#define OP_LE_S     0x14
#define OP_GT_S     0x15
#define OP_GE_S     0x16
#define OP_LOAD     0x17
#define OP_STORE    0x18
#define OP_RET      0x19
#define OP_TRACE    0x1A
#define OP_TRAP     0x1B
#define OP_BNOT     0x1C

// 1-byte immediate (2 bytes)
#define OP_PUSH_I8      0x30
#define OP_LOAD_LOCAL   0x31
#define OP_STORE_LOCAL  0x32
#define OP_PUSH_FRAME   0x33
#define OP_POP_FRAME    0x34

// 2-byte immediate (3 bytes)
#define OP_PUSH_I16     0x50
#define OP_JMP          0x51
#define OP_JZ           0x52
#define OP_JNZ          0x53
#define OP_CALL         0x54

// 4-byte immediate (5 bytes)
#define OP_PUSH_I32     0x70
#define OP_HCALL        0x71
#define OP_CALL_PTR     0x72

// 8-byte immediate (9 bytes)
#define OP_PUSH_I64     0x90
#define OP_ALLOC        0x91
#define OP_LOAD_OFF     0x92
#define OP_STORE_OFF    0x93

// Variable-length
#define OP_PUSH_STR     0xB0

// ── .lair file header ────────────────────────────────────────────────────
// Layout (all little-endian):
//   Offset  Size  Field
//   0       4     Magic: "LAIR"
//   4       4     Version (u32)
//   8       1     Endianness (1=LE, 2=BE)
//   9       3     Reserved (zero)
//   12      4     Flags (u32)
//   16      8     String table offset (u64)
//   24      8     String table size (u64)
//   32      8     Function table offset (u64)
//   40      8     Function table size (u64)
//   48      8     Bytecode offset (u64)
//   56      8     Bytecode size (u64)
//   64      8     Host function table offset (u64)
//   72      8     Host function table size (u64)
//   80      8     Inverse table offset (u64) — noise pairing
//   88      8     Inverse table size (u64)
//   96      —     (padding to 8-byte alignment, header is 96 bytes)

#define LAIR_MAGIC       "LAIR"
#define LAIR_HEADER_SIZE 96
#define LAIR_ENDIAN_LE   1
#define LAIR_ENDIAN_BE   2

// ── Flags ────────────────────────────────────────────────────────────────
#define LAIR_FLAG_DEBUG      (1u << 0)
#define LAIR_FLAG_INVERSE    (1u << 1)

// ── Function table entry (20 bytes each, packed) ─────────────────────────
typedef struct __attribute__((packed)) {
    uint32_t name_idx;          // index into string table
    uint64_t bytecode_offset;   // offset from bytecode section start
    uint32_t bytecode_len;      // length in bytes
    uint16_t local_count;       // number of local slots
    uint16_t arg_count;         // number of argument slots
} LairFunction;

// ── Frame ────────────────────────────────────────────────────────────────
typedef struct {
    uint64_t* locals;           // pointer into flat local storage
    size_t local_count;
    const uint8_t* return_pc;   // PC to resume after ret
    size_t return_frame_idx;    // frame index to restore
} Frame;

// ── Host function ────────────────────────────────────────────────────────
// All host functions follow this signature.
// args[0..arg_count-1] contains the arguments.
// The return value is written to args[0] (in-place).
typedef void (*HostFn)(uint64_t* args, int arg_count);

// ── VM state ─────────────────────────────────────────────────────────────
typedef struct {
    // Operand stack
    uint64_t* stack;
    size_t stack_cap;
    size_t stack_len;

    // Local storage (flat for all frames)
    uint64_t* locals;
    size_t locals_cap;
    size_t locals_len;

    // Frame stack
    Frame* frames;
    size_t frame_cap;
    size_t frame_count;

    // Current execution state
    const uint8_t* bytecode;
    size_t bytecode_len;

    // Loaded .lair tables
    const char* string_table;
    size_t string_table_size;
    LairFunction* function_table;
    size_t function_count;
    HostFn* host_table;
    size_t host_count;

    // Error handling
    char error_buf[256];
    int has_error;
} VmState;

// ── API ──────────────────────────────────────────────────────────────────

// Initialize a VM state. Returns 0 on success, -1 on error.
int vm_init(VmState* vm);

// Free all memory owned by the VM state.
void vm_free(VmState* vm);

// Load a .lair bytecode buffer into the VM.
// Parses the header, string table, function table, and bytecode.
// Returns 0 on success, -1 on error (error_buf populated).
int vm_load_lair(VmState* vm, const uint8_t* lair_data, size_t lair_size);

// Register a host function by ID.
void vm_register_host(VmState* vm, uint32_t id, HostFn fn);

// Execute a function by table index.
// The function receives no arguments from the VM (arguments are pushed
// on the operand stack before calling vm_execute).
// Returns the top-of-stack value (or 0 for void functions).
// On error, returns 0 and populates error_buf.
uint64_t vm_execute(VmState* vm, uint32_t fn_idx);

// Find a function by name in the loaded .lair.
// Returns the function index, or -1 if not found.
int vm_find_function(VmState* vm, const char* name);

// Get the last error message.
const char* vm_error(VmState* vm);

#endif // BRIEV_VM_H
