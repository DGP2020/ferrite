#include <stdint.h>
#include <stddef.h>

int parse_u16_dec(const char *s) {
    if (s[0] == '\0') return -1;
    uint32_t val = 0;
    for (size_t i = 0; s[i] != '\0'; i++) {
        char c = s[i];
        if (c < '0' || c > '9') return -1;
        val = val * 10u + (uint32_t)(c - '0');
        if (val > 65535u) return -1;
    }
    return (int)val;
}

int hex_nibble(int c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

size_t count_kv_lines(const char *s) {
    size_t count = 0;
    int has_eq = 0;
    for (size_t i = 0; s[i] != '\0'; i++) {
        if (s[i] == '=') {
            has_eq = 1;
        } else if (s[i] == '\n') {
            if (has_eq) count++;
            has_eq = 0;
        }
    }
    if (has_eq) count++;
    return count;
}
