#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

int clamp_i32(int val, int min, int max) {
    if (val < min) return min;
    if (val > max) return max;
    return val;
}

int abs_diff_i32(int a, int b) {
    return (a > b) ? (a - b) : (b - a);
}

int max3_i32(int a, int b, int c) {
    int m = (a > b) ? a : b;
    return (c > m) ? c : m;
}

int min3_i32(int a, int b, int c) {
    int m = (a < b) ? a : b;
    return (c < m) ? c : m;
}

uint32_t saturating_add_u32(uint32_t a, uint32_t b) {
    uint32_t res = a + b;
    return (res < a) ? 0xFFFFFFFFu : res;
}

uint32_t popcount_u32(uint32_t x) {
    uint32_t count = 0;
    while (x != 0) {
        count += (x & 1u);
        x >>= 1;
    }
    return count;
}

uint32_t gcd_u32(uint32_t a, uint32_t b) {
    while (b != 0) {
        uint32_t t = b;
        b = a % b;
        a = t;
    }
    return a;
}

int is_leap_year(int year) {
    if (year <= 0) return 0;
    return ((year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)) ? 1 : 0;
}

int signum_i32(int x) {
    if (x > 0) return 1;
    if (x < 0) return -1;
    return 0;
}
