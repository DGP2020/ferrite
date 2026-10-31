use std::sync::Arc;
use tempfile::tempdir;

use ferrite::analyze::Analyzer;
use ferrite::dashboard::Dashboard;
use ferrite::models::{AssuranceLevel, FunctionReport, LibraryConfig, LibraryMeta, VerdictKind};
use ferrite::policy::StaticPolicy;
use ferrite::provider::MockProvider;
use ferrite::translate::{validate_rust_ast, Translator};
use ferrite::verify::Verifier;

#[test]
fn test_vertical_slice_clamp_end_to_end() {
    let tmp = tempdir().expect("create temp work dir");

    // 1. Analyze clamp.c fixture
    let analyzer = Analyzer;
    let fixture_path = std::path::Path::new("fixtures/clamp/clamp.c");
    let units = analyzer
        .analyze_file(fixture_path)
        .expect("analysis should succeed");

    assert_eq!(
        units.len(),
        1,
        "Expected exactly 1 migration unit for clamp"
    );
    let unit = &units[0];
    assert_eq!(unit.name, "clamp");
    assert_eq!(
        unit.rust_contract_sig,
        "pub fn clamp(val: i32, min: i32, max: i32) -> i32"
    );

    // 2. Translate with MockProvider & syn AST validation
    let provider = Arc::new(MockProvider::new());
    let translator = Translator::new(StaticPolicy::default(), provider);
    let attempt = translator
        .translate_one(unit, &[])
        .expect("translation should succeed");

    assert!(attempt.compile_result.ok, "Translation must compile (L0)");

    let (valid_ast, unsafe_count) =
        validate_rust_ast(&attempt.rust_code, &unit.name).expect("AST validation should succeed");
    assert!(valid_ast, "Must contain matching clamp function");
    assert_eq!(unsafe_count, 0, "Must be 100% safe Rust (0 unsafe blocks)");

    // 3. Verify via differential fuzzing
    let verifier = Verifier::new(tmp.path());
    let verdict = verifier
        .verify(unit, &attempt.rust_code)
        .expect("verification should succeed");

    assert_eq!(
        verdict.kind,
        VerdictKind::Equivalent,
        "Clamp translation must be equivalent to original C: {}",
        verdict.notes
    );
    assert_eq!(
        verdict.assurance_level,
        Some(AssuranceLevel::L2),
        "Must reach L2 assurance (differentially tested)"
    );
    assert!(
        verdict.executions >= 10_000,
        "Must achieve at least 10,000 differential executions"
    );

    // 4. Generate Dashboard
    let config = LibraryConfig {
        version: 1,
        library: LibraryMeta {
            name: "clamp_utility".into(),
            compile_commands: None,
            include_dirs: vec![],
            source_files: vec!["clamp.c".into()],
            build_cmd: None,
            test_cmd: None,
        },
        clusters: std::collections::HashMap::new(),
        defines: vec![],
        skip: vec![],
    };

    let report = FunctionReport {
        unit: unit.clone(),
        verdict,
        attempt_count: 1,
        total_tokens: None,
        total_cost_usd: 0.001,
        total_latency_ms: 35,
        unsafe_line_count: unsafe_count,
    };

    let html_path = tmp.path().join("dashboard.html");
    Dashboard::write_html(&config, &[report], &html_path).expect("Dashboard write must succeed");
    assert!(html_path.exists(), "HTML dashboard artifact must exist");
}

#[test]
fn test_cli_help_includes_provider_and_bench_subcommand() {
    let bin = env!("CARGO_BIN_EXE_ferrite");
    let out = std::process::Command::new(bin)
        .arg("--help")
        .output()
        .expect("run ferrite --help");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("--provider"),
        "CLI help must include --provider flag"
    );
    assert!(
        stdout.contains("bench"),
        "CLI help must include bench subcommand"
    );
}
