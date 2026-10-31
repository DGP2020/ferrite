```rust
pub fn bug_signed_overflow_abs(x: i32) -> i32 {
    x.saturating_abs()
}
```
