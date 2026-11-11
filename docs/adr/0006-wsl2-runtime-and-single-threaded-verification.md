# ADR-0006: WSL2 Linux Runtime and Single-Threaded Verification Execution

**Date**: 2026-10-20
**Status**: accepted
**Deciders**: Ferrite Architecture Team

## Context

Ferrite must run reliably on developer workstations (including Windows developer laptops) without causing system unresponsiveness, out-of-memory (OOM) crashes, or excessive I/O latency. On Windows, executing builds across the 9P filesystem bridge (accessing `/mnt/c` from WSL2) causes a 5–10x slowdown in `rustc` and `clang` compilation times.

Furthermore, running multiple compiler invocations (`rustc`, `clang -fsanitize=address`) concurrently alongside libFuzzer campaigns quickly overwhelms systems with limited RAM (e.g., 8 GB laptops), resulting in kernel OOM-killer interventions that terminate random processes.

## Decision

Standardize execution inside a native Linux environment (WSL2 or Docker container) within the native Linux filesystem (`~/ferrite` or rootfs, avoiding `/mnt/c` bind mounts).

Enforce a bounded execution model:
1. System quotas configured for 5 GB RAM and 4 GB swap.
2. Concurrent asynchronous LLM provider requests are permitted to minimize idle latency during network round-trips.
3. Heavy compilation (`rustc`, `clang`) and differential fuzzing campaigns are **strictly serialized** (single-threaded verification execution).
4. LibFuzzer runs with `-rss_limit_mb=1024` to cap fuzzer memory usage.

## Alternatives Considered

### Alternative 1: Native Windows Verification
- **Pros**: Avoids requiring WSL2 or Docker setup for Windows developers.
- **Cons**: Clang AddressSanitizer and UndefinedBehaviorSanitizer on Windows are unstable; `cargo-fuzz` and libFuzzer lack official first-class Windows support.
- **Why not**: Compromises verification fidelity; memory bug triage must rely on production-grade sanitizer behavior.

### Alternative 2: Fully Parallel Multithreaded Verification
- **Pros**: Higher throughput on 32-core/64-core dedicated CI build servers.
- **Cons**: Incurs massive memory spikes ($>8$ GB) that reliably trigger OOM crashes on developer workstations and entry-level CI runners.
- **Why not**: Developer ergonomics and determinism on constrained hardware take precedence over unconstrained parallel scaling.

## Consequences

### Positive
- Completely eliminates cross-filesystem 9P I/O latency bottlenecks.
- Prevents out-of-memory crashes on resource-constrained developer hardware (e.g. 8–16 GB RAM laptops).
- Guarantees deterministic, reproducible sanitizer traces and fuzz execution counts.

### Negative
- Benchmark execution time scales linearly with the number of functions rather than saturating all CPU cores.
- Windows developers must install and launch Ferrite inside WSL2 or Docker.

### Risks
- Large test suites in Stage 6 (L4) may take noticeable time when serialized; mitigated by incremental caching of verified units in `ferrite-out/`.
