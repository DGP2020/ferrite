# Ferrite 3–5 Minute Demo Script (Offline-First Replay Mode)

> [!IMPORTANT]
> **Attribution Notice ("Do Not Oversell" Rule)**:
> In `--provider replay` mode, translations are human-written reference Rust loaded from `fixtures/replay/<fn_name>.rs.md`, **not** live LLM output. The C analyzer, `syn` AST contract validator, FFI/differential harness generator, compiler, sanitizer triage, L3 stateful fuzzer, and L4 hybrid C+Rust test suite runner all execute **for real**.

---

## Overview & Timing (Total: ~4 Minutes)

| Segment | Duration | Focus | Key Verdict / Level |
| :--- | :---: | :--- | :--- |
| **1. Intro & Offline-First Architecture** | 0:45 | Explain "The LLM proposes; the deterministic verification engine disposes" and Replay Mode vs. Live Model Mode. | Architecture overview |
| **2. Clean Migration (`clamp` & `lib_checksum`)** | 1:00 | Run Ferrite on `fixtures/clamp` and `bench/lib_checksum`, showing L2 differential fuzzing (10,012 execs) and L4 hybrid C+Rust test suite execution. | `Equivalent` (`L2` / `L4`) |
| **3. Diverged -> Repaired Case** | 1:00 | Show how a faulty initial translation is caught by the differential harness (`Diverged` with counterexample) and subsequently repaired and re-verified. | `Diverged` $\rightarrow$ `Equivalent` |
| **4. Surfacing Latent C Bugs (`CLatentBug`)** | 1:00 | Run `ferrite bench` to show Ferrite catching off-by-one reads, out-of-bounds writes, and `INT_MIN` signed overflow in C while safe Rust succeeds. | `CLatentBug` |
| **5. Interactive HTML Dashboard & Live Model Switch** | 0:30 | Open `ferrite-out/dashboard.html` and show switching to `--provider openai` via `.env`. | Wrap-up |

---

## Step-by-Step Walkthrough & Commands

### Scene 1: Clean Migration & HTML Dashboard (0:00 – 1:45)

**Narration**:
*"Ferrite migrates C libraries to 100% safe, idiomatic Rust one function at a time. Today we are running in **Replay Mode** (`--provider replay`), where reference translations come from local fixtures so the entire verification engine runs deterministically offline without API keys."*

**Command**:
```powershell
cargo run -- --source-dir fixtures/clamp --provider replay --work-dir ferrite-out
```

**What to highlight on screen**:
1. **`[Analyze]`**: Extracts `clamp` and derives the idiomatic Rust signature contract `pub fn clamp(val: i32, min: i32, max: i32) -> i32`.
2. **`[Translate]`**: Loads `fixtures/replay/clamp.rs.md`, validates the AST with `syn` (confirming `0 unsafe lines`), and passes the `rustc` compile gate (`L0`).
3. **`[Verify]`**: Generates `clamp_shim.c` and `clamp_fuzz.rs`, links both C and Rust into an in-process binary, and executes **10,012 differential inputs** to reach `[ L2 VERIFIED ]`.

Next, show **L4 Hybrid Build Verification** on `bench/lib_checksum` (`adler32_step`, `crc32_byte`, `crc32_buf`):
```powershell
cargo test --test lib_checksum_test -- --nocapture
```
Highlight that Ferrite generates C delegation shims (`ferrite_adler32_step`, `ferrite_crc32_buf`) per ADR 0005, links the hybrid C+Rust library against `test_checksum.c` (RFC 1950 "Wikipedia" and IEEE 802.3 "123456789" vectors), and promotes all 3 functions to **`AssuranceLevel::L4`**.

---

### Scene 2: Diverged Translation Caught and Repaired (1:45 – 2:45)

**Narration**:
*"What happens when a proposed Rust translation compiles cleanly (`L0`) but has a subtle semantic bug? Let's watch the verifier catch a divergence and verify the repair."*

**Command**:
```powershell
cargo test --test verify_test verifier_verifies_two_arg_scalar_and_detects_divergence -- --nocapture
```

**What to highlight on screen**:
- First, a broken candidate (`a.wrapping_sub(b)` instead of `a.wrapping_add(b)`) passes `rustc` compilation (`L0`), but the differential harness immediately exits with `VerdictKind::Diverged` and records the exact counterexample input.
- Note that `ReplayProvider` also supports multi-attempt fixtures (`<fn_name>.attempt1.rs.md` followed by `<fn_name>.rs.md`) so the repair loop can be demonstrated deterministically offline.

---

### Scene 3: Surfacing Latent C Memory & UB Bugs (`CLatentBug`) (2:45 – 3:45)

**Narration**:
*"Differential fuzzing doesn't just find bugs in Rust translations—it frequently uncovers latent memory safety bugs in the original C code. When C and Rust disagree or C faults on an edge input, Ferrite's sanitizer triage distinguishes a bad Rust translation (`Diverged`) from a latent bug in C (`CLatentBug`)."*

**Command**:
```powershell
cargo run -- bench --manifest bench/manifest.toml --provider replay
```

**What to highlight in the Leaderboard Output**:
- **34 functions** across Tier 1 (scalars/strings), Tier 2 (buffers/parsers), Tier 3 (stateful ring-buffer/token-bucket helpers), `lib_checksum`, and `planted_bugs`.
- Focus on the 4 planted C bugs (`bug_off_by_one_read`, `bug_signed_overflow_abs`, `bug_oob_write_reset`, `bug_empty_slice_head`):
  - In `bug_oob_write_reset`, C loops `for (size_t i = 0; i <= len; i++) buf[i] = 0;`, overwriting the canary byte past the end of the buffer.
  - Safe Rust (`for b in buf.iter_mut() { *b = 0; }`) stays strictly within bounds.
  - Ferrite triages all 4 as **`CLatentBug`** and achieves **34/34 (100%) expected verdict match**.

---

### Scene 4: L3 Stateful Sequence Fuzzing & Switching to Live LLM Mode (3:45 – 4:15)

**Command (L3 Ring Buffer)**:
```powershell
cargo test --test stateful_test -- --nocapture
```
**What to highlight**:
- `StatefulVerifier` applies 2,000 randomized `Push`, `Pop`, `Len`, `IsEmpty`, and `Clear` operations simultaneously to an opaque C `CRingBuf` handle and `RustRingBuf`, checking state invariants after every single operation to reach **`AssuranceLevel::L3`**.

**Switching to a Live OpenAI-Compatible Endpoint**:
```powershell
# Copy .env.example and set your endpoint + key:
$env:FERRITE_PROVIDER = "openai"
$env:FERRITE_API_BASE = "https://api.studio.nebius.ai/v1"
$env:FERRITE_API_KEY  = "<your-api-key>"
$env:FERRITE_MODEL    = "nvidia/Llama-3.3-Nemotron-Super-49B-v1"

cargo run -- --source-dir fixtures/clamp --provider openai
```
Open `ferrite-out/dashboard.html` in the browser to show the standalone dark-mode report with per-function execution counts, coverage, and `0 unsafe lines`.
