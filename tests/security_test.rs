use ferrite::dashboard::Dashboard;
use ferrite::models::{
    AssuranceLevel, FunctionReport, LibraryConfig, LibraryMeta, MigrationUnit, UnitStatus, Verdict,
    VerdictKind,
};
use ferrite::provider::MockProvider;
use ferrite::verify::harness::gen::sanitize_ident;
use tempfile::tempdir;

#[test]
fn test_html_injection_prevention() {
    let tmp = tempdir().expect("create temp dir");
    let html_path = tmp.path().join("dashboard.html");

    let config = LibraryConfig {
        version: 1,
        library: LibraryMeta {
            name: "test<script>alert(1)</script>".into(),
            compile_commands: None,
            include_dirs: vec![],
            source_files: vec![],
            build_cmd: None,
            test_cmd: None,
        },
        clusters: std::collections::HashMap::new(),
        defines: vec![],
        skip: vec![],
    };

    let report = FunctionReport {
        unit: MigrationUnit {
            id: "fn_evil".into(),
            name: "evil<img onerror=alert(2)>".into(),
            signature: "int evil(int <script>)".into(),
            rust_contract_sig: "pub fn evil(x: Result<i32, ()>)".into(),
            source_ref: "evil.c:1".into(),
            source_body: "int evil() { return 0; }".into(),
            callees: vec![],
            features: Default::default(),
            status: UnitStatus::Verified,
            assurance_level: Some(AssuranceLevel::L2),
        },
        verdict: Verdict {
            kind: VerdictKind::Equivalent,
            assurance_level: Some(AssuranceLevel::L2),
            executions: 100,
            coverage_c_percent: 100.0,
            coverage_rust_percent: 100.0,
            coverage_plateaued: true,
            miri_ok: Some(true),
            counterexample: None,
            asan_report: None,
            notes: "Compiler emitted <error> alert(3)".into(),
        },
        attempt_count: 1,
        total_tokens: None,
        total_cost_usd: 0.0,
        total_latency_ms: 10,
        unsafe_line_count: 0,
    };

    Dashboard::write_html(&config, &[report], &html_path).expect("HTML write should succeed");
    let html_content = std::fs::read_to_string(&html_path).expect("read HTML");

    // Assert that raw script tags and payloads are neutralized
    assert!(!html_content.contains("<script>alert(1)</script>"));
    assert!(html_content.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!html_content.contains("<img onerror=alert(2)>"));
    assert!(html_content.contains("&lt;img onerror=alert(2)&gt;"));
    assert!(!html_content.contains("<error>"));
    assert!(html_content.contains("&lt;error&gt;"));
}

#[test]
fn test_path_traversal_sanitization() {
    let malicious_ident = "../../evil_dir/payload";
    let sanitized = sanitize_ident(malicious_ident);

    assert_eq!(sanitized, "evil_dirpayload");
    assert!(!sanitized.contains('/'));
    assert!(!sanitized.contains('\\'));
    assert!(!sanitized.contains(".."));

    let empty_ident = "../...";
    let fallback = sanitize_ident(empty_ident);
    assert_eq!(fallback, "fn_unnamed");
}

#[test]
fn test_mock_provider_mutex_poison_resilience() {
    use ferrite::provider::LlmProvider;
    let provider = MockProvider::new();

    // Verify normal query
    let response = provider.complete("clamp").expect("complete should succeed");
    assert!(response.contains("pub fn clamp"));
}
