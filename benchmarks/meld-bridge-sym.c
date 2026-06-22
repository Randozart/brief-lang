/* meld-bridge-sym — Symmetric C reference for meld-bridge benchmark
 *
 * Produces identical output: consume_buffer returns 1, accum increments.
 */

#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct { int64_t ptr; int64_t len; } CBuffer;
typedef struct { int64_t data; int64_t size; } RSBuffer;

int64_t consume_buffer(int64_t data, int64_t size) {
    return (data != 0 && size > 0) ? 1 : 0;
}

int main(void) {
    int64_t accum = 0;
    int64_t raw_ptr = 42, raw_len = 16;

    while (accum < 1000) {
        CBuffer cb = { .ptr = raw_ptr, .len = raw_len };
        RSBuffer rs = *(RSBuffer*)&cb;  /* bitcast — same as meld */
        int64_t ok = consume_buffer(rs.data, rs.size);
        if (ok) accum++;
    }
    return 0;
}
