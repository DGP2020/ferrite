#include <stdint.h>
#include <stddef.h>

uint32_t ringbuf_next_idx(uint32_t idx, uint32_t cap) {
    if (cap == 0) return 0;
    return (idx + 1u) % cap;
}

uint32_t ringbuf_used(uint32_t head, uint32_t tail, uint32_t cap) {
    if (cap == 0) return 0;
    uint32_t h = head % cap;
    uint32_t t = tail % cap;
    return (h >= t) ? (h - t) : (cap - t + h);
}

int stack_can_push(uint32_t len, uint32_t cap) {
    return (len < cap) ? 1 : 0;
}

uint32_t ema_step_u32(uint32_t prev, uint32_t sample) {
    return (prev * 3u + sample) / 4u;
}

uint32_t token_bucket_step(uint32_t tokens, uint32_t add, uint32_t cap) {
    uint32_t next = tokens + add;
    if (next < tokens || next > cap) return cap;
    return next;
}
