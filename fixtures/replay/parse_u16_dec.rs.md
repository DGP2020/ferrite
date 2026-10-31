```rust
pub fn parse_u16_dec(s: &str) -> i32 {
    if s.is_empty() {
        return -1;
    }
    let mut val: u32 = 0;
    for b in s.bytes() {
        if !b.is_ascii_digit() {
            return -1;
        }
        val = val.saturating_mul(10).saturating_add((b - b'0') as u32);
        if val > 65535 {
            return -1;
        }
    }
    val as i32
}
```
