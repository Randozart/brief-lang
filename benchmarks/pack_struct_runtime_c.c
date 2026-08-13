// pack_struct_runtime_c — C reference for pack_struct_runtime.bv
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

typedef struct __attribute__((packed)) {
    uint64_t dst : 48;
    uint64_t src : 48;
    uint16_t etype;
} Eth;

typedef struct {
    uint16_t a : 12;
    uint8_t b : 4;
    uint8_t c;
} Nib;

typedef struct {
    uint16_t hi : 12;
    uint8_t lo : 4;
} Bp;

static long packmix(long i) {
    Eth e;
    Nib n;
    Bp p;
    e.dst = (0x00608000AABBULL + i) & 0xFFFFFFFFFFFFULL;
    e.src = 0x00204000CCDDULL;
    e.etype = 0x0800;
    n.a = (0xABC + i) & 0xFFF;
    n.b = (0xF + i) & 0xF;
    n.c = 0xFF;
    p.hi = (0xABC + i) & 0xFFF;
    p.lo = (0xF + i) & 0xF;
    return e.dst + e.src * 3 + e.etype * 7 + n.a + n.b * 5 + n.c * 11 + p.hi + p.lo * 13;
}

int main(int argc, char **argv) {
    long N = atol(getenv("BOUND"));
    long chk = 0;
    for (long count = 0; count < N; count++) {
        chk = (chk * 31 + packmix(count)) & 0x7FFFFFFF;
        if (count % 3 == 0) printf("%ld\n", chk);
    }
    return 0;
}
