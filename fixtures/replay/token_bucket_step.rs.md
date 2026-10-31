```rust
pub fn token_bucket_step(tokens: u32, add: u32, cap: u32) -> u32 {
    match tokens.checked_add(add) {
        Some(next) if next <= cap => next,
        _ => cap,
    }
}
```
