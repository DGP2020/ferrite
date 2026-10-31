# Contributing to the C-to-Rust Converter

First off, i give my gratitude for considering contributing! 

## Development Workflow (TDD)
This project strictly follows Test-Driven Development (TDD) and is documented via `spec.md` files. 

1. **Read the Specs:** Before writing code, review the relevant `spec.md` to understand the expected behavior.
2. **Write a Failing Test:** Add a test case in the `tests/` directory representing the C-input and expected Rust-output.
3. **Run Tests:** Run `cargo test` and verify your new test fails.
4. **Implement:** Write the minimum amount of Rust code required to make the test pass.
5. **Verify:** Run `cargo test` again. 
6. **Lint & Format:** Finally, run `cargo fmt` and `cargo clippy` before committing.

## Building Locally
Since this is a standard Cargo project, building is straightforward:

```bash
cargo build
cargo test 
cargo clippy 
```