# ADR-0007: Phase 1 Signature Scope: Primitives, Slices, Strings, and Out-Pointers

**Date**: 2026-10-21
**Status**: accepted
**Deciders**: Ferrite Architecture Team

## Context

C codebases feature an expansive array of complex data types: nested structs, self-referential linked lists, tagged and untagged unions, function pointers, variadic arguments (`va_list`), bitfields, and macro-synthesized structures. Attempting to support every possible C construct from day one causes severe feature creep, stalled progress, and brittle fuzz harness generation.

To deliver a working, robust end-to-end verification and repair pipeline, we must define a bounded, high-impact signature scope for Phase 1 that covers the vast majority of harnessable systems logic (math, cryptography, parsers, and buffer utilities).

## Decision

Phase 1 strictly bounds supported function signatures to four categories:
1. **Primitive Scalars**: Standard integer and floating-point types (`int32_t`, `uint64_t`, `double`, `bool`).
2. **Bounded Buffer Slices**: Contiguous buffer and length pairs (e.g. `(const uint8_t *buf, size_t len)` mapped to `&[u8]`; `(uint8_t *buf, size_t len)` mapped to `&mut [u8]`).
3. **NUL-Terminated Strings**: C character strings (`const char *str` mapped to `&str`).
4. **Scalar Out-Pointers**: C out-parameters (e.g., `int *out` or `uint32_t *res` mapped to Rust return tuples or `Result<T, E>`).

Unsupported constructs (nested structs, unions, function pointers, varargs) are identified during the Analyze stage and flagged with descriptive diagnostic messages, placing them on an explicit skip list until Phase 2.

## Alternatives Considered

### Alternative 1: Full Type Support (Structs, Unions, Function Pointers) in Phase 1
- **Pros**: Immediate broad coverage across arbitrary complex C libraries.
- **Cons**: Generating deterministic fuzz harnesses and arbitrary random generators for recursive structs and function pointer callbacks is immensely complex and prone to edge-case bugs.
- **Why not**: Premature complexity would prevent perfecting the core differential verification and triage engine.

### Alternative 2: Scalar Primitives Only (No Buffers or Slices)
- **Pros**: Trivial harness generation and input generation via simple integer fuzzing.
- **Cons**: Excludes buffer operations, parsers, checksums, and string algorithms—the exact areas where C memory vulnerabilities (buffer overflows) are most common.
- **Why not**: Fails to demonstrate Ferrite's core value proposition of eliminating memory safety bugs and detecting latent buffer overflows.

## Consequences

### Positive
- Fully automated, deterministic harness generation with 100% reliability for supported signatures.
- High-value demonstration across real-world libraries (`lib_checksum`, CRC32, Adler32, ASCII parsers).
- Unambiguous error reporting and clear skip diagnostics for unsupported C functions.

### Negative
- Functions utilizing nested structs, callback pointers, or varargs are excluded from automatic Phase 1 translation.
- Manual intervention or clustering configuration is required for functions sharing complex global state.

### Risks
- Users attempting to run Ferrite on arbitrary legacy codebases may encounter unsupported construct skips; mitigated by documentation and pre-check diagnostics in `ferrite analyze`.
