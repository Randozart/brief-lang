// proto_shim.c — Protocol bridge executable
// 2026-07-24: Spawned as subprocess, communicates via stdin/stdout text protocol.
// Protocol: "add 3 4\n" → "7\n"
//
// Compile: gcc -O2 -o proto_shim proto_shim.c -ldl

#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <dlfcn.h>

typedef int64_t (*add_fn_t)(int64_t, int64_t);
typedef int64_t (*mul_fn_t)(int64_t, int64_t);

int main(int argc, char** argv) {
    (void)argc;

    // Determine .so path: same directory as the shim executable
    char so_path[4096];
    char* slash = strrchr(argv[0], '/');
    if (slash) {
        size_t dir_len = slash - argv[0];
        snprintf(so_path, sizeof(so_path), "%.*s/export_add.so", (int)dir_len, argv[0]);
    } else {
        snprintf(so_path, sizeof(so_path), "./export_add.so");
    }

    void* lib = dlopen(so_path, RTLD_LAZY | RTLD_LOCAL);
    if (!lib) {
        fprintf(stderr, "dlopen(%s): %s\n", so_path, dlerror());
        return 1;
    }

    add_fn_t add_fn = (add_fn_t)dlsym(lib, "add");
    if (!add_fn) { fprintf(stderr, "dlsym add: %s\n", dlerror()); return 1; }

    mul_fn_t mul_fn = (mul_fn_t)dlsym(lib, "mul");
    if (!mul_fn) { mul_fn = NULL; }

    char line[1024];
    while (fgets(line, sizeof(line), stdin)) {
        char fn_name[256];
        int64_t a = 0, b = 0;
        int parsed = sscanf(line, "%255s %ld %ld", fn_name, &a, &b);
        if (parsed < 1) continue;

        if (strcmp(fn_name, "add") == 0) {
            printf("%ld\n", add_fn(a, b));
        } else if (strcmp(fn_name, "mul") == 0 && mul_fn) {
            printf("%ld\n", mul_fn(a, b));
        } else {
            printf("-1\n");
        }
        fflush(stdout);
    }

    dlclose(lib);
    return 0;
}
