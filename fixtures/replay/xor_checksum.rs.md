```rust
pub fn xor_checksum(buf: &[u8]) -> u8 {
    buf.iter().fold(0u8, |acc, &b| acc ^ b)
}
```
