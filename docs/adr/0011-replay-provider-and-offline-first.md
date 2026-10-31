# Replay Provider and Offline-First Verification Architecture

- **Status**: Accepted
- **Date**: 2026-10-23
- **Component**: Provider Layer (`src/provider.rs`) & Benchmark Runner (`src/bench.rs`)

## Context

Ferrite's core thesis is that the **verification engine**—differential fuzzing against C compiled with sanitizers, ASan/UBSan triage to separate translation divergences from latent C bugs, stateful L3 sequence fuzzing, and L4 hybrid C+Rust test suite execution—is the product. Requiring live LLM API keys to evaluate, test, or demonstrate the verification pipeline introduces non-determinism, API costs, network dependencies, and barriers for offline reviewers.

At the same time, Ferrite must maintain a zero-friction path to live OpenAI-compatible LLM endpoints (such as Nebius Token Factory / Nemotron tiers or local vLLM servers) without altering any downstream stage of the pipeline.

## Decision

1. **Extended `LlmProvider` Trait**:
   - Extend `LlmProvider` with `complete_request(&self, req: &CompletionRequest) -> anyhow::Result<Completion>`, where `Completion` records `{ text, tokens, cost_usd, latency_ms }`, while preserving a backwards-compatible `complete(&self, prompt: &str)` shim.
2. **Offline-First `ReplayProvider`**:
   - Serve human-written reference Rust translations from `fixtures/replay/<fn_name>.rs.md` (with multi-attempt support via `<fn_name>.attempt1.rs.md` for repair loop demonstrations).
   - Missing fixture files fail immediately with a descriptive error naming the expected `.rs.md` path.
3. **`OpenAiCompatProvider` & `provider_from_env()` Factory**:
   - Provide an OpenAI-compatible HTTP client (`POST {base}/chat/completions`) with configurable timeout, retries, and USD budget cap, selectable via `--provider openai` or `FERRITE_PROVIDER=openai` (`FERRITE_API_BASE`, `FERRITE_API_KEY`, `FERRITE_MODEL`).
4. **Strict "Do Not Oversell" Attribution**:
   - Every leaderboard, CLI banner, README table, and demo script must explicitly state when running in **Replay Mode** that translations are human-written reference fixtures and not live LLM outputs, while the verification engine executes 100% for real.

## Consequences

- **Deterministic CI & Offline Demos**: `cargo test` and `ferrite bench` run completely offline (`--network none` in Docker) with 100% reproducible verdicts across all 34 benchmark functions (`Equivalent`, `Diverged`, and `CLatentBug`).
- **Identical Pipeline Path**: Switching from `--provider replay` to `--provider openai` changes only the `Arc<dyn LlmProvider>` injected at startup; AST contract validation (`syn`), harness generation, differential fuzzing, and HTML reporting remain untouched.
