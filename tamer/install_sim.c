/* 2026-08-23 (plan 2026-08-23-vm-compile-tail-parity §1.2): install
 * simulator — drives the NATIVE (self-hosted) tamer. Reads a .bounty,
 * extracts the .lair + .beastpack sections (same table format as
 * src/bounty/mod.rs write_bounty_full), and calls the exported `tame`
 * from the Briev-compiled tamer binary.
 *
 * Build (see tools/install_sim.sh):
 *   clang -O2 tamer/install_sim.c <tamer-main.ll objects> -o install_sim
 * Run:
 *   ./install_sim <tamer_ll_or_bin> not needed — link once:
 *   clang -O2 tamer/install_sim.c lib/tamer/main.ll-builtin-binary...
 *
 * Simplest wiring: this file is linked TOGETHER with the compiled tamer
 * object (clang tamer.ll + install_sim.c). tame() is declared extern. */

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
 * table:  per section type(1) + offset(u64) + size(u64) = 17 bytes
 * data:   concatenated at the offsets given (absolute file offsets). */
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

/* Exported by lib/tamer/main.bv (compiled natively).
 * ABI (2026-08-23): the export wrapper threads a %state pointer first —
 * the reactive txns it calls take the program-state struct. The tamer
 * keeps no program state of its own, so one zeroed page is passed. */
extern int64_t tame(int64_t state, int64_t lair, int64_t lair_len,
                    int64_t beastpack, int64_t beastpack_len,
                    int64_t entry_bc);

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

    size_t lair_size = 0, bp_size = 0;
    const uint8_t* lair = find_section(data, (size_t)size, SECTION_USER_LAIR, &lair_size);
    if (!lair) lair = find_section(data, (size_t)size, SECTION_LAIR, &lair_size);
    const uint8_t* beastpack = find_section(data, (size_t)size, SECTION_BEASTPACK, &bp_size);
    if (!lair || !beastpack) {
        fprintf(stderr, "install_sim: missing .lair/.beastpack section\n");
        return 3;
    }
    fprintf(stderr, "[install_sim] lair=%zu bytes, beastpack=%zu bytes\n",
            lair_size, bp_size);

    /* Manifest carries "entry_bc":<u64> — the user entry's bytecode offset
     * inside the user .lair (written by the bounty builder). */
    size_t manifest_size = 0;
    const uint8_t* manifest =
        find_section(data, (size_t)size, SECTION_MANIFEST, &manifest_size);
    long long entry_bc = 0;
    if (manifest) {
        const char* p = memmem(manifest, manifest_size, "\"entry_bc\":", 11);
        if (p) entry_bc = strtoll(p + 11, NULL, 10);
    }
    fprintf(stderr, "[install_sim] entry_bc=%lld\n", entry_bc);

    /* 2026-08-23: drive the fetch-execute loop from C — see BUGS.md
     * "convergent-txn loop" entry. tame() initializes buffers + host table,
     * then step() runs one instruction per call until it halts (-1). */
    extern int64_t tame(int64_t state, int64_t lair, int64_t lair_len,
                        int64_t beastpack, int64_t beastpack_len,
                        int64_t entry_bc);
    extern int64_t step(int64_t state, int64_t stack, int64_t locals,
                        int64_t frames, int64_t lair, int64_t bc_end,
                        int64_t fn_table, int64_t foff, int64_t fn_count,
                        int64_t ht, int64_t pc);
    extern int64_t buffers_stack(void);
    extern int64_t buffers_locals(void);
    extern int64_t buffers_frames(void);
    extern int64_t buffers_ht(void);
    extern int64_t rc_fn_off(void);
    extern int64_t rc_fn_count(void);
    static uint8_t zero_state[65536];
    int64_t rc = tame((int64_t)(intptr_t)zero_state,
                      (int64_t)(intptr_t)lair, (int64_t)lair_size,
                      (int64_t)(intptr_t)beastpack, (int64_t)bp_size,
                      (int64_t)entry_bc);

    /* The interpreter buffers + host table live inside tame's frame; the
     * exported buffers_* accessors hand them to the driver loop. */
    int64_t stack_h = buffers_stack();
    int64_t locals_h = buffers_locals();
    int64_t frames_h = buffers_frames();
    int64_t ht_h = buffers_ht();
    int64_t foff = rc_fn_off();
    int64_t fn_count_v = rc_fn_count();
    int64_t pc = entry_bc;
    /* recompute bc_end from the user .lair header (word 6+7) */
    uint64_t bc_off_v, bc_len_v;
    memcpy(&bc_off_v, lair + 48, 8);
    memcpy(&bc_len_v, lair + 56, 8);
    int64_t bc_end = (int64_t)(bc_off_v + bc_len_v);
    int steps = 0;
    while (pc >= 0 && steps < 1000000) {
        pc = step((int64_t)(intptr_t)zero_state, stack_h, locals_h, frames_h,
                  (int64_t)(intptr_t)lair, bc_end,
                  (int64_t)(intptr_t)lair, foff, fn_count_v,
                  ht_h, pc);
        steps++;
    }
    fprintf(stderr, "[install_sim] halted after %d steps\n", steps);
    free(data);
    return 0;
}
