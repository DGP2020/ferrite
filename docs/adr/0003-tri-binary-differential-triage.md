# Tri-Binary In-Process Differential Fuzzing in Linux Environment

Differential fuzzing requires high execution throughput that subprocess IPC cannot deliver, but `cargo-fuzz` and AddressSanitizer are unreliable under native Windows. We run the verification engine inside Linux (WSL2 or Docker), building three distinct binaries (C-only, Rust-only, and unified differential) to definitively triage crashes between C latent bugs, translation divergences, and harness errors.
