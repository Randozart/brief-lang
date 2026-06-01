// Precompute Sum — C reference for Brief LLVM backend Path 3 benchmark
//
// Computes pairwise sum 0..500. Brief precomputes final values at compile time
// and emits only stores. C gets the same optimization: clang O3 eliminates
// the entire loop and stores the resulting constants.
//
// Build:
//   clang -O3 -march=native -o benchmarks/precompute_c benchmarks/precompute_sum_c.c

int main(void) {
    long count = 500;
    long acc_a = 0;
    long acc_b = 0;

    for (; count < 500;) {
        acc_a += count;
        count++;
        if (count >= 500) break;
        acc_b += count;
        count++;
    }

    (void)count;
    (void)acc_a;
    (void)acc_b;
    return 0;
}
