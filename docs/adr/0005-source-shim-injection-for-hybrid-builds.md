# ADR-0005: Source-Level Shim Injection with Per-Library `ferrite.toml` Configuration

**Date**: 2026-10-20
**Status**: accepted
**Deciders**: Ferrite Architecture Team

## Context

To achieve Assurance Level L4, migrated Rust functions must integrate into the original C project and pass the project's native test suite. However, C projects use varied linkage conventions, internal static functions, and complex compilation graphs.

Common binary linkage interception techniques have severe shortcomings:
- Linker symbol wrapping (`--wrap=symbol`) is specific to GNU `ld` and fails to intercept internal static calls within the same translation unit.
- Archive symbol overriding (`libferrite.a` before `libc.a`) is compiler- and platform-fragile, behaving differently across Clang, GCC, and MSVC.
- Preprocessor macro redefinition (`#define foo ferrite_foo`) causes header parse errors when function pointers or struct field names match the identifier.

## Decision

Adopt **Source-Level Shim Injection** orchestrated via a per-library `ferrite.toml` manifest:
1. In the target C source file, Ferrite replaces the target C function body with a concise delegation shim that forwards arguments directly to the exported Rust companion symbol (`ferrite_<name>`).
2. The translated Rust functions are compiled into a static library archive (`libferrite_<name>.a`).
3. Ferrite executes the native project `build_cmd` and `test_cmd` specified in `ferrite.toml`.
4. If the native project test suite completes successfully (`exit code 0`), the function is promoted to **`AssuranceLevel::L4`**.

Ferrite ensures safe atomic modification and restoration of original C source files using atomic write-and-replace patterns and automatic rollback handlers.

## Alternatives Considered

### Alternative 1: GNU Linker Symbol Wrapping (`-Wl,--wrap=symbol`)
- **Pros**: Leaves original C source files untouched on disk.
- **Cons**: Only works with GNU `ld` / `gold`; fails to intercept calls to `static` functions within the same `.c` file; unsupported on macOS/Windows linkers.
- **Why not**: Fails to provide universal coverage for internal library calls and lacks cross-platform portability.

### Alternative 2: Macro Redefinition in Headers
- **Pros**: Minimal modification to build scripts.
- **Cons**: Macro token substitution conflicts with function pointer fields, struct member names, or forward declarations sharing the identifier.
- **Why not**: Fragile and produces syntax errors in real-world C headers.

## Consequences

### Positive
- Works uniformly across all C compilers (Clang, GCC, MSVC) and linkers.
- Intercepts both external calls and internal translation-unit-local invocations.
- Enables true end-to-end integration testing against the original, unmodified project test suite.

### Negative
- Temporarily modifies source files in the target repository during verification runs.
- Requires robust cleanup mechanisms to prevent leaving dirty working directories upon abnormal process interruption.

### Risks
- Process crashes could theoretically leave modified shim files in the target tree; mitigated by registering SIGINT/SIGTERM exit hooks and saving pristine `.orig` backups before modification.
