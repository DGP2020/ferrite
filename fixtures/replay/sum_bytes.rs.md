```rust
pub fn sum_bytes(buf: &[u8]) -> u32 {
    buf.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32))
}
```
