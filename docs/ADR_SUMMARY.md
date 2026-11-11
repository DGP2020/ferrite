# Architecture Decision Records (ADR) Summary

This document indexes and summarizes the key architectural decisions that define Ferrite's system design, safety guarantees, and engineering trade-offs. Individual full records are maintained in the [`docs/adr/`](adr/README.md) directory following the Nygard format.

- **ADR Index**: [`docs/adr/README.md`](adr/README.md)
- **ADR Template**: [`docs/adr/template.md`](adr/template.md)

---

## ADR Index

| ADR ID | Title | Status | Primary Component | Decision Summary |
| :---: | :--- | :---: | :--- | :--- |
| [0001](adr/0001-pure-rust-orchestrator-with-offline-python.md) | Pure Rust Core Orchestrator with Offline Python Analysis | Accepted | Core CLI & Orchestrator | Core CLI, LLM client, and verification pipeline implemented purely in Rust; Python is restricted to offline dataset prep and router training. |
| [0002](adr/0002-clang-ast-dump-json-for-analysis.md) | Clang JSON AST Dump for C Semantic Analysis | Accepted | Semantic Analyzer | Uses `clang -Xclang -ast-dump=json` and `clang -E -dM` to parse C AST with full macro and typedef resolution without brittle libclang FFI bindings. |
| [0003](adr/0003-tri-binary-differential-triage.md) | Tri-Binary In-Process Differential Fuzzing in Linux Environment | Accepted | Verification Engine | Compiles C-only, Rust-only, and differential binaries in Linux to distinguish translation bugs from latent C vulnerabilities. |
| [0004](adr/0004-dual-layer-translation-with-template-ffi.md) | Dual-Layer Translation with Deterministic FFI Generation | Accepted | Translation Subsystem | LLM writes only idiomatic, safe Rust; `extern "C"` FFI glue is generated deterministically to avoid unsafe pointer bugs. |
| [0005](adr/0005-source-shim-injection-for-hybrid-builds.md) | Source-Level Shim Injection with Per-Library `ferrite.toml` Configuration | Accepted | Integration Subsystem | Replaces C function bodies with delegation shims calling exported Rust symbols; executes native project build/test commands. |
| [0006](adr/0006-wsl2-runtime-and-single-threaded-verification.md) | WSL2 Linux Runtime and Single-Threaded Verification Execution | Accepted | Runtime Environment | Runs inside Linux/WSL2 with a 5 GB RAM / 4 GB swap cap; concurrent LLM requests with strictly serialized heavy compilation/fuzz jobs. |
| [0007](adr/0007-phase1-signature-scope.md) | Phase 1 Signature Scope: Primitives, Slices, Strings, and Out-Pointers | Accepted | Analysis & Contracts | Targets high-frequency harnessable primitives, bounded buffer slices, strings, and scalar out-pointers. Structs/unions deferred to Phase 2. |
| [0008](adr/0008-reproduction-check-repair-loop.md) | Two-Phase Reproduction Check for Counterexample Repair | Accepted | Verifier / Repair Loop | Confirms counterexample reproduces failure before prompting model repair, and validates fix immediately before running full differential fuzzing. |
| [0009](adr/0009-ferrite-toml-manifest-schema.md) | Project Manifest Schema (`ferrite.toml`) | Accepted | Configuration & Manifest | Per-library TOML manifest defines include dirs, source files, defines, skip lists, build/test commands, and manual cluster overrides. |
| [0010](adr/0010-fenced-code-contract-syn-validation.md) | Fenced Markdown Code Block Contract with AST Signature Enforcement | Accepted | Translation & Parser | Requires models to reply in fenced ````rust```` blocks; parses output with `syn` to validate planned signature contract and audit `unsafe` usage. |
| [0011](adr/0011-replay-provider-and-offline-first.md) | Replay Provider and Offline-First Verification Architecture | Accepted | Provider & Benchmark | Serves deterministic reference Rust translations from `fixtures/replay/` by default while supporting OpenAI-compatible HTTP endpoints via `FERRITE_PROVIDER=openai`. |

---

## Detailed Records Breakdown

### ADR 0001: Pure Rust Core Orchestrator
- **Context**: Multi-language orchestrators (e.g. Python scripts wrapping Rust binaries) cause Python environment drift, packaging friction, and fragile distribution.
- **Decision**: All runtime components (CLI, LLM provider client, analyzer, scheduler, harness runner, dashboard generator) are pure Rust. Python is strictly an offline utility for model evaluation and router training.
- **Consequences**: Single zero-dependency binary distribution, predictable memory usage, and zero runtime Python dependency.

