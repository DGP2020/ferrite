#include <stdint.h>
#include <stddef.h>

uint32_t sum_bytes(const uint8_t *buf, size_t len) {
    uint32_t sum = 0;
    for (size_t i = 0; i < len; i++) {
        sum += buf[i];
    }
    return sum;
}

uint8_t xor_checksum(const uint8_t *buf, size_t len) {
    uint8_t acc = 0;
    for (size_t i = 0; i < len; i++) {
        acc ^= buf[i];
    }
    return acc;
}

int find_first_zero(const uint8_t *buf, size_t len) {
    for (size_t i = 0; i < len; i++) {
        if (buf[i] == 0) {
            return (int)i;
        }
    }
    return -1;
}

size_t count_nonzero(const uint8_t *buf, size_t len) {
    size_t c = 0;
    for (size_t i = 0; i < len; i++) {
        if (buf[i] != 0) c++;
    }
    return c;
}

uint32_t fnv1a_hash32(const uint8_t *buf, size_t len) {
    uint32_t hash = 0x811c9dc5u;
    for (size_t i = 0; i < len; i++) {
        hash ^= (uint32_t)buf[i];
        hash *= 0x01000193u;
    }
    return hash;
}

uint32_t djb2_hash(const uint8_t *buf, size_t len) {
    uint32_t hash = 5381u;
    for (size_t i = 0; i < len; i++) {
        hash = ((hash << 5) + hash) + (uint32_t)buf[i];
    }
    return hash;
}

void reverse_bytes_in_place(uint8_t *buf, size_t len) {
    for (size_t i = 0; i < len / 2; i++) {
        uint8_t tmp = buf[i];
        buf[i] = buf[len - 1 - i];
        buf[len - 1 - i] = tmp;
    }
}
