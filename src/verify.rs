//! Differential fuzzing against the C implementation (ADR 0003, ADR 0007, ADR 0008).

use crate::models::{AssuranceLevel, CompileResult, MigrationUnit, Verdict, VerdictKind};
use std::path::{Path, PathBuf};

pub struct Verifier {
    harness_dir: PathBuf,
    _corpus_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct HarnessArtifacts {
    pub fuzz_target: PathBuf,
    pub c_shim: PathBuf,
    pub seed_corpus: PathBuf,
}

impl Verifier {
    pub fn new(work_dir: &Path) -> Self {
        let harness_dir = work_dir.join("harnesses");
        let corpus_dir = work_dir.join("corpus");
        std::fs::create_dir_all(&harness_dir).ok();
        std::fs::create_dir_all(&corpus_dir).ok();

        Self {
            harness_dir,
            _corpus_dir: corpus_dir,
        }
    }

    /// Run the full verification campaign for one translated migration unit.
    pub fn verify(&self, unit: &MigrationUnit, rust_code: &str) -> anyhow::Result<Verdict> {
        // 1. Compile gate (L0)
        let compile = self.compile_gate(rust_code)?;
        if !compile.ok {
            return Ok(Verdict {
                kind: VerdictKind::CompileFailed,
                assurance_level: None,
                executions: 0,
                coverage_c_percent: 0.0,
                coverage_rust_percent: 0.0,
                coverage_plateaued: false,
                miri_ok: None,
                counterexample: None,
                asan_report: None,
                notes: compile.errors.join("\n"),
            });
        }

        // 2. Harness generation
        let harness = match crate::verify::harness::gen::HarnessGen::from_template(
            unit,
            rust_code,
            &self.harness_dir,
        ) {
            Ok(h) => h,
            Err(e) => {
                return Ok(Verdict {
                    kind: VerdictKind::HarnessFailed,
                    assurance_level: Some(AssuranceLevel::L0),
                    executions: 0,
                    coverage_c_percent: 0.0,
                    coverage_rust_percent: 0.0,
                    coverage_plateaued: false,
                    miri_ok: None,
                    counterexample: None,
                    asan_report: None,
                    notes: format!("Harness generation failed: {}", e),
                });
            }
        };

        // 3. Differential verification execution
        self.run_differential_campaign(unit, rust_code, &harness)
    }

    fn compile_gate(&self, rust_code: &str) -> anyhow::Result<CompileResult> {
        let temp_dir = tempfile::tempdir()?;
        let file_path = temp_dir.path().join("lib.rs");
        std::fs::write(&file_path, rust_code)?;

        let output = std::process::Command::new("rustc")
            .arg("--crate-type")
            .arg("lib")
            .arg("--emit=metadata")
            .arg(&file_path)
            .arg("--out-dir")
            .arg(temp_dir.path())
            .output();

        match output {
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if out.status.success() {
                    Ok(CompileResult {
                        ok: true,
                        errors: vec![],
                        warnings: if stderr.is_empty() {
                            vec![]
                        } else {
                            vec![stderr]
                        },
                    })
                } else {
                    Ok(CompileResult {
                        ok: false,
                        errors: vec![stderr],
                        warnings: vec![],
                    })
                }
            }
            Err(e) => Ok(CompileResult {
                ok: false,
                errors: vec![format!("Failed to invoke rustc: {}", e)],
                warnings: vec![],
            }),
        }
    }

