#include <stdint.h>
#include <stddef.h>

uint32_t adler32_step(uint32_t adler, const uint8_t *buf, size_t len) {
    uint32_t s1 = adler & 0xffffu;
    uint32_t s2 = (adler >> 16) & 0xffffu;
    for (size_t i = 0; i < len; i++) {
        s1 = (s1 + buf[i]) % 65521u;
        s2 = (s2 + s1) % 65521u;
    }
    return (s2 << 16) | s1;
}

uint32_t crc32_byte(uint32_t crc, uint8_t b) {
    uint32_t c = crc ^ (uint32_t)b;
    for (int k = 0; k < 8; k++) {
        c = (c & 1u) ? (0xedb88320u ^ (c >> 1)) : (c >> 1);
    }
    return c;
}

uint32_t crc32_buf(const uint8_t *buf, size_t len) {
    uint32_t c = 0xffffffffu;
    for (size_t i = 0; i < len; i++) {
        c ^= (uint32_t)buf[i];
        for (int k = 0; k < 8; k++) {
            c = (c & 1u) ? (0xedb88320u ^ (c >> 1)) : (c >> 1);
        }
    }
    return c ^ 0xffffffffu;
}
