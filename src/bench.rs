//! Benchmark manifest runner and leaderboard generator for Ferrite-Bench Lite (ADR 0011).

use crate::analyze::Analyzer;
use crate::models::{AssuranceLevel, VerdictKind};
use crate::policy::StaticPolicy;
use crate::provider::LlmProvider;
use crate::translate::{validate_rust_ast, Translator};
use crate::verify::Verifier;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchManifest {
    #[serde(default = "default_version")]
    pub version: u32,
    pub entries: Vec<BenchEntry>,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchEntry {
    pub name: String,
    pub library: String,
    pub source: String,
    pub tier: u8,
    pub expected_verdict: VerdictKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchItemResult {
    pub name: String,
    pub library: String,
    pub tier: u8,
    pub expected_verdict: VerdictKind,
    pub actual_verdict: VerdictKind,
    pub assurance_level: Option<AssuranceLevel>,
    pub matched: bool,
    pub executions: u64,
    pub unsafe_lines: u32,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchSummary {
    pub provider_mode: String,
    pub total_functions: usize,
    pub compiled_count: usize,
    pub l2_verified_count: usize,
    pub c_latent_bugs_found: usize,
    pub matched_expectations: usize,
    pub results: Vec<BenchItemResult>,
}

impl BenchManifest {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let manifest: BenchManifest = toml::from_str(&content)?;
        Ok(manifest)
    }
}

impl BenchSummary {
    pub fn format_leaderboard(&self) -> String {
        let mut out = String::new();
        out.push_str(
            "================================================================================\n",
        );
        out.push_str(&format!(
            "        FERRITE-BENCH LITE LEADERBOARD [{} - REPLAY MODE]\n",
            self.provider_mode.to_uppercase()
        ));
        out.push_str(
            "  Note: Replay translations are human-written reference Rust (not LLM output).\n",
        );
        out.push_str(
            "================================================================================\n",
        );
        let compile_pct = if self.total_functions == 0 {
            0.0
        } else {
            (self.compiled_count as f64 / self.total_functions as f64) * 100.0
        };
        out.push_str(&format!(
            " Total Functions: {:<6} | Compile Rate (L0): {}/{} ({:.1}%)\n",
            self.total_functions, self.compiled_count, self.total_functions, compile_pct
        ));
        out.push_str(&format!(
            " L2 Differential: {:<6} | C Latent Bugs Found: {:<4} | Verdict Match: {}/{}\n",
            self.l2_verified_count,
            self.c_latent_bugs_found,
            self.matched_expectations,
            self.total_functions
        ));
        out.push_str(
            "--------------------------------------------------------------------------------\n",
        );
        out.push_str(&format!(
            " {:<26} | {:<4} | {:<14} | {:<14} | {:<6}\n",
            "Function", "Tier", "Expected", "Actual", "Match"
        ));
        out.push_str(
            "--------------------------------------------------------------------------------\n",
        );

        for item in &self.results {
            let status_str = if item.matched { "PASS" } else { "FAIL" };
            out.push_str(&format!(
                " {:<26} | T{:<3} | {:<14} | {:<14} | {:<6}\n",
                item.name,
                item.tier,
                format!("{:?}", item.expected_verdict),
                format!("{:?}", item.actual_verdict),
                status_str
            ));
        }
        out.push_str(
            "================================================================================\n",
        );
        out
    }
}

pub struct BenchRunner {
    provider: Arc<dyn LlmProvider>,
    work_dir: PathBuf,
}

impl BenchRunner {
    pub fn new(provider: Arc<dyn LlmProvider>, work_dir: impl AsRef<Path>) -> Self {
        Self {
            provider,
            work_dir: work_dir.as_ref().to_path_buf(),
        }
    }

    pub fn run_manifest(
        &self,
        manifest: &BenchManifest,
        repo_root: &Path,
    ) -> anyhow::Result<BenchSummary> {
        std::fs::create_dir_all(&self.work_dir)?;
        let analyzer = Analyzer;
        let translator = Translator::new(StaticPolicy::default(), self.provider.clone());
        let verifier = Verifier::new(&self.work_dir);

        let mut results = Vec::with_capacity(manifest.entries.len());
        let mut compiled_count = 0usize;
        let mut l2_verified_count = 0usize;
        let mut c_latent_bugs_found = 0usize;
        let mut matched_expectations = 0usize;

        for entry in &manifest.entries {
            let src_path = repo_root.join(&entry.source);
            let units = analyzer.analyze_file(&src_path)?;
            let unit = units
                .into_iter()
                .find(|u| u.name == entry.name)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Function '{}' not found when analyzing '{}'",
                        entry.name,
                        src_path.display()
                    )
                })?;

            let attempt = translator.translate_one(&unit, &[])?;
            if !attempt.compile_result.ok {
                let actual = VerdictKind::CompileFailed;
                let matched = actual == entry.expected_verdict;
                if matched {
                    matched_expectations += 1;
                }
                results.push(BenchItemResult {
                    name: entry.name.clone(),
                    library: entry.library.clone(),
                    tier: entry.tier,
                    expected_verdict: entry.expected_verdict,
                    actual_verdict: actual,
                    assurance_level: None,
                    matched,
                    executions: 0,
                    unsafe_lines: 0,
                    notes: attempt.compile_result.errors.join("; "),
                });
                continue;
            }

            compiled_count += 1;
            let (_, unsafe_lines) = validate_rust_ast(&attempt.rust_code, &unit.name)?;
            let verdict = verifier.verify(&unit, &attempt.rust_code)?;

            if verdict.assurance_level == Some(AssuranceLevel::L2) {
                l2_verified_count += 1;
            }
            if verdict.kind == VerdictKind::CLatentBug {
                c_latent_bugs_found += 1;
            }
            let matched = verdict.kind == entry.expected_verdict;
            if matched {
                matched_expectations += 1;
            }

            results.push(BenchItemResult {
                name: entry.name.clone(),
                library: entry.library.clone(),
                tier: entry.tier,
                expected_verdict: entry.expected_verdict,
                actual_verdict: verdict.kind,
                assurance_level: verdict.assurance_level,
                matched,
                executions: verdict.executions,
                unsafe_lines,
                notes: verdict.notes,
            });
        }

        let summary = BenchSummary {
            provider_mode: self.provider.name().to_string(),
            total_functions: manifest.entries.len(),
            compiled_count,
            l2_verified_count,
            c_latent_bugs_found,
            matched_expectations,
            results,
        };

        let json_path = self.work_dir.join("bench-results.json");
        let serialized = serde_json::to_string_pretty(&summary)?;
        std::fs::write(json_path, serialized)?;

        Ok(summary)
    }
}
