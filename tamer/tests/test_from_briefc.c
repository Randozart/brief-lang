// 2026-07-25: Test loading a .lair file produced by briefc --backend vm.
#include "../interp.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

int main(int argc, char** argv) {
    const char* path = argc > 1 ? argv[1] : "/tmp/test_vm.lair";

    // Read .lair file
    FILE* f = fopen(path, "rb");
    assert(f && "cannot open .lair file");
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    uint8_t* data = (uint8_t*)malloc(sz);
    assert(data);
    fread(data, 1, sz, f);
    fclose(f);

    printf("Loaded .lair: %s (%ld bytes)\n", path, sz);

    // Initialize VM and load .lair
    VmState vm;
    assert(vm_init(&vm) == 0);
    int rc = vm_load_lair(&vm, data, (size_t)sz);
    if (rc != 0) {
        fprintf(stderr, "FAILED: %s\n", vm_error(&vm));
        return 1;
    }

    // Execute function 0 (should be "add")
    // Push arguments: b=7, a=5 (right-to-left)
    vm.stack[vm.stack_len++] = 7;
    vm.stack[vm.stack_len++] = 5;

    uint64_t result = vm_execute(&vm, 0);
    if (vm.has_error) {
        fprintf(stderr, "EXECUTION FAILED: %s\n", vm_error(&vm));
        return 1;
    }

    printf("add(5, 7) = %lu\n", (unsigned long)result);
    assert(result == 12 && "expected 5 + 7 = 12");

    vm_free(&vm);
    free(data);
    printf("PASSED\n");
    return 0;
}
