```rust
pub fn cstr_len_bounded(s: &str) -> usize {
    s.bytes().take_while(|&b| b != 0).take(4096).count()
}
```
