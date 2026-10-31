```rust
pub fn crc32_byte(crc: u32, b: u8) -> u32 {
    let mut c = crc ^ (b as u32);
    for _ in 0..8 {
        if (c & 1) != 0 {
            c = 0xedb88320 ^ (c >> 1);
        } else {
            c >>= 1;
        }
    }
    c
}
```
