# Ferrite Verification Engine

The **Verification Engine** is the deterministic core of Ferrite. In automated translation, language models often generate syntactically convincing code that contains subtle behavioral regressions, silent precision truncations, or boundary errors.

Ferrite rejects LLM self-evaluation. Equivalence between C and Rust is proven empirically through differential execution under memory sanitizers, structured fuzzing, and regression tests.

---

## 1. The Assurance Ladder

Every migrated function is assigned the highest level it demonstrably attains on the Ferrite Assurance Ladder:

| Level | Name | Verification Method | Exit Criteria |
| :---: | :--- | :--- | :--- |
| **L0** | **Syntactic & Typestate** | `cargo check` and `cargo clippy -- -D warnings` | Compiles without errors; passes strict linter rules; 0 unapproved `unsafe` blocks. |
| **L1** | **Unit Tested** | Unit tests synthesized from existing C tests or doc examples | Passes all standalone unit tests with identical return values for known inputs. |
| **L2** | **Differentially Tested** | In-process differential fuzzing campaign against compiled C | Zero output discrepancies across thousands of random and boundary inputs; coverage plateau reached. |
| **L3** | **Stateful API Tested** | Sequential operation fuzzing on opaque handles | Equivalence verified across sequences of stateful calls (`init` $\rightarrow$ `op` $\rightarrow$ `free`). |
| **L4** | **Hybrid Integrated** | Execution of original C project test suite via source delegation shims | Entire C library test suite passes with translated Rust function dynamically linked. |
| **L5** | **Formally Bounded** | Symbolic execution (KLEE / Kani) or bounded equivalence | Formally verified bounded equivalence for pure computational blocks. |

---

## 2. Tri-Binary Differential Triage (ADR 0003)

Differential testing frequently surfaces mismatches, but not all mismatches mean the Rust code is wrong. Often, an unusual input triggers an undefined behavior or memory safety defect in the legacy C implementation that Rust's type system handles safely.

To definitively isolate the root cause of every mismatch, Ferrite executes a **Tri-Binary Triage strategy**:

```mermaid
flowchart TD
    INPUT["Generated Fuzz Input"] --> DIFF{"Unified Differential Binary<br/>(C Shim + Rust Translation)"}
    
    DIFF -- "Outputs Match" --> NEXT["Continue Fuzz Campaign"]
    DIFF -- "Execution Divergence or Crash" --> TRIAGE["Triage Sub-Binaries"]
    
    TRIAGE --> C_BIN["1. Standalone C Binary<br/>(clang + ASan/UBSan)"]
    TRIAGE --> R_BIN["2. Standalone Rust Binary<br/>(rustc + debug assertions)"]
    
    C_BIN -- "ASan / UBSan Crash or Error" --> LATENT["Verdict: C_LATENT_BUG<br/>(Memory defect in original C)"]
    C_BIN -- "C Executes Cleanly" --> R_CHECK{"Check Rust Behavior"}
    
    R_CHECK -- "Rust Returned Divergent Value or Panicked" --> DIVERGED["Verdict: DIVERGED<br/>(Translation defect to repair)"]
    R_CHECK -- "Rust Memory Error" --> RUST_BUG["Verdict: TRANSLATION_FAULT"]
```

### Triage Matrix

| Differential Output | C Binary (ASan/UBSan) | Rust Binary | Classification | Action |
| :--- | :--- | :--- | :--- | :--- |
| Identical return & buffers | Clean | Clean | **Equivalent (L2)** | Increment execution count; monitor edge coverage. |
| Return mismatch | Clean | Clean | **Diverged** | Minimize counterexample input; route to repair prompt. |
| Crash / Signal | Crash (ASan/UBSan report) | Clean return / safe Err | **C Latent Bug** | Log CWE finding, archive sanitizer backtrace, mark Rust verified. |
| Crash / Signal | Clean | Panic / Unwind | **Diverged** | Minimize crash input; escalate model tier for repair. |
| Link / Compilation error | N/A | N/A | **Harness Failed** | Flag signature or type-contract generation issue. |

---

## 3. In-Process Differential Harness Generation

Harnesses are generated deterministically by `src/verify/harness/gen.rs`.

### Architecture of the Unified Binary

```
+--------------------------------------------------------------+
| Unified Differential Runner Binary                           |
|                                                              |
|  +--------------------+             +---------------------+  |
|  | C Implementation   |             | Rust Implementation |  |
|  | (Compiled via CC)  |             | (Compiled via rustc)|  |
|  +---------+----------+             +----------+----------+  |
|            |                                   |             |
|            v                                   v             |
|  +--------------------+             +---------------------+  |
|  | C FFI Shim Header  |             | extern "C" Wrapper  |  |
|  +---------+----------+             +----------+----------+  |
|            |                                   |             |
|            +-----------------+-----------------+             |
|                              |                               |
|                              v                               |
|             +---------------------------------+              |
|             | Harness Comparator (main)       |              |
|             |  - Read seed input (argv[1])    |              |
|             |  - Execute C shim               |              |
|             |  - Execute Rust wrapper         |              |
|             |  - Compare results & buffers    |              |
|             +---------------------------------+              |
+--------------------------------------------------------------+
```

