```rust
pub fn hex_nibble(c: i32) -> i32 {
    if c >= b'0' as i32 && c <= b'9' as i32 {
        c - b'0' as i32
    } else if c >= b'a' as i32 && c <= b'f' as i32 {
        c - b'a' as i32 + 10
    } else if c >= b'A' as i32 && c <= b'F' as i32 {
        c - b'A' as i32 + 10
    } else {
        -1
    }
}
```
