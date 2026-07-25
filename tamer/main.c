// 2026-07-25: tamer — Brief install-time compiler system tool.
// Reads a .bounty file, extracts .lair + .beastpack, executes the
// compilation passes via the VM, and produces a native binary.
//
// Usage: tamer <bounty_file> [-o <output_dir>]
//
// This is the platform-specific component in the Bounty pipeline.
// Installed once per platform via package manager (apt, brew, winget).

#include "interp.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// ── .bounty format constants ─────────────────────────────────────────────
#define BOUNTY_MAGIC       "BOUNDATA\0"
#define BOUNTY_MAGIC_LEN   9
#define SECTION_LAIR       1
#define SECTION_BEASTPACK  2
#define SECTION_MANIFEST   3

// ── Host FFI stubs (MVP) ─────────────────────────────────────────────────
// These will be replaced with real LLVM/linker bindings in a future phase.
// For now, they log and return reasonable defaults.

static void host_log(uint64_t* args, int n) {
    // args[0] = string table index (the message)
    fprintf(stderr, "[tamer] host_log: arg0=%lu (n=%d)\n", (unsigned long)args[0], n);
}

static void host_cpuid(uint64_t* args, int n) {
    // Return CPU feature bitmask: assume x86_64 with basic features
    args[0] = 0x07; // bits: SSE, SSE2, AVX
    (void)n;
}

static void host_os_abi(uint64_t* args, int n) {
    // Return OS identifier: 0 = Linux
    args[0] = 0;
    (void)n;
}

// ── Section reading ──────────────────────────────────────────────────────
// Reads the .bounty section table and extracts a section by type.

static const uint8_t* find_section(const uint8_t* data, size_t size,
                                    uint8_t type, size_t* out_size) {
    if (size < 21) return NULL;
    if (memcmp(data, BOUNTY_MAGIC, BOUNTY_MAGIC_LEN) != 0) return NULL;

    uint32_t section_count;
    memcpy(&section_count, data + 17, 4);
    size_t table_start = 21;

    for (uint32_t i = 0; i < section_count; i++) {
        size_t entry_off = table_start + i * 17;
        if (entry_off + 17 > size) return NULL;

        if (data[entry_off] == type) {
            uint64_t sec_offset, sec_size;
            memcpy(&sec_offset, data + entry_off + 1, 8);
            memcpy(&sec_size, data + entry_off + 9, 8);
            if (sec_offset + sec_size > size) return NULL;
            *out_size = (size_t)sec_size;
            return data + sec_offset;
        }
    }
    return NULL;
}

// ── Main ─────────────────────────────────────────────────────────────────

int main(int argc, char** argv) {
    if (argc < 2) {
        fprintf(stderr, "Usage: tamer <bounty_file> [-o <output_dir>]\n");
        return 1;
    }

    const char* bounty_path = argv[1];
    const char* output_dir = ".";
    for (int i = 2; i < argc; i++) {
        if (strcmp(argv[i], "-o") == 0 && i + 1 < argc) {
            output_dir = argv[i + 1];
            i++;
        }
    }

    printf("[tamer] Loading .bounty: %s\n", bounty_path);

    // 1. Read .bounty file
    FILE* f = fopen(bounty_path, "rb");
    if (!f) {
        fprintf(stderr, "[tamer] Error: cannot open '%s'\n", bounty_path);
        return 1;
    }
    fseek(f, 0, SEEK_END);
    long file_size = ftell(f);
    fseek(f, 0, SEEK_SET);
    uint8_t* file_data = (uint8_t*)malloc((size_t)file_size);
    if (!file_data) { fclose(f); return 1; }
    fread(file_data, 1, (size_t)file_size, f);
    fclose(f);

    // 2. Extract sections
    size_t lair_size = 0;
    const uint8_t* lair = find_section(file_data, (size_t)file_size, SECTION_LAIR, &lair_size);
    if (!lair) {
        fprintf(stderr, "[tamer] Error: .lair section not found\n");
        free(file_data);
        return 1;
    }
    printf("[tamer]   .lair: %zu bytes\n", lair_size);

    size_t beastpack_size = 0;
    const uint8_t* beastpack = find_section(file_data, (size_t)file_size, SECTION_BEASTPACK, &beastpack_size);
    if (!beastpack) {
        fprintf(stderr, "[tamer] Error: .beastpack section not found\n");
        free(file_data);
        return 1;
    }
    printf("[tamer]   .beastpack: %zu bytes\n", beastpack_size);

    size_t manifest_size = 0;
    find_section(file_data, (size_t)file_size, SECTION_MANIFEST, &manifest_size);
    printf("[tamer]   manifest: %zu bytes\n", manifest_size);

    // 3. Initialize VM
    VmState vm;
    if (vm_init(&vm) != 0) {
        fprintf(stderr, "[tamer] Error: VM initialization failed\n");
        free(file_data);
        return 1;
    }

    // 4. Register host functions
    vm_register_host(&vm, 0, host_log);
    vm_register_host(&vm, 1, host_cpuid);
    vm_register_host(&vm, 2, host_os_abi);

    // 5. Load .lair bytecode
    if (vm_load_lair(&vm, lair, lair_size) != 0) {
        fprintf(stderr, "[tamer] Error: failed to load .lair: %s\n", vm_error(&vm));
        vm_free(&vm);
        free(file_data);
        return 1;
    }
    printf("[tamer]   functions: %zu\n", vm.function_count);

    // 6. Push .beastpack pointer as argument (convention: arg 0 = beastpack data)
    vm.stack[vm.stack_len++] = (uint64_t)(uintptr_t)beastpack;
    vm.stack[vm.stack_len++] = (uint64_t)beastpack_size;
    // Push output path string as arg
    // For MVP: arg 2 = output directory pointer (0 = cwd)
    vm.stack[vm.stack_len++] = (uint64_t)(uintptr_t)output_dir;

    // 7. Execute entry point (function 0 = main/tame)
    printf("[tamer] Taming the beast...\n");
    uint64_t result = vm_execute(&vm, 0);

    if (vm.has_error) {
        fprintf(stderr, "[tamer] Error during execution: %s\n", vm_error(&vm));
        vm_free(&vm);
        free(file_data);
        return 1;
    }

    // 8. Report success
    printf("[tamer] Taming complete! (exit code: %lu)\n", (unsigned long)result);
    printf("[tamer] Output directory: %s\n", output_dir);

    vm_free(&vm);
    free(file_data);
    return 0;
}
