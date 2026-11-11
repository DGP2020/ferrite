# ADR-0011: Replay Provider and Offline-First Verification Architecture

**Date**: 2026-10-23
**Status**: accepted
**Deciders**: Ferrite Architecture Team

## Context

Ferrite's core engineering thesis is that the **verification engine**—differential fuzzing against C compiled with sanitizers, ASan/UBSan triage to separate translation divergences from latent C bugs, stateful L3 sequence fuzzing, and L4 hybrid C+Rust test suite execution—is the product. Requiring live LLM API keys to evaluate, test, or demonstrate the verification pipeline introduces non-determinism, API token costs, network dependencies, and significant friction for offline reviewers and CI environments.

At the same time, Ferrite must maintain a zero-friction path to live OpenAI-compatible LLM endpoints (such as Nebius Token Factory, local vLLM, or Ollama servers) without altering any downstream stage of the verification or integration pipeline.

## Decision

Establish an offline-first architecture powered by a pluggable `LlmProvider` trait and a deterministic `ReplayProvider`:
1. **Extended `LlmProvider` Trait**: Define `complete_request(&self, req: &CompletionRequest) -> anyhow::Result<Completion>`, recording `{ text, tokens, cost_usd, latency_ms }`, while preserving a backwards-compatible `complete(&self, prompt: &str)` interface.
2. **Offline-First `ReplayProvider` (Default)**: Serve human-written reference Rust translations from `fixtures/replay/<fn_name>.rs.md` (with multi-attempt support via `<fn_name>.attempt1.rs.md` for repair loop demonstrations).
3. **`OpenAiCompatProvider` Factory**: Connect to OpenAI-compatible HTTP completions endpoints via `--provider openai` or `FERRITE_PROVIDER=openai`, configurable with base URLs, API keys, models, timeouts, retries, and USD budget caps.
4. **Strict "Do Not Oversell" Attribution**: Every leaderboard, CLI banner, README table, and demo script must explicitly state when running in **Replay Mode** that translations are human-written reference fixtures, while the verification engine executes 100% for real.

## Alternatives Considered

### Alternative 1: Mandatory Live LLM API Keys for All Executions
- **Pros**: Always tests the live network path and live model inference.
- **Cons**: Tests fail when offline or when API credits are exhausted; non-deterministic CI results; impossible to run in sandboxed Docker containers (`--network none`).
- **Why not**: Creates unacceptable barriers for benchmark replication and evaluation audits.

### Alternative 2: Recorded HTTP Traffic Mocking (e.g. VCR / WireMock)
- **Pros**: Captures raw HTTP JSON payloads and headers.
- **Cons**: Bloated, opaque fixture files filled with timestamp headers and binary encodings; difficult for developers to inspect, audit, or edit individual function translations.
- **Why not**: Human-readable markdown files (`<fn_name>.rs.md`) are far more transparent, maintainable, and inspectable.

## Consequences

### Positive
- **Deterministic CI & Offline Demos**: `cargo test` and `ferrite bench` run completely offline (`--network none` in Docker) with 100% reproducible verdicts across all benchmark functions.
- **Identical Pipeline Path**: Switching from `--provider replay` to `--provider openai` changes only the injected `Arc<dyn LlmProvider>`; AST contract validation, harness generation, differential fuzzing, and HTML reporting remain completely unchanged.
- **Honest Attribution**: Clear transparency regarding the role of reference fixtures vs. live model capabilities.

### Negative
- Curating and maintaining reference `.rs.md` fixture translations for new benchmark entries requires manual effort.
- Replay provider does not test LLM latency fluctuations or token billing dynamics.

### Risks
- Users or evaluators might confuse Replay Mode with live model inference; mitigated by prominent terminal disclaimer banners and benchmark metadata labeling (`provider: replay`).
