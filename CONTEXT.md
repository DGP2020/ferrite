# Ferrite Domain Context

Ferrite is an automated, model-agnostic C-to-Rust migration agent that verifies translations via differential fuzzing and ASan/UBSan triage.

## Language

**MigrationUnit**:
The atomic unit of planning, translation, and verification, containing one or more related C functions and their associated types. Single functions for pure code; clusters for functions sharing structs or state.
_Avoid_: FunctionUnit, WorkItem, Task

**Dual-Layer Translation**:
The architecture where the LLM produces an idiomatic, memory-safe Rust function, accompanied by a deterministically generated `extern "C"` FFI wrapper that bridges C callers to Rust.
_Avoid_: Direct translation, C-style Rust

**Signature Contract**:
The exact Rust function signature generated deterministically by the analyzer/planner and passed into the LLM prompt, validated against model output using `syn` prior to compilation.
_Avoid_: Function spec, type guess

**Differential Harness**:
A unified in-process test binary linking both C and Rust implementations, fed identical structured inputs via `arbitrary` to detect execution divergences.
_Avoid_: Comparative fuzzer, dual test

**Persistent Harness Crate**:
A dedicated, pre-compiled Rust crate within the target workspace to which new differential fuzz targets are added, amortizing dependency compilation costs across verification runs.
_Avoid_: Temp harness, ad-hoc crate

**Tri-Binary Triage**:
The execution strategy compiling three distinct binaries (C-only, Rust-only, and Differential) to isolate whether a failure is a translation divergence, a Rust bug, or a latent bug in the original C codebase.
_Avoid_: ASan run, crash analysis

**C Latent Bug**:
A memory-safety or undefined-behavior defect present in the original C implementation that is triggered during differential testing under sanitizers, where Rust handles the input safely.
_Avoid_: False positive, divergence, C bug

**Reproduction Check**:
The verification step that executes a synthesized counterexample test before sending it to the repair prompt (confirming failure) and immediately after repair (confirming fix before full differential re-fuzzing).
_Avoid_: Sanity check, dry run

**Source Shim**:
A transformed C function body that delegates execution directly to the exported Rust companion symbol (`ferrite_<name>`), preserving the original project's build hierarchy.
_Avoid_: Wrapper, stub, trampoline

**Clang AST Dump**:
The JSON representation of the Clang AST extracted via `clang -Xclang -ast-dump=json` and filtered to translation-unit-local nodes, supplemented by `clang -E -dM` for macros.
_Avoid_: Tree-sitter tree, Clang FFI

**Assurance Level**:
The discrete verification tier reached by a MigrationUnit (L0: compiles/clippy, L1: unit tests pass, L2: differential fuzzing clean, L3: stateful API sequence fuzzing clean, L4: original test suite passes).
_Avoid_: Confidence score, test grade
