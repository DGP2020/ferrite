```rust
pub fn is_leap_year(year: i32) -> i32 {
    if year <= 0 {
        return 0;
    }
    if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
        1
    } else {
        0
    }
}
```
