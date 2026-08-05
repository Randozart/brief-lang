// gate.go — Gate A + B for Go (cgo).
package main

/*
#cgo LDFLAGS: -L${SRCDIR} -lbench
#include <stdint.h>
typedef struct BrivState BrivState;
extern BrivState* __briv_init_state(void);
extern int64_t feature_hash(BrivState*, int64_t, int64_t);
extern int64_t add(int64_t, int64_t);
*/
import "C"
import (
    "fmt"
    "os"
    "strconv"
    "time"
)

func nativeFh(count, seed int64) int64 {
    h := seed
    for i := int64(0); i < count; i++ {
        h = (h ^ (i * 2654435761)) * 1099511628211
    }
    return h
}

func nativeAdd(a, b int64) int64 { return a + b }

func main() {
    r, _ := strconv.ParseInt(os.Args[1], 10, 64)
    state := C.__briv_init_state()
    const N = 200000
    const N2 = 2000000
    var sink int64
    C.feature_hash(state, 1000, C.int64_t(r))
    t0 := time.Now()
    for i := 0; i < N; i++ {
        sink += int64(C.feature_hash(state, 1000, C.int64_t(r)))
    }
    fmt.Printf("BRIEF_FH %.1f\n", float64(time.Since(t0).Nanoseconds())/N)
    nativeFh(1000, r)
    t0 = time.Now()
    for i := 0; i < N; i++ {
        sink += nativeFh(1000, r)
    }
    fmt.Printf("NATIVE_FH %.1f\n", float64(time.Since(t0).Nanoseconds())/N)
    C.add(C.int64_t(r), 4)
    t0 = time.Now()
    for i := 0; i < N2; i++ {
        sink += int64(C.add(C.int64_t(r), 4))
    }
    fmt.Printf("BRIEF_ADD %.2f\n", float64(time.Since(t0).Nanoseconds())/N2)
    nativeAdd(r, 4)
    t0 = time.Now()
    for i := 0; i < N2; i++ {
        sink += nativeAdd(r, 4)
    }
    fmt.Printf("BRIEF_ADD %.2f\n", float64(time.Since(t0).Nanoseconds())/N2)
    fmt.Println("sink", sink) // keep sink live
}
