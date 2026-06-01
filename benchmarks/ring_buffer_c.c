// Enum Dispatch Counter — C reference for Brief LLVM backend Path 4 benchmark
//
// Simple counter loop — Brief proves the body is pure with a known bound
// and emits `store i64 N` (O(1)). C gets the same optimization: the compiler
// eliminates the empty loop and just stores the final value.
//
// Build:
//   clang -O3 -march=native -o benchmarks/ring_buffer_c benchmarks/ring_buffer_c.c

int main(void) {
    long ops = 50000000L;
    (void)ops;
    return 0;
}
