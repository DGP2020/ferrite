```rust
pub fn count_nonzero(buf: &[u8]) -> usize {
    buf.iter().filter(|&&b| b != 0).count()
}
```
