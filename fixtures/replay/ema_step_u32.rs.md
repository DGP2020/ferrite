```rust
pub fn ema_step_u32(prev: u32, sample: u32) -> u32 {
    prev.wrapping_mul(3).wrapping_add(sample) / 4
}
```
