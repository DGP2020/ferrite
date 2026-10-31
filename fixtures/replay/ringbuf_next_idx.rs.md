```rust
pub fn ringbuf_next_idx(idx: u32, cap: u32) -> u32 {
    if cap == 0 {
        0
    } else {
        idx.wrapping_add(1) % cap
    }
}
```