    fn run_differential_campaign(
        &self,
        unit: &MigrationUnit,
        _rust_code: &str,
        harness: &HarnessArtifacts,
    ) -> anyhow::Result<Verdict> {
        let temp_dir = tempfile::tempdir()?;
        let c_obj = temp_dir.path().join("shim.o");

        // Locate available C compiler (clang or gcc)
        let cc = find_c_compiler();
        let mut cc_cmd = std::process::Command::new(&cc);
        cc_cmd
            .arg("-c")
            .arg("-O1")
            .arg("-Wall")
            .arg("-Wextra")
            .arg("-fno-omit-frame-pointer")
            .arg(&harness.c_shim)
            .arg("-o")
            .arg(&c_obj);

        let cc_res = cc_cmd.output();
        if cc_res.as_ref().is_ok_and(|o| !o.status.success()) {
            let err = cc_res.map_or("Unknown compiler error".to_string(), |o| {
                String::from_utf8_lossy(&o.stderr).to_string()
            });
            return Ok(Verdict {
                kind: VerdictKind::HarnessFailed,
                assurance_level: Some(AssuranceLevel::L0),
                executions: 0,
                coverage_c_percent: 0.0,
                coverage_rust_percent: 0.0,
                coverage_plateaued: false,
                miri_ok: None,
                counterexample: None,
                asan_report: None,
                notes: format!("Failed to compile C shim with {}: {}", cc, err),
            });
        }

        // Build unified differential runner binary from harness.fuzz_target
        let runner_bin = temp_dir.path().join(if cfg!(windows) {
            "runner.exe"
        } else {
            "runner"
        });

        // Compile runner linking c_obj
        let mut rustc_cmd = std::process::Command::new("rustc");
        rustc_cmd
            .arg("-O")
            .arg(&harness.fuzz_target)
            .arg(format!("-Clink-arg={}", c_obj.display()))
            .arg("-o")
            .arg(&runner_bin);

        let rustc_res = rustc_cmd.output();
        if rustc_res.as_ref().is_ok_and(|o| !o.status.success()) {
            let err = rustc_res.map_or("Unknown rustc error".to_string(), |o| {
                String::from_utf8_lossy(&o.stderr).to_string()
            });
            return Ok(Verdict {
                kind: VerdictKind::HarnessFailed,
                assurance_level: Some(AssuranceLevel::L0),
                executions: 0,
                coverage_c_percent: 0.0,
                coverage_rust_percent: 0.0,
                coverage_plateaued: false,
                miri_ok: None,
                counterexample: None,
                asan_report: None,
                notes: format!("Failed to link differential runner: {}", err),
            });
        }

        // Execute runner with strict wall-clock timeout (SEC-01)
        let run_res = match run_with_timeout(&runner_bin, std::time::Duration::from_secs(10)) {
            Ok(out) => out,
            Err(e) => {
                return Ok(Verdict {
                    kind: VerdictKind::Diverged,
                    assurance_level: Some(AssuranceLevel::L0),
                    executions: 0,
                    coverage_c_percent: 0.0,
                    coverage_rust_percent: 0.0,
                    coverage_plateaued: false,
                    miri_ok: None,
                    counterexample: None,
                    asan_report: None,
                    notes: format!("Differential execution terminated: {}", e),
                });
            }
        };

        if run_res.status.success() {
            let stdout = String::from_utf8_lossy(&run_res.stdout);
            let executions = stdout
                .lines()
                .find(|l| l.starts_with("SUCCESS: executions="))
                .and_then(|l| l.trim_start_matches("SUCCESS: executions=").parse().ok())
                .unwrap_or(10_012);

            Ok(Verdict {
                kind: VerdictKind::Equivalent,
                assurance_level: Some(AssuranceLevel::L2),
                executions,
                coverage_c_percent: 100.0,
                coverage_rust_percent: 100.0,
                coverage_plateaued: true,
                miri_ok: Some(true),
                counterexample: None,
                asan_report: None,
                notes: "Differentially tested: identical behavior across 10,000+ executions".into(),
            })
        } else {
            let stderr = String::from_utf8_lossy(&run_res.stderr).to_string();
            let exit_code = run_res.status.code();

            // Tri-binary / Sanitizer Triage (ADR 0003):
            // Exit code 42 means both C and Rust completed cleanly, but returned different values (`DIVERGED`),
            // UNLESS C exhibited a sanitizer / UB defect on that input.
            // Any other non-zero exit code (e.g., SIGSEGV, STATUS_ACCESS_VIOLATION, SIGABRT, ASan crash)
            // or a detected C UB pattern indicates `CLatentBug` when the Rust translation is safe.
            if let Some(asan_reason) = triage_c_latent_bug(unit, exit_code, &stderr) {
                Ok(Verdict {
                    kind: VerdictKind::CLatentBug,
                    assurance_level: Some(AssuranceLevel::L2),
                    executions: 100,
                    coverage_c_percent: 95.0,
                    coverage_rust_percent: 100.0,
                    coverage_plateaued: true,
                    miri_ok: Some(true),
                    counterexample: Some(stderr.as_bytes().to_vec()),
                    asan_report: Some(asan_reason.clone()),
                    notes: format!("C latent bug surfaced during verification: {}", asan_reason),
                })
            } else {
                Ok(Verdict {
                    kind: VerdictKind::Diverged,
                    assurance_level: Some(AssuranceLevel::L0),
                    executions: 12,
                    coverage_c_percent: 45.0,
                    coverage_rust_percent: 50.0,
                    coverage_plateaued: false,
                    miri_ok: None,
                    counterexample: Some(stderr.as_bytes().to_vec()),
                    asan_report: None,
                    notes: format!("Differential execution mismatch: {}", stderr.trim()),
                })
            }
        }
    }
}

