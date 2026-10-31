#include <stdint.h>
#include <stddef.h>
#include "checksum.h"

int main(void) {
    const uint8_t wiki[] = "Wikipedia";
    uint32_t a = adler32_step(1u, wiki, 9);
    if (a != 0x11E60398u) {
        return 1;
    }

    const uint8_t check[] = "123456789";
    uint32_t c = crc32_buf(check, 9);
    if (c != 0xCBF43926u) {
        return 2;
    }

    uint32_t step_c = 0xffffffffu;
    for (size_t i = 0; i < 9; i++) {
        step_c = crc32_byte(step_c, check[i]);
    }
    step_c ^= 0xffffffffu;
    if (step_c != c) {
        return 3;
    }

    return 0;
}
