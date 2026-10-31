```rust
pub fn djb2_hash(buf: &[u8]) -> u32 {
    let mut hash: u32 = 5381;
    for &b in buf {
        hash = hash.wrapping_shl(5).wrapping_add(hash).wrapping_add(b as u32);
    }
    hash
}
```
