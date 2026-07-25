// 2026-07-25: Test the VM interpreter with a simple program.
// Constructs a .lair buffer in memory that computes 3 + 4.
// Verifies the result is 7.

#include "../interp.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

// Build a minimal .lair buffer with one function.
// Returns a malloc'd buffer (caller must free).
static uint8_t* build_lair_simple(size_t* out_size) {
    // Bytecode: push_i64(3), push_i64(4), add, ret
    uint8_t bytecode[] = {
        OP_PUSH_I64, 3, 0, 0, 0, 0, 0, 0, 0,  // push_i64 3
        OP_PUSH_I64, 4, 0, 0, 0, 0, 0, 0, 0,  // push_i64 4
        OP_ADD,                                  // add
        OP_RET,                                  // ret
    };
    size_t bc_len = sizeof(bytecode);

    // Function table: 1 function
    LairFunction fn;
    fn.name_idx = 0;         // "main" at string table index 0
    fn.bytecode_offset = 0;  // bytecode starts at beginning
    fn.bytecode_len = bc_len;
    fn.local_count = 0;
    fn.arg_count = 0;

    // String table: just "main\0"
    const char* strings = "main\0";
    size_t str_len = 5;

    // Calculate layout
    size_t header_size = LAIR_HEADER_SIZE;
    size_t str_off = header_size;
    size_t str_sz = str_len;
    size_t fn_off = str_off + str_sz;
    size_t fn_sz = sizeof(fn);
    size_t bc_off = fn_off + fn_sz;
    size_t total = bc_off + bc_len;

    uint8_t* buf = (uint8_t*)calloc(total, 1);
    // Header
    memcpy(buf, "LAIR", 4);
    uint32_t version = 1;
    memcpy(buf + 4, &version, 4);
    buf[8] = LAIR_ENDIAN_LE;
    // Section offsets
    memcpy(buf + 16, &str_off, 8);  // str_off
    uint64_t tmp64 = str_sz; memcpy(buf + 24, &tmp64, 8);  // str_sz
    memcpy(buf + 32, &fn_off, 8);   // fn_off
    tmp64 = fn_sz; memcpy(buf + 40, &tmp64, 8);  // fn_sz
    memcpy(buf + 48, &bc_off, 8);   // bc_off
    tmp64 = bc_len; memcpy(buf + 56, &tmp64, 8);  // bc_sz

    // String table
    memcpy(buf + str_off, strings, str_sz);

    // Function table
    memcpy(buf + fn_off, &fn, sizeof(fn));

    // Bytecode
    memcpy(buf + bc_off, bytecode, bc_len);

    *out_size = total;
    return buf;
}

int main(void) {
    // Build .lair
    size_t lair_size;
    uint8_t* lair = build_lair_simple(&lair_size);
    assert(lair != NULL);

    // Initialize VM
    VmState vm;
    int rc = vm_init(&vm);
    assert(rc == 0 && "vm_init failed");

    // Load .lair
    rc = vm_load_lair(&vm, lair, lair_size);
    if (rc != 0) {
        fprintf(stderr, "vm_load_lair failed: %s\n", vm_error(&vm));
        return 1;
    }

    // Execute function 0 (main)
    uint64_t result = vm_execute(&vm, 0);
    if (vm.has_error) {
        fprintf(stderr, "vm_execute failed: %s\n", vm_error(&vm));
        return 1;
    }

    // Verify: 3 + 4 = 7
    assert(result == 7 && "expected 3 + 4 = 7");
    printf("test_simple PASSED: 3 + 4 = %lu\n", (unsigned long)result);

    // Cleanup
    vm_free(&vm);
    free(lair);
    return 0;
}
