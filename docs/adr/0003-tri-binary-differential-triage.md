# ADR-0003: Tri-Binary In-Process Differential Fuzzing in Linux Environment

**Date**: 2026-10-19
**Status**: accepted
**Deciders**: Ferrite Architecture Team

## Context

Differential fuzzing between the legacy C implementation and candidate Rust translations requires high throughput (>10,000 executions per second) to expose edge-case bugs and boundary discrepancies. Subprocess IPC (e.g. running C in one process and Rust in another via pipes or sockets) incurs heavy operating system context switching overhead, capping throughput at <100 executions per second.

Furthermore, AddressSanitizer (ASan) and UndefinedBehaviorSanitizer (UBSan) are essential for catching memory defects, but their support and stability under native Windows are notoriously fragile.

Crucially, when differential execution produces a mismatch or crash, it cannot be assumed that the Rust translation is faulty. In practice, boundary inputs generated during fuzzing frequently trigger latent buffer overflows, out-of-bounds reads, or undefined behaviors in the legacy C implementation while safe Rust gracefully returns an error or clamps values.

## Decision

Execute the verification engine inside Linux (WSL2 or Docker container) utilizing an in-process unified test harness linking both C and Rust implementations.

Adopt a **Tri-Binary Triage strategy**: upon detecting any output discrepancy or crash in the unified differential binary, Ferrite compiles and executes three distinct binaries:
1. **Standalone C Binary** compiled with Clang and `-fsanitize=address,undefined`.
2. **Standalone Rust Binary** compiled with `rustc` and debug assertions enabled.
3. **Unified Differential Binary** comparing returns and output buffers side-by-side.

If the standalone C binary crashes under ASan/UBSan while Rust executes cleanly, the finding is triaged as `Verdict::CLatentBug`. If C runs cleanly and Rust returns a different value or panics, it is triaged as `Verdict::Diverged`.

## Alternatives Considered

### Alternative 1: Out-of-Process Differential Fuzzing via IPC Pipes
- **Pros**: Complete crash isolation; memory corruption in C cannot destabilize the test runner.
- **Cons**: Severe performance degradation (<100 execs/sec), rendering deep fuzzing campaigns infeasible.
- **Why not**: Prevents achieving the necessary fuzzing depth and coverage plateaus within acceptable developer turnaround times.

### Alternative 2: Dual-Binary Comparison (Differential Only)
- **Pros**: Simpler build infrastructure; only one binary per function.
- **Cons**: When an ASan crash or signal occurs, the runner cannot definitively isolate whether C or Rust provoked the fault.
- **Why not**: Causes false-negative rejections of correct Rust translations that safely handle edge cases where legacy C crashes.

## Consequences

### Positive
- Fuzzing throughput exceeding 10,000 executions per second in-process.
- Definitive, automated triage distinguishing translation bugs from legacy C vulnerabilities (`CLatentBug`).
- Automatic generation of CWE classifications (CWE-787, CWE-125, CWE-190, CWE-476) and symbolicated crash backtraces.

### Negative
- Requires a Linux environment (WSL2 or Docker on Windows hosts) to leverage mature Clang sanitizers and libFuzzer.
- Memory corruption in legacy C during differential execution requires fallback isolation via standalone sub-binaries.

### Risks
- Severe C memory corruption could corrupt harness comparator memory before an ASan signal triggers; mitigated by executing isolated standalone C and Rust binaries on any anomalous signal or return difference.
