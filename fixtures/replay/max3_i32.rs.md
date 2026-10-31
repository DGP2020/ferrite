```rust
pub fn max3_i32(a: i32, b: i32, c: i32) -> i32 {
    let m = if a > b { a } else { b };
    if c > m { c } else { m }
}
```
