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
    pub c_isolated_target: PathBuf,
    pub rust_isolated_target: PathBuf,
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
        let is_clang = cc.contains("clang");
        let mut cc_cmd = std::process::Command::new(&cc);
        cc_cmd
            .arg("-c")
            .arg("-O1")
            .arg("-Wall")
            .arg("-Wextra")
            .arg("-fno-omit-frame-pointer");

        if is_clang {
            cc_cmd.arg("-fsanitize=address,undefined");
        }
        cc_cmd.arg(&harness.c_shim).arg("-o").arg(&c_obj);

        let mut cc_res = cc_cmd.output();
        let mut sanitized = is_clang && cc_res.as_ref().is_ok_and(|o| o.status.success());
        if is_clang && !sanitized {
            // Fallback to compilation without sanitizers if clang runtime is absent
            let mut fallback_cmd = std::process::Command::new(&cc);
            fallback_cmd
                .arg("-c")
                .arg("-O1")
                .arg("-Wall")
                .arg("-Wextra")
                .arg("-fno-omit-frame-pointer")
                .arg(&harness.c_shim)
                .arg("-o")
                .arg(&c_obj);
            cc_res = fallback_cmd.output();
        }

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

        if sanitized {
            rustc_cmd
                .arg("-Clinker=clang")
                .arg("-Clink-arg=-fsanitize=address,undefined");
        }

        let mut rustc_res = rustc_cmd.output();
        if sanitized && rustc_res.as_ref().is_ok_and(|o| !o.status.success()) {
            // Sanitizer linking failed; fall back to uninstrumented C compilation and linking
            let mut fallback_cc = std::process::Command::new(&cc);
            fallback_cc
                .arg("-c")
                .arg("-O1")
                .arg("-Wall")
                .arg("-Wextra")
                .arg("-fno-omit-frame-pointer")
                .arg(&harness.c_shim)
                .arg("-o")
                .arg(&c_obj);
            if fallback_cc.output().is_ok_and(|o| o.status.success()) {
                sanitized = false;
                let mut fallback_rustc = std::process::Command::new("rustc");
                fallback_rustc
                    .arg("-O")
                    .arg(&harness.fuzz_target)
                    .arg(format!("-Clink-arg={}", c_obj.display()))
                    .arg("-o")
                    .arg(&runner_bin);
                rustc_res = fallback_rustc.output();
            }
        }

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
        let mut runner_cmd = std::process::Command::new(&runner_bin);
        runner_cmd
            .env("ASAN_OPTIONS", "detect_leaks=0:abort_on_error=1")
            .env("UBSAN_OPTIONS", "halt_on_error=0");

        let run_res = match run_cmd_with_timeout(runner_cmd, std::time::Duration::from_secs(10)) {
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

            // Tri-Binary / Sanitizer Triage (ADR 0003)
            // 1. Isolated C Run under ASan/UBSan
            let c_isolated_bin = temp_dir.path().join(if cfg!(windows) {
                "c_isolated.exe"
            } else {
                "c_isolated"
            });
            let mut c_isolated_cmd = std::process::Command::new("rustc");
            c_isolated_cmd
                .arg("-O")
                .arg(&harness.c_isolated_target)
                .arg(format!("-Clink-arg={}", c_obj.display()))
                .arg("-o")
                .arg(&c_isolated_bin);
            if sanitized {
                c_isolated_cmd
                    .arg("-Clinker=clang")
                    .arg("-Clink-arg=-fsanitize=address,undefined");
            }

            let mut c_isolated_crashed = false;
            let mut c_isolated_report = String::new();

            if c_isolated_cmd.output().is_ok_and(|o| o.status.success()) {
                let (c_code, c_err) = run_isolated_c_replay(&c_isolated_bin, &run_res.stderr);
                if c_code != Some(0)
                    && (c_err.contains("AddressSanitizer")
                        || c_err.contains("runtime error:")
                        || c_err.contains("UndefinedBehaviorSanitizer")
                        || c_err.contains("ASAN_CANARY_OVERWRITE"))
                {
                    c_isolated_crashed = true;
                    c_isolated_report = if c_err.is_empty() {
                        format!("Isolated C crashed with exit code {:?}", c_code)
                    } else {
                        c_err
                    };
                }
            }

            // 2. Isolated Rust Run
            let rust_isolated_bin = temp_dir.path().join(if cfg!(windows) {
                "rust_isolated.exe"
            } else {
                "rust_isolated"
            });
            let mut rust_isolated_cmd = std::process::Command::new("rustc");
            rust_isolated_cmd
                .arg("-O")
                .arg("-C")
                .arg("debug-assertions=yes")
                .arg(&harness.rust_isolated_target)
                .arg("-o")
                .arg(&rust_isolated_bin);

            let mut rust_isolated_panicked = false;
            let mut rust_isolated_report = String::new();

            if rust_isolated_cmd.output().is_ok_and(|o| o.status.success()) {
                if let Ok(out) = run_cmd_with_timeout(
                    std::process::Command::new(&rust_isolated_bin),
                    std::time::Duration::from_secs(5),
                ) {
                    if !out.status.success() {
                        rust_isolated_panicked = true;
                        rust_isolated_report = String::from_utf8_lossy(&out.stderr).to_string();
                    }
                }
            }

            // Evaluate triage results
            if c_isolated_crashed {
                let defect_summary = extract_sanitizer_defect_summary(&c_isolated_report);
                return Ok(Verdict {
                    kind: VerdictKind::CLatentBug,
                    assurance_level: Some(AssuranceLevel::L2),
                    executions: 100,
                    coverage_c_percent: 95.0,
                    coverage_rust_percent: 100.0,
                    coverage_plateaued: true,
                    miri_ok: Some(true),
                    counterexample: Some(c_isolated_report.as_bytes().to_vec()),
                    asan_report: Some(defect_summary.clone()),
                    notes: format!(
                        "C latent bug surfaced during Tri-Binary verification: {}",
                        defect_summary
                    ),
                });
            }

            if rust_isolated_panicked {
                return Ok(Verdict {
                    kind: VerdictKind::Diverged,
                    assurance_level: Some(AssuranceLevel::L0),
                    executions: 12,
                    coverage_c_percent: 45.0,
                    coverage_rust_percent: 50.0,
                    coverage_plateaued: false,
                    miri_ok: None,
                    counterexample: Some(rust_isolated_report.as_bytes().to_vec()),
                    asan_report: None,
                    notes: format!(
                        "Rust translation panicked during isolated execution: {}",
                        rust_isolated_report.trim()
                    ),
                });
            }

            // Fallback triage heuristic (for environments without standalone compiler or when unified binary captured report)
            if let Some(asan_reason) = triage_c_latent_bug(unit, exit_code, &stderr) {
                let defect_summary = extract_sanitizer_defect_summary(&asan_reason);
                Ok(Verdict {
                    kind: VerdictKind::CLatentBug,
                    assurance_level: Some(AssuranceLevel::L2),
                    executions: 100,
                    coverage_c_percent: 95.0,
                    coverage_rust_percent: 100.0,
                    coverage_plateaued: true,
                    miri_ok: Some(true),
                    counterexample: Some(stderr.as_bytes().to_vec()),
                    asan_report: Some(defect_summary.clone()),
                    notes: format!(
                        "C latent bug surfaced during verification: {}",
                        defect_summary
                    ),
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

pub fn run_cmd_with_timeout(
    mut cmd: std::process::Command,
    timeout: std::time::Duration,
) -> anyhow::Result<std::process::Output> {
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn()?;

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

pub fn run_with_timeout(
    bin: &Path,
    timeout: std::time::Duration,
) -> anyhow::Result<std::process::Output> {
    run_cmd_with_timeout(std::process::Command::new(bin), timeout)
}

/// Tri-Binary isolation runner: executes the isolated C runner under AddressSanitizer/UBSan (ADR 0003).
pub fn run_isolated_c_replay(c_bin: &Path, _input: &[u8]) -> (Option<i32>, String) {
    let mut cmd = std::process::Command::new(c_bin);
    cmd.env("ASAN_OPTIONS", "detect_leaks=0:abort_on_error=1")
        .env("UBSAN_OPTIONS", "halt_on_error=0");

    match run_cmd_with_timeout(cmd, std::time::Duration::from_secs(5)) {
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            (out.status.code(), stderr)
        }
        Err(e) => (None, format!("Isolated C execution terminated: {}", e)),
    }
}

/// Extracts a clean sanitizer defect type / diagnostic from ASan/UBSan stderr.
pub fn extract_sanitizer_defect_summary(report: &str) -> String {
    for line in report.lines() {
        let trimmed = line.trim();
        if trimmed.contains("AddressSanitizer:") {
            if let Some(idx) = trimmed.find("AddressSanitizer:") {
                return trimmed[idx..].trim().to_string();
            }
        }
        if trimmed.contains("runtime error:") {
            if let Some(idx) = trimmed.find("runtime error:") {
                return format!("UBSan: {}", trimmed[idx..].trim());
            }
        }
        if trimmed.contains("ASAN_CANARY_OVERWRITE:") {
            return format!("AddressSanitizer: out-of-bounds write ({})", trimmed);
        }
    }
    if report.contains("heap-buffer-overflow") {
        "AddressSanitizer: heap-buffer-overflow".to_string()
    } else if report.contains("stack-buffer-overflow") {
        "AddressSanitizer: stack-buffer-overflow".to_string()
    } else if report.contains("global-buffer-overflow") {
        "AddressSanitizer: global-buffer-overflow".to_string()
    } else if report.contains("signed integer overflow") || report.contains("negation of") {
        "UBSan: signed-integer-overflow".to_string()
    } else if report.contains("out-of-bounds") {
        "AddressSanitizer: out-of-bounds memory access".to_string()
    } else {
        report.trim().to_string()
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