### ADR 0002: Clang JSON AST Dump for C Semantic Analysis
- **Context**: Tree-sitter cannot resolve preprocessor macros or cross-header typedefs. Conversely, linking directly against `libclang` via FFI requires local LLVM development headers and introduces fragile cross-compilation linking issues.
- **Decision**: Invoke the external `clang` binary with `-Xclang -ast-dump=json` and deserialize the resulting AST via `serde_json`, supplementing with `clang -E -dM` for macro expansion.
- **Consequences**: Accurate C type resolution without dynamic library linking headaches.

### ADR 0003: Tri-Binary In-Process Differential Fuzzing in Linux
- **Context**: Subprocess IPC introduces unacceptable overhead during differential fuzzing campaigns. Furthermore, AddressSanitizer and `cargo-fuzz` lack parity on Windows.
- **Decision**: Run verification under Linux (WSL2 or Docker). For each function, build three binaries (C-only, Rust-only, and unified differential) to isolate whether a failure is a translation divergence, a Rust runtime panic, or a latent memory defect in the C original.
- **Consequences**: High fuzz throughput ($>10,000$ execs/sec) and unambiguous triage between C bugs and Rust translation errors.

### ADR 0004: Dual-Layer Translation with Deterministic FFI Generation
- **Context**: Allowing the LLM to write C-ABI signatures produces unidiomatic, raw-pointer-polluted Rust. Manual unsafe lifting after the fact is error-prone.
- **Decision**: Restrict the LLM to pure, idiomatic Rust. Ferrite's code generator creates the companion `extern "C"` FFI wrapper that handles pointer conversions, length checks, and return marshaling.
- **Consequences**: 100% safe Rust core logic; unsafe FFI boundaries are confined to battle-tested templates.

### ADR 0005: Source-Level Shim Injection for Hybrid Builds
- **Context**: Linker wrapping (`--wrap`) fails on internal static function calls within the same translation unit. Archive symbol overriding behaves inconsistently across toolchains.
- **Decision**: Replace original C function bodies with explicit delegation shims calling exported Rust symbols (`ferrite_<name>`), using configuration options in `ferrite.toml`.
- **Consequences**: Works uniformly across compilers, preserves project build order, and cleanly enables L4 test suite integration.

### ADR 0006: WSL2 Linux Runtime and Single-Threaded Verification
- **Context**: Cross-filesystem file I/O (e.g., from `/mnt/c`) degrades compilation speed, and host memory on developer machines may be limited.
- **Decision**: Execute inside native Linux filesystem (`~/ferrite`) with a 5 GB RAM / 4 GB swap quota. LLM API calls may run concurrently, but compilation and fuzzing jobs are strictly serialized with a 1 GB RSS limit.
- **Consequences**: Eliminates OOM crashes and cross-filesystem I/O penalties.

### ADR 0007: Phase 1 Signature Scope
- **Context**: Attempting to support every C construct (complex unions, bitfields, varargs) upfront delays delivering a working verification loop.
- **Decision**: Phase 1 scope is strictly primitives, slices, strings, and scalar out-pointers. Unsupported constructs are flagged as unhandled.
- **Consequences**: Rapidly validated core pipeline with zero ambiguity over harness generation feasibility.

### ADR 0008: Two-Phase Reproduction Check for Counterexample Repair
- **Context**: Generating repairs for unverified counterexamples wastes tokens and CPU cycles.
- **Decision**: Ferrite compiles and runs a standalone reproduction test before calling the repair LLM, and verifies the fix immediately before running full differential fuzzing.
- **Consequences**: Minimizes wasted repair cycles and prevents prompt thrashing.

### ADR 0009: Project Manifest Schema (`ferrite.toml`)
- **Context**: Build systems vary widely across C projects; attempting automatic inference is fragile.
- **Decision**: Standardize on a clear, human-editable `ferrite.toml` manifest specifying library metadata, flags, include paths, skip lists, and cluster definitions.
- **Consequences**: Predictable builds and zero reliance on reverse-engineering custom Makefiles.

### ADR 0010: Fenced Markdown Code Block Contract with AST Signature Enforcement
- **Context**: JSON-escaped code blocks frequently cause syntax rejections or malformed escapes across LLM providers.
- **Decision**: Models return plain markdown fenced ````rust```` blocks. Output is parsed by `syn` to verify that the AST strictly matches the expected contract and to audit `unsafe` usage.
- **Consequences**: Provider-agnostic compatibility and guaranteed AST conformance before compiler invocation.

### ADR 0011: Replay Provider and Offline-First Verification Architecture
- **Context**: Requiring live LLM API keys to evaluate or demo the verification engine introduces non-determinism, API costs, and network barriers.
- **Decision**: Default to `ReplayProvider` (loading reference Rust from `fixtures/replay/<fn_name>.rs.md`) while providing `OpenAiCompatProvider` (`--provider openai`) behind the same `LlmProvider` trait, with clear attribution that replay translations are human-written.
- **Consequences**: 100% deterministic offline verification and benchmarking (`--network none`), with zero pipeline changes when switching to a live model.
