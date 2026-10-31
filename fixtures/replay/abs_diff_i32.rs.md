```rust
pub fn abs_diff_i32(a: i32, b: i32) -> i32 {
    if a > b {
        a.wrapping_sub(b)
    } else {
        b.wrapping_sub(a)
    }
}
```