### Seed Corpus & Edge-Case Generation
Before open-ended fuzzing begins, Ferrite synthesizes deterministic boundary seeds for each parameter:
- **Integers**: `0`, `1`, `-1`, `i32::MIN`, `i32::MAX`, signed/unsigned overflow boundaries.
- **Buffers / Slices**: Empty slices (`len = 0`), single-element buffers, off-by-one lengths, alignment-unaligned buffers.
- **Strings**: Empty strings `""`, strings without trailing nulls, embedded null bytes, strings exceeding expected buffer capacity.

---

## 4. Two-Phase Reproduction Check & Repair Loop (ADR 0008)

When a translation divergence is detected, blindly sending raw errors back to the model wastes token budget and CPU cycles. Ferrite implements a two-phase confirmation protocol:

```mermaid
sequenceDiagram
    autonumber
    participant Verifier as Verification Engine
    participant Repo as Reproduction Runner
    participant LLM as Provider Layer
    
    Note over Verifier: Mismatch detected on input X
    Verifier->>Repo: Phase 1: Synthesize standalone test(X)
    Repo->>Repo: Execute test(X) against failing translation
    
    alt Test does NOT reproduce failure
        Repo-->>Verifier: Reproduction failed (flaky or harness issue)
        Note over Verifier: Abort repair; inspect harness
    else Test confirms failure
        Repo-->>LLM: Repair prompt (contract, code, failing test, diff)
        LLM-->>Repo: Patched Rust translation
        
        Note over Repo: Phase 2: Post-repair check
        Repo->>Repo: Re-execute test(X) against patched code
        
        alt Patched code still fails test(X)
            Repo-->>LLM: Escalated repair attempt with error feedback
        else Patched code passes test(X)
            Repo-->>Verifier: Re-launch full differential fuzz campaign
        end
    end
```

---

## 5. Latent Bug Classification & Cybersecurity Reporting

When the C implementation crashes under AddressSanitizer or UndefinedBehaviorSanitizer while the Rust translation completes safely, Ferrite captures the event as a **C Latent Bug**.

### Taxonomy & CWE Mapping

| Sanitizer Output Pattern | CWE Classification | Description |
| :--- | :--- | :--- |
| `AddressSanitizer: global-buffer-overflow` / `stack-buffer-overflow` | **CWE-787** | Out-of-bounds Write |
| `AddressSanitizer: out-of-bounds-read` | **CWE-125** | Out-of-bounds Read |
| `AddressSanitizer: heap-use-after-free` | **CWE-416** | Use After Free |
| `UndefinedBehaviorSanitizer: signed-integer-overflow` | **CWE-190** | Integer Overflow / Wrap Around |
| `AddressSanitizer: SEGV on unknown address 0x00000000` | **CWE-476** | NULL Pointer Dereference |

### Finding Artifacts
For each verified latent defect, Ferrite generates:
1. **Trigger Input**: Raw and minimized binary payload that reproduces the sanitizer violation.
2. **Crash Backtrace**: Full symbolicated stack trace from AddressSanitizer.
3. **C vs. Rust Execution Diffs**: Side-by-side demonstration showing the C crash alongside Rust's safe execution (e.g. `None`, `Err(OutOfBounds)`, or clamped computation).

---

## 6. Stateful API Fuzzing for L3 Assurance (`src/verify/stateful.rs`)

Pure scalar and stateless buffer functions are fully verified under L2 differential fuzzing. However, systems libraries (ring buffers, memory pools, parser contexts, opaque connection handles) maintain internal state across function invocations (`init` $\rightarrow$ `op1` $\rightarrow$ `op2` $\rightarrow$ `free`).

Ferrite's `StatefulVerifier` elevates stateful migrations to **Assurance Level L3**:

```mermaid
flowchart LR
    GEN["Random Sequence Generator<br/>(PRNG Seed)"] --> OPS["Op Sequence: [Push, Pop, Len, Clear]"]
    OPS --> C_HANDLE["Opaque C Handle<br/>(c_ringbuf_push, pop, len)"]
    OPS --> R_HANDLE["Rust Safe Model<br/>(RingBuf::push, pop, len)"]
    C_HANDLE --> CMP{"State Invariant Comparator<br/>(assert identical state after every op)"}
    R_HANDLE --> CMP
    CMP -- "All Ops Match Across Runs" --> L3["Verdict::Equivalent (L3)"]
    CMP -- "State Desynchronization" --> DIV["Verdict::Diverged (Counterexample Op Trace)"]
```

