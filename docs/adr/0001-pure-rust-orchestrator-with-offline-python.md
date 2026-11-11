# ADR-0001: Pure Rust Core Orchestrator with Offline Python Analysis

**Date**: 2026-10-17
**Status**: accepted
**Deciders**: Ferrite Architecture Team

## Context

Ferrite requires an orchestrator that coordinates C source analysis, LLM translation requests, in-process compilation, differential fuzzing, and interactive reporting. Multi-language or Python-driven orchestrators (such as Python CLI wrappers invoking Rust binaries via subcommands or PyO3) introduce runtime dependency drift, virtualenv and packaging friction, slow cold startup, and distribution hurdles across heterogeneous developer environments (WSL2, Docker, native Linux).

Conversely, Python remains the dominant ecosystem for exploratory data science, statistical analysis (bootstrap confidence intervals), and machine learning router training. We need an architectural separation that provides zero-dependency binary distribution while retaining Python's strength for offline modeling.

## Decision

All runtime components—including the CLI entrypoint, provider abstraction, C AST parsing via Clang subprocess, topological planner, harness generator, verification triage engine, and terminal/HTML dashboard—are implemented purely in Rust as a single standalone executable.

Python is strictly restricted to offline tasks: benchmark dataset curation, offline multi-model evaluation analysis, router model training, and research experiments. Python exports its learned decision tree or routing parameters as static JSON artifacts (`router_table.json`) which the Rust runtime consumes at startup.

## Alternatives Considered

### Alternative 1: Python-First CLI with Rust Native Extension (PyO3)
- **Pros**: Rapid prototyping of prompt chaining; seamless integration with scikit-learn and PyTorch.
- **Cons**: Requires users to configure a Python 3.10+ runtime, pip dependencies, and matching platform wheels; dynamic linking failures across diverse glibc/musl distributions.
- **Why not**: Breaks the requirement for a single zero-dependency binary distribution that runs reliably inside minimal Docker containers and developer workstations.

### Alternative 2: Shell Script Wrapper Coordinating Independent Binaries
- **Pros**: Minimal coupling between separate C, Rust, and Python executables.
- **Cons**: Brittle process management, weak error handling, platform portability issues (Windows PowerShell vs. Linux bash), and high process creation overhead.
- **Why not**: Subprocess orchestration of every pipeline phase impedes high-throughput differential verification and robust error recovery.

## Consequences

### Positive
- Single standalone static binary (`ferrite`) with sub-second startup and low memory footprint.
- Zero runtime Python dependencies on end-user machines or CI verification runners.
- Shared compile-time type safety across domain models (`MigrationUnit`, `Verdict`, `AssuranceLevel`).

### Negative
- Higher initial development effort for HTTP provider adapters, JSON schemas, and HTML dashboard generation in Rust compared to Python.
- Router model complexity is constrained to architectures that can be serialized to JSON and evaluated without Python runtime dependencies.

### Risks
- Feature drift between offline Python feature extraction scripts and Rust analyzer logic; mitigated by sharing test fixtures in `fixtures/` and validating feature vector schema parity in CI.
