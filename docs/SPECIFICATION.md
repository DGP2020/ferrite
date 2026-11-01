# Ferrite Configuration & Technical Specification

This document details the configuration formats, CLI interfaces, signature scopes, and runtime environments supported by Ferrite.

---

## 1. Project Manifest Schema (`ferrite.toml`)

Rather than attempting brittle reverse-engineering of arbitrary build systems (CMake, Autotools, Make, Meson), Ferrite standardizes project configuration via a per-library manifest file named `ferrite.toml` (ADR 0009).

### Example `ferrite.toml`

```toml
version = 1

[library]
name = "libclamp"
compile_commands = "build/compile_commands.json"
include_dirs = ["include", "src"]
source_files = ["src/clamp.c", "src/math_utils.c"]
build_cmd = "make lib"
test_cmd = "make test"

# Preprocessor defines required for accurate Clang AST dumping
defines = ["FEATURE_SAFE_MATH=1", "NDEBUG"]

# Functions to skip during migration
skip = ["inline_asm_helper", "legacy_varargs_logger"]

# Manual cluster overrides for functions sharing internal static state
[clusters]
ring_buffer = ["rb_init", "rb_push", "rb_pop", "rb_free"]
memory_pool = ["pool_create", "pool_alloc", "pool_destroy"]
```

### Schema Definition

| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `version` | Integer | Yes | Manifest version (currently `1`). |
| `library.name` | String | Yes | Name of the target C library or component. |
| `library.compile_commands` | String | No | Path to `compile_commands.json` for compilation flags and include paths. |
| `library.include_dirs` | Array of Strings | No | Explicit search directories for C header files (`-I<dir>`). |
| `library.source_files` | Array of Strings | No | List of C source files to analyze. |
| `library.build_cmd` | String | No | Command to recompile the hybrid C+Rust library for L4 testing. |
| `library.test_cmd` | String | No | Command to execute the target project's original test suite. |
| `defines` | Array of Strings | No | Macro definitions passed to Clang preprocessor (`-D<macro>`). |
| `skip` | Array of Strings | No | Function names excluded from migration (e.g., unsupported varargs). |
| `clusters` | Map of String $\rightarrow$ Array | No | Named clusters of functions migrated and verified as a single cohesive unit. |

---

## 1.5. Benchmark Manifest Schema (`manifest.toml`)

For offline evaluation and multi-tier benchmarking, Ferrite defines a manifest format in `bench/manifest.toml` (ADR 0011).

### Example `bench/manifest.toml`

```toml
version = 1

[[entries]]
name = "clamp_i32"
library = "tier1_scalars"
source = "bench/tier1/scalars.c"
tier = 1
expected_verdict = "Equivalent"

[[entries]]
name = "sum_bytes"
library = "tier2_buffers"
source = "bench/tier2/buffers.c"
tier = 2
expected_verdict = "Equivalent"

[[entries]]
name = "bug_off_by_one_read"
library = "planted_bugs"
source = "bench/planted_bugs/bugs.c"
tier = 2
expected_verdict = "CLatentBug"
```

### Schema Definition

| Field | Type | Description |
| :--- | :--- | :--- |
| `version` | Integer | Schema version (currently `1`). |
| `entries[].name` | String | Target C function identifier. |
| `entries[].library` | String | Logical component, library, or difficulty grouping. |
| `entries[].source` | String | Relative file path containing the C function definition. |
| `entries[].tier` | Integer | Difficulty tier (`1` = scalars/strings, `2` = buffers/parsers, `3` = stateful/data structures). |
| `entries[].expected_verdict` | String | Expected outcome (`Equivalent`, `CLatentBug`, or `Diverged`). |

---

## 2. Command Line Interface (CLI)

Ferrite provides a unified pure-Rust command-line executable (ADR 0001).

```bash
ferrite [OPTIONS]
```

### Options

| Flag | Short | Default | Description |
| :--- | :---: | :--- | :--- |
| `--source-dir <PATH>` | `-s` | `fixtures/clamp` | Path to the directory or individual C source file to analyze and migrate. |
| `--config <PATH>` | `-c` | `None` | Path to a custom `ferrite.toml` configuration manifest. |
| `--work-dir <PATH>` | `-w` | `ferrite-out` | Output workspace for generated harnesses, seed corpora, and dashboards. |
| `--policy <POLICY>` | `-p` | `static` | Model routing policy (`static` or `cascade`). |
| `--provider <PROVIDER>` | | `replay` | Translation provider (`replay`, `mock`, or `openai`). |
| `--help` | `-h` | | Print help and argument information. |
| `--version` | `-V` | | Print Ferrite version. |

### Subcommands

#### `ferrite bench`
Executes the Ferrite-Bench Lite manifest against target functions across multiple tiers, validates actual outcomes against expected verdicts, and outputs `bench-results.json` along with an ANSI leaderboard table.

```bash
ferrite bench [OPTIONS]
```

