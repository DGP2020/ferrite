```rust
pub fn find_first_zero(buf: &[u8]) -> i32 {
    for (i, &b) in buf.iter().enumerate() {
        if b == 0 {
            return i as i32;
        }
    }
    -1
}
```
