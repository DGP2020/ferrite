#include <stdint.h>
#include <stddef.h>
#include <limits.h>

uint32_t bug_off_by_one_read(const uint8_t *buf, size_t len) {
    /* BUG: off-by-one read at index i == len */
    uint32_t sum = 0;
    for (size_t i = 0; i <= len; i++) {
        sum += buf[i];
    }
    return sum;
}

int bug_signed_overflow_abs(int x) {
    /* BUG: signed integer overflow UB when x == INT_MIN */
    return (x < 0) ? -x : x;
}

void bug_oob_write_reset(uint8_t *buf, size_t len) {
    /* BUG: out-of-bounds write at buf[len] */
    for (size_t i = 0; i <= len; i++) {
        buf[i] = 0;
    }
}

uint32_t bug_empty_slice_head(const uint8_t *buf, size_t len) {
    /* BUG: reads buf[len] when len == 0 */
    if (len == 0) {
        return (uint32_t)buf[len];
    }
    return (uint32_t)buf[0];
}