### Operation Sequence Invariants
1. **Sequence Synthesis**: Generates hundreds of randomized operations (`RingBufOp::Push(val)`, `RingBufOp::Pop`, `RingBufOp::Len`, `RingBufOp::IsEmpty`, `RingBufOp::Clear`).
2. **Dual-Model Execution**: Concurrently drives the C implementation via raw pointers and the Rust implementation via safe abstractions.
3. **Continuous Step Invariance**: Validates return values, internal buffer counts, and boundary behaviors after every single operation.
4. **Fault Injection Guard**: Tested via mutation fault injection—if the Rust model drops an element or drifts by 1 index, the verifier immediately halts with `VerdictKind::Diverged`.

---

## 7. Hybrid C+Rust Integration & L4 Verification (`src/integrate.rs`)

The ultimate real-world test for any migrated module is whether the parent project's existing tests continue to pass when compiled against a hybrid binary.

Ferrite's `Integrator` orchestrates the **L4 Verification** workflow (ADR 0005, ADR 0009):

```
+-------------------------------------------------------------+
| Hybrid Project Build (e.g. bench/lib_checksum)              |
|                                                             |
|  +------------------------+      +-----------------------+  |
|  | Original C Header      |      | Safe Rust Library     |  |
|  | checksum.h             |      | (Compiled staticlib)  |  |
|  +-----------+------------+      +-----------+-----------+  |
|              |                               |              |
|              v                               v              |
|  +------------------------+      +-----------------------+  |
|  | Source-Level Shim      |      | Exported FFI Symbols  |  |
|  | ferrite_shims.c        +----->+ ferrite_adler32_step  |  |
|  | (Replaces C functions) |      | ferrite_crc32_buf     |  |
|  +-----------+------------+      +-----------------------+  |
|              |                                              |
|              v                                              |
|  +-------------------------------------------------------+  |
|  | Native C Project Test Harness (e.g. test_checksum.c)  |  |
|  | Executes test_cmd from ferrite.toml                   |  |
|  | Returns 0 -> PROMOTED TO ASSURANCE LEVEL L4           |  |
|  +-------------------------------------------------------+  |
+-------------------------------------------------------------+
```

### L4 Promotion Criteria
To reach `AssuranceLevel::L4`, a `MigrationUnit` must:
1. Pass L0 compilation and Clippy checks with 0 unsafe lines.
2. Pass L2 differential fuzzing across $\ge 10,000$ inputs with 100% coverage plateau.
3. Successfully link as a static archive (`libferrite_<name>.a`) via source shims into the target C library.
4. Pass the library's original test suite (e.g., standard RFC 1950 and IEEE 802.3 test vectors) with zero regressions.

---

## 8. Ferrite-Bench Lite & Planted Defect Suite

To evaluate verifier accuracy and demonstrate `CLatentBug` triage without requiring external LLM API tokens, Ferrite provides **Ferrite-Bench Lite** (`bench/manifest.toml`):

### Benchmark Composition (34 Functions)
- **Tier 1 (Scalars & Strings)**: Math utilities, popcounts, leap year logic, GCD, bounded string lengths, ASCII parsers.
- **Tier 2 (Buffers & Parsers)**: Buffer checksums, byte reversals, key-value line counters, hexadecimal parsers.
- **Tier 3 (Stateful Systems)**: Ring-buffer index calculators, ring-buffer capacity monitors, token bucket rate-limiters.
- **Real Library (`lib_checksum`)**: `adler32_step`, `crc32_byte`, and `crc32_buf` migrated to L4.

### Planted C Latent Bugs
The benchmark includes 4 deliberate legacy C memory safety defects to verify sanitizer triage fidelity:

| Bug Identifier | C Vulnerability Pattern | Sanitizer Diagnosis | Safe Rust Defense |
| :--- | :--- | :--- | :--- |
| `bug_off_by_one_read` | Accesses `buf[len]` when `len == 0` or off-by-one | **CWE-125**: Out-of-bounds Read | Returns `None` or safely checks `len` |
| `bug_signed_overflow_abs` | Executes `-x` on `INT_MIN` in C | **CWE-190**: Signed Integer Overflow UB | `x.wrapping_abs()` or `checked_abs()` |
| `bug_oob_write_reset` | Loops `for (size_t i = 0; i <= len; i++) buf[i] = 0;` | **CWE-787**: Out-of-bounds Write | `buf.iter_mut().for_each(...)` bounds safety |
| `bug_empty_slice_head` | Dereferences `buf[0]` without checking `len > 0` | **CWE-476**: NULL/OOB Dereference | `buf.first().copied().unwrap_or(0)` |
