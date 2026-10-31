```rust
pub fn is_ascii_digits(s: &str) -> i32 {
    if s.is_empty() {
        return 0;
    }
    if s.bytes().all(|b| b.is_ascii_digit()) {
        1
    } else {
        0
    }
}
```
