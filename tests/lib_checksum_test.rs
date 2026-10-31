use ferrite::integrate::Integrator;
use ferrite::models::{AssuranceLevel, VerdictKind};
use ferrite::provider::ReplayProvider;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_lib_checksum_hybrid_build_and_l4_verification() {
    let tmp = tempdir().expect("tempdir");
    let lib_dir = Path::new("bench/lib_checksum");
    let provider = Arc::new(ReplayProvider::new("fixtures/replay"));

    let integrator = Integrator::new(provider, tmp.path());
    let report = integrator
        .migrate_and_verify_l4(lib_dir)
        .expect("migrate_and_verify_l4 should succeed");

    assert_eq!(report.library_name, "lib_checksum");
    assert_eq!(report.units_migrated, 3);
    assert!(
        report.l4_passed,
        "Original C test suite must pass against hybrid C+Rust build: {}",
        report.test_output
    );

    // Every unit must reach L4 assurance after passing both L2 differential fuzzing and L4 original C test suite
    for fn_rep in &report.function_reports {
        assert_eq!(fn_rep.verdict.kind, VerdictKind::Equivalent);
        assert_eq!(fn_rep.verdict.assurance_level, Some(AssuranceLevel::L4));
    }

    // Verify source shim file was generated with `ferrite_<name>` calls (ADR 0005)
    let shim_c = std::fs::read_to_string(&report.generated_c_shim_path).expect("read shim c");
    assert!(shim_c.contains("ferrite_adler32_step"));
    assert!(shim_c.contains("ferrite_crc32_byte"));
    assert!(shim_c.contains("ferrite_crc32_buf"));
}
