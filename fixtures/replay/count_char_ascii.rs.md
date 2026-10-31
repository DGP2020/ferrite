```rust
pub fn count_char_ascii(s: &str) -> usize {
    s.bytes().filter(|&b| b == b'a' || b == b'A').count()
}
```
