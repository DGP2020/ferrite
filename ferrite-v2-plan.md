# Ferrite v2: Model-Agnostic Verified C-to-Rust Migration

**Window:** Sat Oct 17 to Wed Dec 30, 2026 (75 days, about 10.5 weeks). Features freeze Dec 15, buffer Dec 25-30.
**Assumption:** 1-2 people at student pace (roughly 15-20 hours per week each). If your December has exams, the plan is front-loaded and Phase 4 is the most compressible.

---

## 1. What changed from v1

- **No vendor lock-in.** Any hosted or local model works behind one provider interface. Results are reported *per model*, which turns the project into a benchmark as well as a tool.
- **No hackathon limits.** The stretch items from v1 (stateful fuzzing, router, c2rust comparison) are now core. A research track adds a fine-tuning spike and bounded formal checks.
- **The router has a real dataset.** With about 120+ benchmark functions across 6+ models, a learned router is credible instead of an overfit toy.

## 2. Definition of done (targets, not promises)

1. CLI and dashboard that migrate a C project function by function, with every function labeled by the **highest assurance level** it reached (section 4).
2. **Ferrite-Bench:** 120+ C functions in four difficulty tiers, with planted-bug cases and held-out libraries.
3. At least **3 real C libraries** migrated into a hybrid build where the original tests still pass.
4. A multi-model evaluation (6+ models, mix of hosted and open-weight) with ablations.
5. A **technical report** (6-8 pages), a polished README, and a 3-5 minute demo video.
6. Open-source release (Apache-2.0 or MIT).

## 3. Positioning (what makes it not a wrapper)

| Approach | Weakness | Ferrite |
|---|---|---|
| `c2rust` | Faithful but fills the output with `unsafe` | Verified lifting from unsafe to safe, with re-verification at each step |
| Single-shot LLM translation | Plausible code, no evidence it is correct | Compile, differential fuzz and test gates; counterexample-driven repair |
| LLM test generation | Model grades its own work | Sanitizers, fuzzers and original tests are the judge |