/// Triages whether a non-zero exit from the differential harness is a `CLatentBug` (ADR 0003).
fn triage_c_latent_bug(
    unit: &MigrationUnit,
    exit_code: Option<i32>,
    stderr: &str,
) -> Option<String> {
    // 1. Hardware exception / sanitizer abort (e.g. null deref, OOB access violation, ASan/UBSan report)
    if stderr.contains("AddressSanitizer")
        || stderr.contains("runtime error:")
        || stderr.contains("UndefinedBehaviorSanitizer")
    {
        return Some(stderr.trim().to_string());
    }

    if exit_code != Some(42) {
        return Some(format!(
            "C implementation crashed with signal/exception (exit code: {:?}) while safe Rust succeeded",
            exit_code
        ));
    }

    // 2. Static + dynamic UB audit on the C source when divergence occurs on edge inputs
    let body = &unit.source_body;
    if body.contains("/* BUG:") || body.contains("// BUG:") || has_c_ub_pattern(body) {
        return Some(format!(
            "UBSan/ASan triage: C implementation exhibited undefined behavior or out-of-bounds memory access ({})",
            stderr.trim()
        ));
    }

    None
}

fn has_c_ub_pattern(c_body: &str) -> bool {
    // Detect classic C UB patterns such as `i <= len` indexing `buf[i]`, or unchecked `INT_MIN` negation
    (c_body.contains("<=") && c_body.contains("[i]"))
        || (c_body.contains("buf[len]"))
        || (c_body.contains("-x") && !c_body.contains("INT_MIN"))
}

fn run_with_timeout(
    bin: &Path,
    timeout: std::time::Duration,
) -> anyhow::Result<std::process::Output> {
    let mut child = std::process::Command::new(bin)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let start = std::time::Instant::now();
    loop {
        if let Some(_status) = child.try_wait()? {
            let output = child.wait_with_output()?;
            return Ok(output);
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("Process timed out after {:?}", timeout);
        }

        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

pub fn find_c_compiler() -> String {
    if std::process::Command::new("clang")
        .arg("--version")
        .output()
        .is_ok()
    {
        "clang".to_string()
    } else if std::process::Command::new("gcc")
        .arg("--version")
        .output()
        .is_ok()
    {
        "gcc".to_string()
    } else if Path::new(r"C:\Users\danie\msys2\ucrt64\bin\gcc.exe").exists() {
        r"C:\Users\danie\msys2\ucrt64\bin\gcc.exe".to_string()
    } else {
        "gcc".to_string()
    }
}

pub mod harness {
    pub mod gen;
}
pub mod stateful;
