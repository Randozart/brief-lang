/* 2026-08-23 (Plan 1 HCALL slice): install simulator — drives the NATIVE
 * (self-hosted) tamer. Architecture after the ABI findings:
 *
 *   C owns memory + tables: mallocs the interpreter buffers, parses the
 *   .lair header / host table / entry offset from the .bounty archive, and
 *   drives the fetch-execute loop. Briev (lib/tamer/main.bv, compiled via
 *   LLVM) exports ONE pure function: step(). Rationale: exported-defn
 *   state writes don't round-trip through the %state view (accessors read
 *   zeros → NULL derefs), and the convergent-txn loop engine can't host
 *   the driver loop yet — both documented in BUGS.md 2026-08-23.
 *
 * ABI: every exported Briev defn takes a LEADING %state pointer; we pass
 * one zeroed page throughout.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

/* Section type ids — must match src/bounty/mod.rs */
#define SECTION_LAIR        1u
#define SECTION_BEASTPACK   2u
#define SECTION_MANIFEST    3u
#define SECTION_USER_LAIR   4u

/* Layout — must match bounty::write_bounty_impl:
 * header: MAGIC(9) + version(u32) + flags(u32) + count(u32) = 21 bytes
 * table:  per section type(1) + offset(u64) + size(u64) = 17 bytes */
static const uint8_t BOUNTY_MAGIC[9] = {'B','O','U','N','D','A','T','A','\0'};

static const uint8_t* find_section(const uint8_t* data, size_t size,
                                   uint8_t type_id, size_t* out_size) {
    if (size < 21 || memcmp(data, BOUNTY_MAGIC, 9) != 0) return NULL;
    uint32_t count;
    memcpy(&count, data + 17, 4);
    size_t table = 21;
    if (table + (size_t)count * 17 > size) return NULL;
    for (uint32_t i = 0; i < count; i++) {
        const uint8_t* e = data + table + (size_t)i * 17;
        if (e[0] != type_id) continue;
        uint64_t off, ssize;
        memcpy(&off, e + 1, 8);
        memcpy(&ssize, e + 9, 8);
        if (off + ssize > size) return NULL;
        if (out_size) *out_size = (size_t)ssize;
        return data + off;
    }
    return NULL;
}

/* .lair header words (u64 at byte 16): str_off str_size fn_off fn_size
 * bc_off bc_len host_off host_size. */
static uint64_t lair_word(const uint8_t* lair, int i) {
    uint64_t v;
    memcpy(&v, lair + 16 + (size_t)i * 8, 8);
    return v;
}

/* Interpreter buffer layouts — must match lib/tamer/vm.bv structs:
 * VMStack = Int[1024] + len = 8200 B; VMLocals = Int[4096] + len = 32776 B;
 * VMFrames = Frame[256](24B each) + count = 6152 B.
 * HostTable = ids Int[64] + arities Int[64] + count = 1032 B. */
#define VMSTACK_BYTES  8200
#define VMLOCALS_BYTES 32776
#define VMFRAMES_BYTES 6152
#define HT_BYTES       1032

int main(int argc, char** argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: %s <bounty_file>\n", argv[0]);
        return 2;
    }
    FILE* f = fopen(argv[1], "rb");
    if (!f) { perror("cannot open bounty"); return 2; }
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    uint8_t* data = malloc((size_t)size);
    if (!data || fread(data, 1, (size_t)size, f) != (size_t)size) {
        fprintf(stderr, "cannot read bounty\n"); return 2;
    }
    fclose(f);

    size_t lair_size = 0, bp_size = 0, manifest_size = 0;
    const uint8_t* lair =
        find_section(data, (size_t)size, SECTION_USER_LAIR, &lair_size);
    if (!lair) lair = find_section(data, (size_t)size, SECTION_LAIR, &lair_size);
    const uint8_t* beastpack =
        find_section(data, (size_t)size, SECTION_BEASTPACK, &bp_size);
    const uint8_t* manifest =
        find_section(data, (size_t)size, SECTION_MANIFEST, &manifest_size);
    if (!lair || !beastpack || !manifest) {
        fprintf(stderr, "install_sim: missing .lair/.beastpack/manifest section\n");
        return 3;
    }
    fprintf(stderr, "[install_sim] lair=%zu beastpack=%zu manifest=%zu bytes\n",
            lair_size, bp_size, manifest_size);

    /* Manifest: "entry_bc":<u64> — absolute bytecode offset of user entry. */
    long long entry_bc = -1;
    {
        const char* p = memmem(manifest, manifest_size, "\"entry_bc\":", 11);
        if (p) entry_bc = strtoll(p + 11, NULL, 10);
    }
    if (entry_bc < 0) { fprintf(stderr, "manifest lacks entry_bc\n"); return 3; }

    /* Host table: parse in C (12-byte entries: name_idx/id/arity u32). */
    uint64_t str_off  = lair_word(lair, 0);
    uint64_t str_size = lair_word(lair, 1);
    uint64_t fn_off   = lair_word(lair, 2);
    uint64_t fn_size  = lair_word(lair, 3);
    uint64_t bc_off   = lair_word(lair, 4);
    uint64_t bc_len   = lair_word(lair, 5);
    uint64_t host_off = lair_word(lair, 6);
    uint64_t host_size = lair_word(lair, 7);

    extern void briev_host_table_set(long long idx, long long id, long long arity);
    uint64_t hcount = host_size / 12;
    if (hcount > 64) { fprintf(stderr, "host table too large\n"); return 3; }
    for (uint64_t i = 0; i < hcount; i++) {
        const uint8_t* e = lair + host_off + i * 12;
        uint32_t id, arity;
        memcpy(&id, e + 4, 4);
        memcpy(&arity, e + 8, 4);
        briev_host_table_set((long long)i, (long long)id, (long long)arity);
    }

    /* Interpreter buffers, zero-initialized (len/count start at 0). */
    uint8_t* vstack  = calloc(1, VMSTACK_BYTES);
    uint8_t* vlocals = calloc(1, VMLOCALS_BYTES);
    uint8_t* vframes = calloc(1, VMFRAMES_BYTES);
    if (!vstack || !vlocals || !vframes) { fprintf(stderr, "oom\n"); return 3; }

    /* Exports from lib/tamer/main.bv (compiled natively). All take the
     * leading %state pointer. */
    extern int64_t step(int64_t state, int64_t stack, int64_t locals,
                        int64_t frames, int64_t lair, int64_t bc_end,
                        int64_t fn_table, int64_t foff, int64_t fn_count,
                        int64_t pc);
    extern void briev_host_print_int(long long v);   /* briev_rt.c */
    extern long long briev_host_fail(long long id, long long arg);

    static uint8_t zero_state[65536];
    int64_t st = (int64_t)(intptr_t)zero_state;

    int64_t pc = entry_bc;
    int64_t bc_end = (int64_t)(bc_off + bc_len);
    (void)str_off; (void)str_size; /* names ride for diagnostics only */
    int steps = 0;
    while (pc >= 0 && steps < 1000000) {
        pc = step(st, (int64_t)(intptr_t)vstack, (int64_t)(intptr_t)vlocals,
                  (int64_t)(intptr_t)vframes, (int64_t)(intptr_t)lair,
                  bc_end, (int64_t)(intptr_t)(lair + fn_off),
                  (int64_t)fn_off, (int64_t)fn_size / 20, pc);
        steps++;
    }
    fprintf(stderr, "[install_sim] halted after %d steps\n", steps);
    free(vstack); free(vlocals); free(vframes); free(data);
    return 0;
}
