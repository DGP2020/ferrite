use serde::{Deserialize, Serialize};

/// The atomic unit of planning, translation, and verification.
/// Contains one or more related C functions and their associated types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationUnit {
    pub id: String,
    pub name: String,
    pub signature: String, // C declaration text
    #[serde(default)]
    pub rust_contract_sig: String, // Idiomatic Rust signature contract
    pub source_ref: String, // file:line locator
    pub source_body: String, // raw C body text
    pub callees: Vec<String>, // ids of functions this calls
    pub features: FeatureVector,
    pub status: UnitStatus,
    #[serde(default)]
    pub assurance_level: Option<AssuranceLevel>,
}

pub type FunctionUnit = MigrationUnit;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssuranceLevel {
    /// Compiles, clippy clean.
    L0,
    /// Passes unit tests derived from examples or LLM test suite.
    L1,
    /// Clean differential fuzzing against original C.
    L2,
    /// Stateful API-sequence fuzzing clean (for opaque handles).
    L3,
    /// Original C test suite passes against hybrid C+Rust build.
    L4,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum UnitStatus {
    Analyzed,
    Planned,
    Translating,
    TranslatingFailed,
    Translated,
    Verifying,
    Verified,
    Diverged,
    CLatentBug,
    HarnessFailed,
    CompileFailed,
    Integrated,
}

/// A compact feature vector the policy and harness generator consume.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureVector {
    pub pointer_op_count: u32,
    pub malloc_free_count: u32,
    pub buffer_length_pairs: u32,
    pub union_count: u32,
    pub function_pointer_count: u32,
    pub global_count: u32,
    pub callee_count: u32,
    pub ast_node_count: u32,
    pub line_count: u32,
    pub has_loops: bool,
    pub has_pointers: bool,
    pub has_strings: bool,
    pub has_out_params: bool,
    pub harnessable: bool,
}

/// One attempt at producing + verifying a Rust translation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    pub id: String,
    pub function_id: String,
    pub tier: Tier,
    pub stage: Stage,
    pub prompt_hash: String,
    pub rust_code: String,
    pub compile_result: CompileResult,
    pub fuzz_result: Option<FuzzResult>,
    pub verdict: Option<Verdict>,
    pub tokens: Option<Tokens>,
    pub cost_usd: Option<f64>,
    pub latency_ms: Option<u64>,
    pub attempt_number: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Tier {
    Nano,
    Super,
    Ultra,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Stage {
    Plan,
    Translate,
    Seed,
    Repair,
    Explain,
}

/// Token usage statistics for an attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tokens {
    pub prompt: u64,
    pub completion: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileResult {
    pub ok: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzResult {
    pub executions: u64,
    pub coverage_c_percent: f64,
    pub coverage_rust_percent: f64,
    pub coverage_plateaued: bool,
    pub miri_ok: Option<bool>,
    pub min_counterexample: Option<Vec<u8>>,
    pub runtime_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    pub kind: VerdictKind,
    pub assurance_level: Option<AssuranceLevel>,
    pub executions: u64,
    pub coverage_c_percent: f64,
    pub coverage_rust_percent: f64,
    pub coverage_plateaued: bool,
    pub miri_ok: Option<bool>,
    pub counterexample: Option<Vec<u8>>,
    pub asan_report: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerdictKind {
    /// C and Rust behave identically across the fuzz campaign.
    Equivalent,
    /// C and Rust diverged and C was clean. Repair needed.
    Diverged,
    /// C crashed or flagged UB under ASan/UBSan. Rust is arguably more correct.
    CLatentBug,
    /// The generated harness itself failed to compile or run.
    HarnessFailed,
    /// The Rust translation failed to compile.
    CompileFailed,
}

/// A minimal benchmark datum captured per verified function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionReport {
    pub unit: MigrationUnit,
    pub verdict: Verdict,
    pub attempt_count: u32,
    pub total_tokens: Option<Tokens>,
    pub total_cost_usd: f64,
    pub total_latency_ms: u64,
    pub unsafe_line_count: u32,
}

/// Manifest schema for per-library `ferrite.toml` (ADR 0009).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    pub library: LibraryMeta,
    #[serde(default)]
    pub clusters: std::collections::HashMap<String, Vec<String>>,
    #[serde(default)]
    pub defines: Vec<String>,
    #[serde(default)]
    pub skip: Vec<String>,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryMeta {
    pub name: String,
    #[serde(default)]
    pub compile_commands: Option<String>,
    #[serde(default)]
    pub include_dirs: Vec<String>,
    #[serde(default)]
    pub source_files: Vec<String>,
    #[serde(default)]
    pub build_cmd: Option<String>,
    #[serde(default)]
    pub test_cmd: Option<String>,
}
