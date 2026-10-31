```rust
pub fn fnv1a_hash32(buf: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for &b in buf {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}
```
