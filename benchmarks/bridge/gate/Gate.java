// Gate.java — Gate A + B for Java (JNI). The class name MUST be `bench` so the
// JNI shim's Java_bench_* symbols match these native methods.
class bench {
    static { System.loadLibrary("bench"); }
    public static native long FeatureHash(long count, long seed);
    public static native long Add(long a, long b);

    static long nativeFh(long count, long seed) {
        long h = seed;
        for (long i = 0; i < count; i++) h = (h ^ (i * 2654435761L)) * 1099511628211L;
        return h;
    }
    static long nativeAdd(long a, long b) { return a + b; }

    public static void main(String[] args) {
        long r = Long.parseLong(args[0]);
        final int N = 200000, N2 = 2000000;
        long sink = 0;
        FeatureHash(1000, r);
        long t0 = System.nanoTime();
        for (int i = 0; i < N; i++) sink += FeatureHash(1000, r);
        System.out.printf("BRIEF_FH %.1f%n", (double)(System.nanoTime() - t0) / N);
        nativeFh(1000, r);
        t0 = System.nanoTime();
        for (int i = 0; i < N; i++) sink += nativeFh(1000, r + i);
        System.out.printf("NATIVE_FH %.1f%n", (double)(System.nanoTime() - t0) / N);
        Add(r, 4);
        t0 = System.nanoTime();
        for (int i = 0; i < N2; i++) sink += Add(r, 4);
        System.out.printf("BRIEF_ADD %.2f%n", (double)(System.nanoTime() - t0) / N2);
        nativeAdd(r, 4);
        t0 = System.nanoTime();
        for (int i = 0; i < N2; i++) sink += nativeAdd(r, i & 7);
        System.out.printf("NATIVE_ADD %.2f%n", (double)(System.nanoTime() - t0) / N2);
        if (sink == 0) System.out.println();
    }
}
