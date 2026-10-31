```rust
pub fn ringbuf_used(head: u32, tail: u32, cap: u32) -> u32 {
    if cap == 0 {
        return 0;
    }
    let h = head % cap;
    let t = tail % cap;
    if h >= t {
        h - t
    } else {
        cap - t + h
    }
}
```
