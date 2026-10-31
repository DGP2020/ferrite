use ferrite::bench::{BenchManifest, BenchRunner};
use ferrite::models::VerdictKind;
use ferrite::provider::ReplayProvider;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_bench_manifest_parses_all_tiers_and_planted_bugs() {
    let manifest_path = Path::new("bench/manifest.toml");
    let manifest = BenchManifest::load(manifest_path).expect("load bench/manifest.toml");

    assert!(
        manifest.entries.len() >= 25,
        "Bench lite must have ~30 functions across 3 tiers, got {}",
        manifest.entries.len()
    );

    let tier1_count = manifest.entries.iter().filter(|e| e.tier == 1).count();
    let tier2_count = manifest.entries.iter().filter(|e| e.tier == 2).count();
    let tier3_count = manifest.entries.iter().filter(|e| e.tier == 3).count();
    let bug_count = manifest
        .entries
        .iter()
        .filter(|e| e.expected_verdict == VerdictKind::CLatentBug)
        .count();

    assert!(tier1_count >= 10, "Expected ~12 tier 1 functions");
    assert!(tier2_count >= 8, "Expected ~10 tier 2 functions");
    assert!(tier3_count >= 4, "Expected ~5 tier 3 functions");
    assert!(
        bug_count >= 3,
        "Expected at least 3 planted C latent bugs, got {}",
        bug_count
    );
}

#[test]
fn test_bench_runner_executes_subset_and_writes_json_results() {
    let tmp = tempdir().expect("tempdir");
    let manifest_path = Path::new("bench/manifest.toml");
    let mut manifest = BenchManifest::load(manifest_path).expect("load manifest");

    // Pick 1 clean Tier 1, 1 clean Tier 2, and 1 planted bug for fast unit test
    manifest.entries.retain(|e| {
        e.name == "clamp_i32" || e.name == "sum_bytes" || e.name == "bug_off_by_one_read"
    });
    assert_eq!(manifest.entries.len(), 3);

    let provider = Arc::new(ReplayProvider::new("fixtures/replay"));
    let runner = BenchRunner::new(provider, tmp.path());
    let summary = runner
        .run_manifest(&manifest, Path::new("."))
        .expect("run_manifest should succeed");

    assert_eq!(summary.total_functions, 3);
    assert_eq!(summary.matched_expectations, 3);
    assert_eq!(summary.c_latent_bugs_found, 1);

    let json_out = tmp.path().join("bench-results.json");
    assert!(json_out.exists(), "bench-results.json must be written");

    let table = summary.format_leaderboard();
    assert!(table.contains("REPLAY MODE"));
    assert!(table.contains("clamp_i32"));
    assert!(table.contains("bug_off_by_one_read"));
}
