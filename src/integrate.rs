//! Source-level shim injection and hybrid C+Rust L4 test suite integration (ADR 0004, ADR 0005, ADR 0009).

use crate::analyze::Analyzer;
use crate::models::{AssuranceLevel, FunctionReport, LibraryConfig, UnitStatus, VerdictKind};
use crate::plan::Planner;
use crate::policy::StaticPolicy;
use crate::provider::LlmProvider;
use crate::translate::{validate_rust_ast, Translator};
use crate::verify::harness::gen::{ParamSpec, SignatureSpec};
use crate::verify::{find_c_compiler, Verifier};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Integrator {
    provider: Arc<dyn LlmProvider>,
    work_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct IntegrationReport {
    pub library_name: String,
    pub units_migrated: usize,
    pub l4_passed: bool,
    pub test_output: String,
    pub generated_c_shim_path: PathBuf,
    pub generated_rust_ffi_path: PathBuf,
    pub function_reports: Vec<FunctionReport>,
}

impl Integrator {
    pub fn new(provider: Arc<dyn LlmProvider>, work_dir: impl AsRef<Path>) -> Self {
        Self {
            provider,
            work_dir: work_dir.as_ref().to_path_buf(),
        }
    }

    /// Migrates all functions in `lib_dir`, verifies each at L2 via differential fuzzing,
    /// generates source-level C delegation shims (`ferrite_<name>`, ADR 0005) and Rust FFI
    /// exports (ADR 0004), links the hybrid C+Rust binary against the library's original C
    /// test suite, and promotes verified units to `AssuranceLevel::L4`.
    pub fn migrate_and_verify_l4(&self, lib_dir: &Path) -> anyhow::Result<IntegrationReport> {
        std::fs::create_dir_all(&self.work_dir)?;

        let config_path = lib_dir.join("ferrite.toml");
        let config_str = std::fs::read_to_string(&config_path)?;
        let config: LibraryConfig = toml::from_str(&config_str)?;

        let analyzer = Analyzer;
        let mut raw_units = Vec::new();
        for src in &config.library.source_files {
            raw_units.extend(analyzer.analyze_file(&lib_dir.join(src))?);
        }
        let units = Planner::order(raw_units);

        let translator = Translator::new(StaticPolicy::default(), self.provider.clone());
        let verifier = Verifier::new(&self.work_dir);

        let mut function_reports = Vec::new();
        let mut c_shim_bodies = String::from(
            "/* Auto-generated Ferrite Hybrid Source Shims (ADR 0005) */\n#include <stdint.h>\n#include <stddef.h>\n#include <stdbool.h>\n\n",
        );
        let mut rust_ffi_module = String::from(
            "/* Auto-generated Ferrite Rust Library + FFI Exports (ADR 0004) */\n#![allow(clippy::all, unused_variables)]\n\n",
        );

        for mut unit in units {
            let attempt = translator.translate_one(&unit, &[])?;
            if !attempt.compile_result.ok {
                anyhow::bail!(
                    "Failed to compile translation for '{}' in '{}'",
                    unit.name,
                    config.library.name
                );
            }

            let (_, unsafe_count) = validate_rust_ast(&attempt.rust_code, &unit.name)?;
            let verdict = verifier.verify(&unit, &attempt.rust_code)?;
            if verdict.kind != VerdictKind::Equivalent {
                anyhow::bail!(
                    "Unit '{}' failed L2 differential verification: {:?}",
                    unit.name,
                    verdict.kind
                );
            }

            let spec = SignatureSpec::parse(&unit)?;
            c_shim_bodies.push_str(&generate_c_delegation_shim(&spec));
            c_shim_bodies.push('\n');

            rust_ffi_module.push_str(&attempt.rust_code);
            rust_ffi_module.push('\n');
            rust_ffi_module.push_str(&generate_rust_ffi_companion(&spec));
            rust_ffi_module.push('\n');

            unit.status = UnitStatus::Verified;
            unit.assurance_level = Some(AssuranceLevel::L2);

            function_reports.push(FunctionReport {
                unit,
                verdict,
                attempt_count: 1,
                total_tokens: attempt.tokens,
                total_cost_usd: attempt.cost_usd.unwrap_or(0.0),
                total_latency_ms: attempt.latency_ms.unwrap_or(0),
                unsafe_line_count: unsafe_count,
            });
        }

        let hybrid_dir = self.work_dir.join("hybrid_l4");
        std::fs::create_dir_all(&hybrid_dir)?;

        let c_shim_path = hybrid_dir.join("checksum_hybrid_shim.c");
        let rust_ffi_path = hybrid_dir.join("ferrite_hybrid_runner.rs");
        std::fs::write(&c_shim_path, &c_shim_bodies)?;

        // Build hybrid test runner: compile original C test file (`test_checksum.c` with `main` renamed to `c_test_main`)
        // + C source shim (`checksum_hybrid_shim.c`) linked into Rust binary providing `ferrite_<name>` symbols.
        let orig_test_c = std::fs::read_to_string(lib_dir.join("test_checksum.c"))?;
        let patched_test_c = orig_test_c.replace("int main(", "int c_test_main(");
        let test_c_path = hybrid_dir.join("test_suite_wrapped.c");
        std::fs::write(&test_c_path, patched_test_c)?;

        let cc = find_c_compiler();
        let shim_obj = hybrid_dir.join("shim.o");
        let test_obj = hybrid_dir.join("test_suite.o");

        let cc_shim = std::process::Command::new(&cc)
            .arg("-c")
            .arg("-O2")
            .arg("-I")
            .arg(lib_dir)
            .arg(&c_shim_path)
            .arg("-o")
            .arg(&shim_obj)
            .output()?;
        if !cc_shim.status.success() {
            anyhow::bail!(
                "Failed to compile C source shim: {}",
                String::from_utf8_lossy(&cc_shim.stderr)
            );
        }

        let cc_test = std::process::Command::new(&cc)
            .arg("-c")
            .arg("-O2")
            .arg("-I")
            .arg(lib_dir)
            .arg(&test_c_path)
            .arg("-o")
            .arg(&test_obj)
            .output()?;
        if !cc_test.status.success() {
            anyhow::bail!(
                "Failed to compile original C test suite: {}",
                String::from_utf8_lossy(&cc_test.stderr)
            );
        }

        rust_ffi_module.push_str(
            r#"
extern "C" {
    fn c_test_main() -> i32;
}

fn main() {
    let code = unsafe { c_test_main() };
    if code == 0 {
        println!("L4_TEST_SUITE_PASSED");
    }
    std::process::exit(code);
}
"#,
        );
        std::fs::write(&rust_ffi_path, &rust_ffi_module)?;

        let hybrid_bin = hybrid_dir.join(if cfg!(windows) {
            "l4_test_runner.exe"
        } else {
            "l4_test_runner"
        });

        let rustc_out = std::process::Command::new("rustc")
            .arg("-O")
            .arg(&rust_ffi_path)
            .arg(format!("-Clink-arg={}", shim_obj.display()))
            .arg(format!("-Clink-arg={}", test_obj.display()))
            .arg("-o")
            .arg(&hybrid_bin)
            .output()?;

        if !rustc_out.status.success() {
            anyhow::bail!(
                "Failed to link hybrid C+Rust L4 test binary: {}",
                String::from_utf8_lossy(&rustc_out.stderr)
            );
        }

        let test_run = std::process::Command::new(&hybrid_bin).output()?;
        let l4_passed = test_run.status.success();
        let test_output = format!(
            "{}{}",
            String::from_utf8_lossy(&test_run.stdout),
            String::from_utf8_lossy(&test_run.stderr)
        );

        if l4_passed {
            for rep in &mut function_reports {
                rep.unit.status = UnitStatus::Integrated;
                rep.unit.assurance_level = Some(AssuranceLevel::L4);
                rep.verdict.assurance_level = Some(AssuranceLevel::L4);
                rep.verdict.notes = format!(
                    "{}; L4 original C test suite passed against hybrid build",
                    rep.verdict.notes
                );
            }
        }

        Ok(IntegrationReport {
            library_name: config.library.name,
            units_migrated: function_reports.len(),
            l4_passed,
            test_output,
            generated_c_shim_path: c_shim_path,
            generated_rust_ffi_path: rust_ffi_path,
            function_reports,
        })
    }
}

/// Generates the C source shim replacing the original C body with a call to `ferrite_<name>` (ADR 0005).
pub fn generate_c_delegation_shim(spec: &SignatureSpec) -> String {
    let ret_kw = if spec.c_ret == "void" { "" } else { "return " };
    format!(
        r#"extern {c_ret} ferrite_{name}({c_params});

{c_ret} {name}({c_params}) {{
    {ret_kw}ferrite_{name}({c_args});
}}
"#,
        c_ret = spec.c_ret,
        name = spec.fn_name,
        c_params = spec.c_shim_params(),
        ret_kw = ret_kw,
        c_args = spec.c_call_args()
    )
}

/// Generates the `#[no_mangle] pub extern "C" fn ferrite_<name>` companion wrapper (ADR 0004).
pub fn generate_rust_ffi_companion(spec: &SignatureSpec) -> String {
    let extern_ret = if spec.rust_ret == "()" {
        String::new()
    } else {
        format!(" -> {}", spec.rust_ret)
    };

    let mut conv_lines = String::new();
    let mut call_args = Vec::new();

    for p in &spec.params {
        match p {
            ParamSpec::Scalar { name, .. } => {
                call_args.push(name.clone());
            }
            ParamSpec::SliceU8 {
                name,
                len_name,
                mutable,
            } => {
                if *mutable {
                    conv_lines.push_str(&format!(
                        "    let {name}_slice = if {name}.is_null() || {len} == 0 {{ &mut [] }} else {{ unsafe {{ std::slice::from_raw_parts_mut({name}, {len}) }} }};\n",
                        name = name,
                        len = len_name
                    ));
                } else {
                    conv_lines.push_str(&format!(
                        "    let {name}_slice = if {name}.is_null() || {len} == 0 {{ &[] }} else {{ unsafe {{ std::slice::from_raw_parts({name}, {len}) }} }};\n",
                        name = name,
                        len = len_name
                    ));
                }
                call_args.push(format!("{}_slice", name));
            }
            ParamSpec::CString { name } => {
                conv_lines.push_str(&format!(
                    "    let {name}_str = if {name}.is_null() {{ \"\" }} else {{ unsafe {{ std::ffi::CStr::from_ptr({name}).to_str().unwrap_or(\"\") }} }};\n",
                    name = name
                ));
                call_args.push(format!("{}_str", name));
            }
        }
    }

    format!(
        r#"#[no_mangle]
pub extern "C" fn ferrite_{name}({extern_params}){extern_ret} {{
{conv_lines}    {name}({call_args})
}}
"#,
        name = spec.fn_name,
        extern_params = spec.rust_extern_params(),
        extern_ret = extern_ret,
        conv_lines = conv_lines,
        call_args = call_args.join(", ")
    )
}
