# gate.py — Gate A + B for Python. Native feature_hash masks to 64 bits
# (Python ints are bignums — no wrap); N is scaled for the interpreted native.
import sys
import time

sys.path.insert(0, sys.argv[2] if len(sys.argv) > 2 else ".")
import bench

MASK = (1 << 64) - 1


def native_fh(count, seed):
    h = seed
    for i in range(count):
        h = ((h ^ (i * 2654435761)) * 1099511628211) & MASK
    return h


def native_add(a, b):
    return a + b


def main():
    r = int(sys.argv[1])
    N, N2, Nn = 200000, 2000000, 2000
    bench.feature_hash(1000, r)
    t0 = time.perf_counter()
    for _ in range(N):
        bench.feature_hash(1000, r)
    print("BRIV_FH %.1f" % ((time.perf_counter() - t0) / N * 1e9))
    native_fh(1000, r)
    t0 = time.perf_counter()
    for i in range(Nn):
        native_fh(1000, r + i)
    print("NATIVE_FH %.1f" % ((time.perf_counter() - t0) / Nn * 1e9))
    bench.add(r, 4)
    t0 = time.perf_counter()
    for _ in range(N2):
        bench.add(r, 4)
    print("BRIV_ADD %.2f" % ((time.perf_counter() - t0) / N2 * 1e9))
    native_add(r, 4)
    t0 = time.perf_counter()
    for i in range(N2):
        native_add(r, i % 8)
    print("NATIVE_ADD %.2f" % ((time.perf_counter() - t0) / N2 * 1e9))


if __name__ == "__main__":
    main()
