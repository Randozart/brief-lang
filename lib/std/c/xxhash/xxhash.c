/*
 * xxhash.c — Single-implementation wrapper for xxHash
 *
 * #define XXH_IMPLEMENTATION must be defined in exactly one compilation unit
 * before including xxhash.h. This file is that unit.
 *
 * #define XXH_STATIC_LINKING_ONLY is required because xxhash.h v0.8.2
 * places struct definitions (XXH32_state_s, XXH64_state_s) behind this
 * guard, and the implementation code references struct internals.
 *
 * Included by: import "link/xxhash/xxhash.c" in Brief source.
 */
#define XXH_STATIC_LINKING_ONLY
#define XXH_IMPLEMENTATION
#include "xxhash.h"
