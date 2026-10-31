# Ferrite

Verified C-to-Rust migration agent.

Ferrite migrates C code to safe Rust one function at a time, and only accepts
a translation after it survives differential fuzzing against the original C.
Along the way it surfaces memory bugs that were latent in the C.

The LLM proposes. A deterministic engine (compiler, sanitizers, fuzzers, the
project's own tests) disposes. Every claim on screen ("verified", "diverged",
"latent C bug") comes from execution, not from a model's opinion.

[![Rust CI](https://github.com/DGP2020/ferrite/actions/workflows/ci.yml/badge.svg)](https://github.com/DGP2020/ferrite/actions/workflows/ci.yml)
[![Release](https://github.com/DGP2020/ferrite/actions/workflows/release.yml/badge.svg?branch=main)](https://github.com/DGP2020/ferrite/actions/workflows/release.yml)
## Non-goal & Attribution Rule

Fuzzing gives **strong evidence of equivalence, not a formal proof**. We say
"differentially tested" and show execution counts and coverage. We do not
oversell.

> [!IMPORTANT]
> **Replay Mode vs. Live Model Mode (ADR 0011)**:
> By default (`--provider replay`), Ferrite loads **human-written reference Rust translations** from `fixtures/replay/<fn_name>.rs.md` so the verification engine, sanitizer triage, stateful L3 fuzzer, and L4 hybrid C+Rust builder can be evaluated 100% deterministically and offline without API keys. Replay results are **not** LLM model outputs. To evaluate a live OpenAI-compatible LLM endpoint, pass `--provider openai` (or set `FERRITE_PROVIDER=openai`).

## Ferrite-Bench Lite Results (Replay Mode)

Running `cargo run -- bench --manifest bench/manifest.toml --provider replay` executes the real verification engine across all **34 benchmark functions** (10,012 differential executions per function):

| Suite / Library | Tier | Functions | Compile Rate (`L0`) | Verified (`L2`/`L3`/`L4`) | `CLatentBug` Surfaced | Expected Verdict Match |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| `tier1_scalars` (`clamp_i32`, `gcd_u32`, `popcount_u32`, ...) | Tier 1 | 9 | 9 / 9 (100%) | 9 (`L2`) | 0 | 9 / 9 |
| `tier1_strings` (`cstr_len_bounded`, `count_char_ascii`, ...) | Tier 1 | 3 | 3 / 3 (100%) | 3 (`L2`) | 0 | 3 / 3 |
| `tier2_buffers` (`sum_bytes`, `fnv1a_hash32`, `reverse_bytes_in_place`, ...) | Tier 2 | 7 | 7 / 7 (100%) | 7 (`L2`) | 0 | 7 / 7 |
| `tier2_parsers` (`parse_u16_dec`, `hex_nibble`, `count_kv_lines`) | Tier 2 | 3 | 3 / 3 (100%) | 3 (`L2`) | 0 | 3 / 3 |
| `tier3_stateful` (`ringbuf_next_idx`, `ringbuf_used`, `token_bucket_step`, ...) | Tier 3 | 5 | 5 / 5 (100%) | 5 (`L2` + `L3` ringbuf) | 0 | 5 / 5 |
| `planted_bugs` (`bug_off_by_one_read`, `bug_signed_overflow_abs`, `bug_oob_write_reset`, `bug_empty_slice_head`) | Tier 1–2 | 4 | 4 / 4 (100%) | 4 (`L2` triaged) | **4** | 4 / 4 |
| `lib_checksum` (`adler32_step`, `crc32_byte`, `crc32_buf`) | Tier 1–2 | 3 | 3 / 3 (100%) | 3 (`L2` + **`L4`** hybrid C test suite) | 0 | 3 / 3 |
| **Total (Replay Mode)** | **T1–T3** | **34** | **34 / 34 (100%)** | **34 / 34 (100%)** | **4** | **34 / 34 (100%)** |

## Replay Mode vs. Live Model Mode

Ferrite's `LlmProvider` trait (`src/provider.rs`) decouples translation synthesis from the deterministic verification engine:

1. **Replay Mode (`--provider replay`, default)**:
   - Loads `fixtures/replay/<fn_name>.rs.md`.
   - Zero network calls, zero API cost, 100% deterministic CI & Docker execution (`--network none`).
2. **Mock Mode (`--provider mock`)**:
   - In-memory canned responses for fast unit tests.
3. **OpenAI-Compatible HTTP Mode (`--provider openai`)**:
   - Sends `POST {FERRITE_API_BASE}/chat/completions` with retries, wall-clock timeout, and USD budget cap.
   - Activated via CLI or environment variables (`.env.example`):
     ```bash
     export FERRITE_PROVIDER=openai
     export FERRITE_API_BASE=https://api.studio.nebius.ai/v1
     export FERRITE_API_KEY=your-api-key
     export FERRITE_MODEL=nvidia/Llama-3.3-Nemotron-Super-49B-v1
     cargo run -- --source-dir fixtures/clamp --provider openai
     ```

## Architecture & Documentation

Comprehensive technical documentation is maintained in the [`docs/`](docs/README.md) directory:
- [System Architecture](docs/ARCHITECTURE.md) — 7-stage pipeline, component breakdown, and AST contracts.
- [Verification Engine](docs/VERIFICATION_ENGINE.md) — Tri-binary triage, differential fuzzing, and latent bug classification.
- [Specification Guide](docs/SPECIFICATION.md) — `ferrite.toml` manifest schema, CLI reference, and signature scopes.
- [Demo & Video Walkthrough Script](docs/DEMO.md) — 3–5 minute end-to-end demonstration script.
- [Architecture Decision Records (ADRs)](docs/ADR_SUMMARY.md) — Index of ADRs 0001 through 0011 (including [ADR 0011: Replay Provider and Offline-First Architecture](docs/adr/0011-replay-provider-and-offline-first.md)).
- [Ferrite v2 Roadmap & Plan](ferrite-v2-plan.md) — 75-day project roadmap, Ferrite-Bench specification, and evaluation metrics.

Pipeline stages:

1. **Analyze** — Clang JSON AST dump & lexical analyzer extract functions, call graph, and risk features (ADR 0002).
2. **Plan** — Bottom-up topological ordering and ownership/contract hints.
3. **Translate** — Dual-layer generation: idiomatic safe Rust + deterministic FFI companion; `syn` AST validation (ADR 0004, ADR 0010).
4. **Seed** — Edge-case scalar boundaries, empty/canary-protected slices, and NUL strings.
5. **Verify** — Differential fuzzing under sanitizers/canaries with triage (`Equivalent`, `Diverged`, `CLatentBug`) and L3 stateful API sequence fuzzing (`src/verify/stateful.rs`) (ADR 0003, ADR 0008).
6. **Integrate** — Source-level shim injection (`ferrite_<name>`) and L4 project test execution (`src/integrate.rs`, `bench/lib_checksum`) (ADR 0005, ADR 0009).
7. **Report** — Terminal overview, `bench-results.json`, and standalone interactive HTML dashboard (`dashboard.html`).

## Quick Start

```powershell
# 1. Run the single-function vertical slice (Replay Mode)
cargo run -- --source-dir fixtures/clamp --provider replay

# 2. Run the full 34-function Ferrite-Bench Lite suite
cargo run -- bench --manifest bench/manifest.toml --provider replay

# 3. Run the entire test suite (including L3 ring buffer & L4 lib_checksum hybrid build)
cargo test

# 4. Run inside isolated Linux Docker container (--network none)
./scripts/ferrite-docker.ps1 -Mode all
```

## License

Apache 2.0. See [LICENSE](LICENSE).
