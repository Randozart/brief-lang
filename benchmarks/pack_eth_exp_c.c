// pack_eth_exp_c — rule-19 packed-emission experiment reference (Phase 2)
// Reads the SAME 14-byte Ethernet header layout through a native
// __attribute__((packed)) bitfield struct — the `<{ i48, i48, i16 }>` GEP
// analog. The bytes + mutation must match pack_eth_exp.bv exactly:
//   w[0] = 0x45670123456789AB, w[1] = 0x00000800CDEF0123 (little-endian)
//   each iteration: i = count & 1; w[i] ^= count; then extract.
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#pragma pack(push, 1)
struct eth_hdr {
    uint64_t dst_mac : 48;
    uint64_t src_mac : 48;
    uint16_t ethertype : 16;
};
#pragma pack(pop)

int main(int argc, char **argv) {
    (void)argc;
    long N = 50000000;
    const char *b = getenv("BOUND");
    if (b) N = atol(b);
    union {
        struct eth_hdr h;
        uint64_t w[2];
    } u;
    u.w[0] = 0x45670123456789ABULL;
    u.w[1] = 0x00000800CDEF0123ULL;
    long long chk = 0;
    for (long count = 0; count < N; count++) {
        int i = (int)(count & 1);
        u.w[i] ^= (uint64_t)count;
        uint64_t dst = u.h.dst_mac;
        uint64_t src = u.h.src_mac;
        uint16_t etype = u.h.ethertype;
        chk = (chk * 31 + (long long)dst + (long long)src * 3 + (long long)etype * 7) & 0x7FFFFFFF;
        if (count % 3 == 0) printf("%lld\n", chk);
    }
    return 0;
}