```rust
pub fn count_kv_lines(s: &str) -> usize {
    let mut count = 0usize;
    let mut has_eq = false;
    for b in s.bytes() {
        if b == b'=' {
            has_eq = true;
        } else if b == b'\n' {
            if has_eq {
                count += 1;
            }
            has_eq = false;
        }
    }
    if has_eq {
        count += 1;
    }
    count
}
```
