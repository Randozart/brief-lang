// brief_bridge — Brief extern "C" declarations (auto-generated)
// Tier 1: Zero-cost FFI — after LTO these are the same binary.
// Build with: brief build my_module.bv --shared --out .

extern "C" {

    pub fn add(a: i64, b: i64) -> i64;
    pub fn mul(a: i64, b: i64) -> i64;
}
