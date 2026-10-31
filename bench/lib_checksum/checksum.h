#ifndef FERRITE_CHECKSUM_H
#define FERRITE_CHECKSUM_H

#include <stdint.h>
#include <stddef.h>

uint32_t adler32_step(uint32_t adler, const uint8_t *buf, size_t len);
uint32_t crc32_byte(uint32_t crc, uint8_t b);
uint32_t crc32_buf(const uint8_t *buf, size_t len);

#endif
