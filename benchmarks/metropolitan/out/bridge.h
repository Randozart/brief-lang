// brief_bridge — Brief export declarations (auto-generated)
// Tier 1: Zero-cost FFI — after LTO these IS the native function.
// Link with: brief build --shared --out . my_module.bv

#ifndef brief_bridge_H
#define brief_bridge_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

extern int64_t add(int64_t a, int64_t b);
extern int64_t mul(int64_t a, int64_t b);

#ifdef __cplusplus
}
#endif

#endif
