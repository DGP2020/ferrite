# ADR-0002: Clang JSON AST Dump for C Semantic Analysis

**Date**: 2026-10-18
**Status**: accepted
**Deciders**: Ferrite Architecture Team

## Context

Accurate C source analysis is required to extract function boundaries, parameter types, pointer indirection depths, array lengths, struct dependencies, and preprocessor definitions. Without accurate semantic information, Ferrite cannot generate safe Rust signature contracts or in-process FFI shims.

Tree-sitter provides fast syntactic parsing but operates purely at the lexical grammar level without expanding `#include` or `#define` directives, leaving typedefs unresolved and macros unexpanded. Conversely, linking directly against `libclang` via FFI crates (e.g. `clang-sys`) requires local LLVM development headers and specific shared libraries (`libclang.so`/`libclang.dll`), causing brittle platform-specific compilation and linking failures.

## Decision

Ferrite executes the standard external `clang` executable with `-Xclang -ast-dump=json` (filtered to translation-unit-local nodes) and deserializes the resulting AST using `serde_json`. Preprocessor macro definitions are captured separately via `clang -E -dM`.

The deserialized JSON AST is inspected to construct function signatures, detect pointer-buffer pairs, and identify unsupported language features (such as inline assembly or complex bitfields).

## Alternatives Considered

### Alternative 1: Tree-sitter C Parser
- **Pros**: Pure in-process Rust binding with zero external compiler dependencies; fast incremental parsing.
- **Cons**: Cannot perform preprocessor macro expansion, typedef resolution across system headers, or struct layout calculation.
- **Why not**: Generates incomplete or ambiguous type contracts for C codebases that rely on typedefs (e.g., `uint32_t`, `size_t`, domain typedefs), which causes downstream Rust compilation failures.

### Alternative 2: In-Process `libclang` C Bindings (`clang-sys`)
- **Pros**: Rich in-memory Clang AST traversal with native compiler fidelity.
- **Cons**: Requires `libclang` dynamic library and LLVM C-header development packages installed on the host system; prone to version mismatches and cross-compilation errors.
- **Why not**: Substantially degrades ease of installation and developer onboarding, conflicting with zero-friction distribution.

## Consequences

### Positive
- Zero native C library linking requirements; any system with a standard `clang` binary in `PATH` is supported.
- Complete fidelity with real Clang preprocessing, macro expansions, and header typedef resolutions.
- Robust JSON schema deserialization via `serde` with graceful handling of unmodeled AST nodes.

### Negative
- Process spawn overhead for `clang` per translation unit (typically 50–200 ms).
- AST JSON output can be verbose for headers with deep standard library includes, requiring local filtering.

### Risks
- Clang AST JSON representation may shift across major LLVM versions; mitigated by pinning schema expectations to Clang 14+ compatible node types and testing against multiple LLVM versions in CI.
