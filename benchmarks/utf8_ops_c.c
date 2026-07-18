// UTF-8 Operations — C reference for Brief LLVM backend
//
// Symmetric with utf8_ops.bv: Allocates buffers, stores loop
// counter, compares via memcmp + validates via utf8_validate.
// Uses pre-allocated buffers to match Alloc# arena semantics.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

static uint8_t buf_a[8];
static uint8_t buf_b[8];

static int utf8_validate(const uint8_t *data, int64_t len) {
    if (!data || len < 0) return 0;
    int64_t i = 0;
    while (i < len) {
        uint8_t c = data[i];
        int64_t seq_len;
        uint32_t min_cp;
        if (c < 0x80) { seq_len = 1; min_cp = 0; }
        else if (c < 0xC0) return 0;
        else if (c < 0xE0) { seq_len = 2; min_cp = 0x80; }
        else if (c < 0xF0) { seq_len = 3; min_cp = 0x800; }
        else if (c < 0xF8) { seq_len = 4; min_cp = 0x10000; }
        else return 0;
        if (i + seq_len > len) return 0;
        uint32_t cp = c & (0x7F >> seq_len);
        for (int64_t j = 1; j < seq_len; j++) {
            uint8_t b = data[i + j];
            if ((b & 0xC0) != 0x80) return 0;
            cp = (cp << 6) | (b & 0x3F);
        }
        if (cp < min_cp) return 0;
        if (cp >= 0xD800 && cp <= 0xDFFF) return 0;
        if (cp > 0x10FFFF) return 0;
        i += seq_len;
    }
    return 1;
}

int main(void) {
    const long N = 50000000L;
    long ops = 0;
    long checksum = 0;

    while (ops < N) {
        memcpy(buf_a, &ops, 8);
        long next = ops + 1;
        memcpy(buf_b, &next, 8);

        int cmp = memcmp(buf_a, buf_b, 8);
        int valid = utf8_validate(buf_a, 8);
        checksum += cmp + valid;

        ops++;
        if (ops % 5000000 == 0) {
            printf("%ld\n", checksum);
        }
    }

    return 0;
}