Do a one-day literature scan in Phase 0 (c2rust, DARPA's TRACTOR program, recent LLM-based C-to-Rust research) and cite what you build on. Your novelty claim should be specific: verifier-in-the-loop with latent-bug triage, plus a multi-model benchmark.

Wording rule: fuzzing is **strong evidence, not proof**. Use "differentially tested" and report executions and coverage.

---

## 4. Assurance ladder

Each function ends at the highest level it reaches. The dashboard shows the distribution.

| Level | Check |
|---|---|
| L0 | Compiles, clippy clean |
| L1 | Passes unit tests derived from the original's tests or examples |
| L2 | Differential fuzzing against the C original (coverage plateau reached) |
| L3 | Stateful API-sequence fuzzing against an opaque handle (for init/use/free style APIs) |
| L4 | The project's **original test suite** passes against the hybrid C+Rust build |
| L5 (research) | Bounded symbolic equivalence on small pure functions, if a toolchain works (see R1) |

---

## 5. Architecture

```
 C project (+ compile_commands.json)
    |
    v
[1 ANALYZE] -> [2 PLAN] -> [3 TRANSLATE] -> [4 VERIFY] -> [5 LIFT] -> [6 INTEGRATE] -> [7 REPORT]
 clang/tree-    LLM:        workers,         assurance     unsafe ->    Rust crate with   dashboard,
 sitter, call   order,      parallel,        ladder        safe, each   extern "C",       CLI, git
 graph, macro   ownership,  idiom-memory     (sect. 4)     step         linked to C,      branch with
 table,         type        few-shot                       re-verified  project tests     per-function
 features       mapping                                                 run               commits
                    ^            |              |
                    |            v              | counterexamples, compiler errors,
                    +------ [MODEL POLICY] <----+ sanitizer reports
                            static / cascade / predictive
                                 |
                          [PROVIDER LAYER]
              OpenAI-compatible | Anthropic | Google | local (vLLM, Ollama, llama.cpp)

 Shared: run DB, LLM cache, rootless sandbox, budget caps, job queue
```

### Core components

**Provider layer**
- One interface: `complete(messages, schema=None) -> {text, usage, cost}`. No vendor-specific features in core logic.
- Adapters: OpenAI-compatible (covers many hosted services and local servers), Anthropic, Google. A library like LiteLLM can save time; a thin custom adapter is more educational and easier to explain.
- Model registry (YAML): context length, price, structured-output support, temperature support.
- Retries and rate limits, per-run **budget caps**, disk cache keyed by (model, prompt, params), JSON-schema validation with auto-repair.

**Analyzer**
- Uses `compile_commands.json` so multi-file projects and preprocessor output work.
- Extracts functions, call graph, struct and enum layouts, globals, macros, and risk features: pointer arithmetic, malloc/free pairs, buffer+length pairs, unions, bitfields, function pointers, varargs, setjmp, inline asm.
- Marks unsupported constructs (varargs, setjmp, inline asm) as **flagged, not translated**, and says so honestly in the report.

**Planner**
- Bottom-up order from the call graph. Per function: ownership guess for each pointer (owned, borrowed, slice, out-param), error-handling strategy (errno or return codes to `Result`), type mapping.

**Translator**
- Per-function workers, with callee signatures and struct definitions in context.
- Prompt carries a checklist of known C-to-Rust semantic traps: signed overflow is UB in C but panics or wraps in Rust (choose `wrapping_*` or `checked_*` deliberately), NUL-terminated strings, pointer arithmetic to slice indices, `repr(C)` layout where it crosses the FFI boundary.
- **Idiom memory:** only *verified* pairs are stored (embedded and retrieved as few-shot examples). Measure its uplift with an ablation.

**Verification engine**
1. Compile gate (cargo check, clippy).
2. Harness generator: deterministic templates for common signatures (scalars, strings, buffers, out-params), an LLM only for unusual ones. FFI shim around the original C compiled with ASan and UBSan; `cargo-fuzz` target using `arbitrary`.
3. Differential run with LLM-generated seed corpus; compare returns, buffers, out-params.
4. **Triage:** on mismatch, rerun on the C side under sanitizers.
   - C reports UB or a memory error: verdict `C_LATENT_BUG`, saved as a finding.
   - C is clean: `DIVERGED`; minimize the input and send it to repair.
5. Stateful mode (L3): generate operation sequences against an opaque handle and compare observable state after each operation.
6. Confidence record: executions, edge coverage on both sides, plateau detection, Miri run.
7. Harness quality check: `cargo-mutants` on the translation; the harness must kill the mutants.
8. Performance guard: `criterion` microbenchmarks, Rust against C on the same inputs; flag regressions beyond a configurable threshold.

**Latent-bug reporter** (your cybersecurity angle)
- Turns sanitizer output into a structured finding: function, minimized input, stack hash for dedupe, and a CWE guess (for example CWE-787 out-of-bounds write, CWE-125 out-of-bounds read, CWE-416 use after free, CWE-190 integer overflow).
- If you find a genuine bug in a real library, follow responsible disclosure: report to the maintainers privately first.

**Unsafe lifter**
- Start from a faithful translation (possibly with `unsafe`, or `c2rust` output as an optional front end), then apply one lifting step at a time (raw pointer to slice, `malloc` to `Vec`, and so on). Every step must re-pass L2. This is a **verified refactoring loop**: a regression is caught immediately.
- c2rust can be painful to install (specific LLVM versions), so treat it as optional.

**Model policy and router**
```python
class ModelPolicy:
    def select(self, fn, stage, history) -> ModelId: ...
```
- `StaticPolicy`: fixed model per stage.
- `CascadePolicy`: cheapest viable first, escalate when verification fails. The verifier provides free labels.
- `PredictivePolicy`: features to minimum model likely to pass. Train on benchmark sweeps; evaluate with **leave-one-library-out** cross-validation to avoid leakage.

**Interfaces**
- CLI: `ferrite migrate ./proj --policy cascade --models cheap,mid,strong`
- Web dashboard: per-function cards, assurance level, counterexamples, diffs, cost.
- Git integration: one branch, one commit per verified function, with the evidence in the commit message. Optional human review step before accept.

---

## 6. Benchmark and evaluation methodology

**Ferrite-Bench**
- **Tier 1:** pure scalar and string utilities
- **Tier 2:** buffer manipulation and small parsers
- **Tier 3:** pointer-heavy and stateful (ring buffer, linked list, arena allocator)
- **Tier 4:** whole small libraries (INI parser, JSON subset, checksum or compression subset)
- **Planted-bug set:** off-by-one, use-after-free, integer overflow, written by you
- **Splits:** dev and test split **by library**, never by function, to prevent leakage
- Use only permissively licensed C sources, and keep the license notices.

**Models:** 6+, covering hosted frontier, hosted small, and 2-3 open-weight coder models (Nemotron is welcome as one entry), at least one run locally if hardware allows.

**Baselines:** single-shot whole-file translation; single-shot per-function with no verification (shows what the verifier adds); c2rust (optional).

**Ablations (these impress reviewers):** no counterexample repair, no LLM seeding, no triage, no idiom memory, static vs cascade vs predictive routing.

**Metrics:** compile rate, assurance-level distribution, unsafe lines before and after, coverage, latent bugs found, performance ratio, cost and time per verified function. Report bootstrap confidence intervals and repeat sampling-based runs.

---

## 7. Optional research tracks (pick at most two)

- **R1: Bounded equivalence for pure functions.** Try compiling C and Rust to LLVM IR and checking equivalence symbolically (for example with KLEE-style execution), or CBMC on the C side with Kani on the Rust side against shared test vectors. LLVM version mismatches between toolchains are a real risk, so spike it for **two days** and drop it if it does not work.
- **R2: Distillation on verified pairs.** Your verifier produces a clean dataset of (C, Rust) pairs. LoRA fine-tune a small open coder model on pairs from the dev libraries and test on held-out libraries. Needs a GPU (rented, cloud credits, or Colab). Time-box to 1.5 weeks.
- **R3: Embedded showcase (fits hardware interests, no hardware needed).** Migrate a small embedded-style C driver (volatile register access, bit manipulation, a ring buffer) to `no_std` Rust, fuzzed against a mock register file.
- **R4: CI mode.** A GitHub Action that re-runs equivalence checks on pull requests to a migrated project.

---

## 8. Roadmap

| Phase | Dates | Work | Exit criteria |
|---|---|---|---|
| **0 Foundations** | Oct 17-23 | Repo and license; provider layer with 3 adapters; Docker toolchain; hand-written C/Rust pair with a working differential harness; literature scan | Harness catches a planted bug; 3 backends respond |
| **1 Core pipeline** | Oct 24-Nov 6 | Analyzer, translator, compile-repair loop, harness generator, fuzz runner, verdict classifier, ASan triage | **M1:** 20 functions verified automatically |
| **2 Scale and structure** | Nov 7-20 | Orchestrator, bottom-up order, run DB, cache, multi-file and preprocessor handling, struct/enum/union mapping, Benchmark v1 (60 functions) | **M2:** a ~500-1000 line library migrated, original tests pass (L4) |
| **3 Depth** | Nov 21-Dec 4 | Stateful fuzzing (L3), latent-bug reporter, unsafe lifter, performance guard, dashboard and CLI | **M3:** all assurance levels L0-L4 working end to end |
| **4 Models and routing** | Dec 5-14 | Full multi-model sweep, cascade and predictive router, idiom memory; one research track (R2 or R3 or R1) | **M4:** leaderboard, Pareto chart, ablation tables |
| **5 Freeze and write-up** | Dec 15-24 | **Feature freeze Dec 15.** Final runs, statistics, technical report, README, demo video | Report and video done |
| **Buffer** | Dec 25-30 | Fix, polish, release | Nothing new |

Weekly rhythm: Monday plan, Friday demo to each other (or to yourself on video). A running log of decisions and failures becomes most of your technical report.

---

## 9. Risks and cut list

| Risk | Mitigation |
|---|---|
| Harness generation is harder than translation | Phase 0-1 priority; restrict v1 to harnessable signatures |
| API costs | Cache everything, cheap models for bulk, per-run budget caps, small-sample dry runs before full sweeps |
| Toolchain pain (c2rust, KLEE, Kani) | Spike with a 2-day time box, all optional |
| Benchmark leakage or contamination | Split by library, prefer less famous libraries, note the contamination risk in the report |
| Scope creep | Freeze Dec 15; follow the cut list |
| Exams or busy weeks | Phase 4 is compressible; research tracks are optional |

**Cut order if behind:** research tracks, predictive router, idiom memory, unsafe lifter, performance guard, CI mode. Never cut: verification engine, benchmark, evaluation, write-up.

## 10. Safety and licensing notes

- Compiling and running arbitrary C is dangerous. Always use a rootless container with no network, resource limits and timeouts. Do not host a public endpoint that runs uploaded C; publish recorded runs and bundled examples only.
- Respect licenses of benchmark libraries and include attribution.
- Do not claim zero bugs or full equivalence anywhere, in the README, report or video.

## 11. Showcase checklist

- [ ] README with a results table and a 60-second quickstart
- [ ] Technical report (arXiv-style PDF)
- [ ] Demo video (3-5 minutes) showing a migration run, a counterexample repair, and a latent-bug finding
- [ ] Leaderboard page for models on Ferrite-Bench
- [ ] Tagged release and license file