| Flag | Default | Description |
| :--- | :--- | :--- |
| `--manifest <PATH>` | `bench/manifest.toml` | Path to the benchmark manifest TOML. |
| `--work-dir <PATH>` | `ferrite-out` | Output workspace for results and harnesses. |
| `--provider <PROVIDER>` | `replay` | Translation provider (`replay`, `mock`, or `openai`). |

### Environment Variables

Ferrite can be configured via environment variables, enabling automated CI runs and zero-credential local replay testing:

| Variable | Default | Description |
| :--- | :--- | :--- |
| `FERRITE_PROVIDER` | `replay` | Provider selection override (`replay`, `mock`, or `openai`). |
| `FERRITE_REPLAY_DIR` | `fixtures/replay` | Path to the directory containing `.rs.md` replay translation fixtures. |
| `FERRITE_API_BASE` | `http://127.0.0.1:8000/v1` | Base URL for OpenAI-compatible HTTP endpoints (Ollama, vLLM, remote APIs). |
| `FERRITE_API_KEY` | *(empty)* | Authorization Bearer token for remote HTTP model completions. |
| `FERRITE_MODEL` | `nvidia/Llama-3.3-Nemotron-Super-49B-v1` | Target model identifier passed in completion payloads. |

### Quick Start Commands

```bash
# Run migration on default fixtures using local offline replay mode
cargo run -- --source-dir fixtures/clamp --provider replay

# Run the 34-function benchmark suite offline
cargo run -- bench --manifest bench/manifest.toml --provider replay

# Run migration with a custom manifest and cascade model escalation
cargo run -- --config ./my_lib/ferrite.toml --work-dir ./out --policy cascade
```

---

## 3. Signature & Type Scope

### Phase 1 Scope (ADR 0007)
Phase 1 focuses on high-frequency, deterministically harnessable signatures:

| C Construct | Rust Idiomatic Contract | Deterministic FFI Wrapper |
| :--- | :--- | :--- |
| **Scalar Primitives** (`int`, `uint32_t`, `double`) | Native types (`i32`, `u32`, `f64`) | Pass-by-value |
| **Bounded Input Buffer** (`const uint8_t *buf, size_t len`) | Bounded immutable slice `&[u8]` | Pointer + length reconstruction |
| **Bounded Output Buffer** (`uint8_t *buf, size_t len`) | Bounded mutable slice `&mut [u8]` | Pointer + length reconstruction |
| **NUL-Terminated Strings** (`const char *str`) | String slice `&str` | `CStr::to_str` conversion |
| **Scalar Out-Pointers** (`int *out_val`) | Value return `T` or `Result<T, E>` | Dereference and write back to pointer |

### Phase 2 Scope (Planned)
- Structs and composite aggregates passed by value or reference.
- Opaque handles (`typedef struct Handle* HandleRef`) fuzzed using stateful sequences (L3).
- Enum mapping (`#[repr(C)] enum`).

### Flagged as Unsupported (Explicitly Not Translated)
- Variable arguments (`va_list`, `...`).
- Non-local jumps (`setjmp` / `longjmp`).
- Inline assembly (`__asm__`).
- Unstructured raw pointer casting without length guarantees.

---

## 4. Contract Specification: Dual-Layer & Fenced Code Blocks

### Fenced Markdown Code Contract (ADR 0010)
To avoid escaping bugs and schema parsing failures common in JSON-encoded code strings across diverse LLM providers, prompts demand a strict fenced code contract:

```markdown
```rust
pub fn clamp(val: i32, min: i32, max: i32) -> i32 {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}
```
```

Before compiling:
1. The code block is extracted from the model's markdown stream.
2. The code is parsed into a `syn::File` Abstract Syntax Tree.
3. The function name and signature are validated against the exact planned signature contract.
4. The AST is scanned for `syn::ExprUnsafe` items; unauthorized unsafe blocks cause immediate rejection.

### Deterministic FFI Generation (ADR 0004)
The companion FFI wrapper is generated by Ferrite—not the LLM:

```rust
#[no_mangle]
pub unsafe extern "C" fn ferrite_clamp(val: i32, min: i32, max: i32) -> i32 {
    clamp(val, min, max)
}
```

---

## 5. Execution Environment & Sandboxing

### Linux Execution Environment (ADR 0003, ADR 0006)
- **WSL2 / Docker**: Because libFuzzer and AddressSanitizer require native Linux memory management and POSIX signals, verification runs under WSL2 or Linux containers.
- **Resource Boundaries**:
  - Memory limit: 5 GB RAM / 4 GB swap cap on the verification subsystem.
  - libFuzzer execution cap: `-rss_limit_mb=1024`.
  - Wall-clock timeouts on all compiler invocations and fuzzer campaigns.
  - Concurrency: Multiple LLM API requests can run concurrently, but resource-intensive Clang/Rustc compilation and fuzzing jobs are strictly serialized to maintain low memory pressure.
