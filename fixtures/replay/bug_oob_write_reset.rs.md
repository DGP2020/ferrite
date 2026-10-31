```rust
pub fn bug_oob_write_reset(buf: &mut [u8]) {
    for b in buf.iter_mut() {
        *b = 0;
    }
}
```
