```rust
pub fn crc32_buf(buf: &[u8]) -> u32 {
    let mut c: u32 = 0xffffffff;
    for &b in buf {
        c ^= b as u32;
        for _ in 0..8 {
            if (c & 1) != 0 {
                c = 0xedb88320 ^ (c >> 1);
            } else {
                c >>= 1;
            }
        }
    }
    c ^ 0xffffffff
}
```
