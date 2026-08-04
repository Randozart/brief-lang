// gate_go_native.go — the NATIVE side of the Go gate (pure Go, CGO=0).
// Kept separate from the cgo gate.go: measuring a pure-Go function inside a
// cgo-linked binary produced bogus sub-1ns/iter numbers (Go's compiler
// optimized the pure call path differently when cgo was linked).
package main

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
    const N = 200000
    const N2 = 2000000
    var sink int64
    for i := 0; i < 50000; i++ { sink += nativeFh(1000, r+int64(i)) }
    t0 := time.Now()
    for i := 0; i < N; i++ { sink += nativeFh(1000, r+int64(i)) }
    fmt.Printf("NATIVE_FH %.1f\n", float64(time.Since(t0).Nanoseconds())/N)
    t0 = time.Now()
    for i := 0; i < N2; i++ { sink += nativeAdd(r, int64(i&7)) }
    fmt.Printf("NATIVE_ADD %.2f\n", float64(time.Since(t0).Nanoseconds())/N2)
    fmt.Println("sink", sink) // keep sink live (DCE would strip the timed loops)
}
