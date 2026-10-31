use ferrite::models::{FeatureVector, FunctionUnit, UnitStatus, VerdictKind};
use ferrite::verify::Verifier;
use tempfile::tempdir;

fn dummy_unit() -> FunctionUnit {
    FunctionUnit {
        id: "fn_add".into(),
        name: "add".into(),
        signature: "int add(int a, int b)".into(),
        rust_contract_sig: "pub fn add(a: i32, b: i32) -> i32".into(),
        source_ref: "math.c:10".into(),
        source_body: "int add(int a, int b) { return a + b; }".into(),
        callees: vec![],
        features: FeatureVector::default(),
        status: UnitStatus::Analyzed,
        assurance_level: None,
    }
}

#[test]
fn verifier_creates_workspace_directories() {
    let tmp = tempdir().expect("create temp dir");
    let _verifier = Verifier::new(tmp.path());

    assert!(
        tmp.path().join("harnesses").exists(),
        "harnesses directory must be created"
    );
    assert!(
        tmp.path().join("corpus").exists(),
        "corpus directory must be created"
    );
}

#[test]
fn verifier_rejects_invalid_rust_code_with_compile_failed() {
    let tmp = tempdir().expect("create temp dir");
    let verifier = Verifier::new(tmp.path());
    let unit = dummy_unit();

    let invalid_rust = "fn broken( { invalid syntax";
    let verdict = verifier
        .verify(&unit, invalid_rust)
        .expect("verify should return Ok(verdict)");

    assert_eq!(
        verdict.kind,
        VerdictKind::CompileFailed,
        "Broken code must produce CompileFailed verdict"
    );
    assert!(
        !verdict.notes.is_empty(),
        "CompileFailed verdict must include error notes"
    );
}

#[test]
fn verifier_verifies_two_arg_scalar_and_detects_divergence() {
    let tmp = tempdir().expect("create temp dir");
    let verifier = Verifier::new(tmp.path());
    let unit = dummy_unit(); // int add(int a, int b)

    let good_rust = "pub fn add(a: i32, b: i32) -> i32 { a.wrapping_add(b) }";
    let v_ok = verifier.verify(&unit, good_rust).expect("verify ok");
    assert_eq!(v_ok.kind, VerdictKind::Equivalent, "notes: {}", v_ok.notes);

    let bad_rust = "pub fn add(a: i32, b: i32) -> i32 { a.wrapping_sub(b) }";
    let v_div = verifier.verify(&unit, bad_rust).expect("verify diverged");
    assert_eq!(v_div.kind, VerdictKind::Diverged);
}

#[test]
fn verifier_verifies_slice_and_string_signatures() {
    let tmp = tempdir().expect("create temp dir");
    let verifier = Verifier::new(tmp.path());

    let slice_unit = FunctionUnit {
        id: "fn_sum_bytes".into(),
        name: "sum_bytes".into(),
        signature: "uint32_t sum_bytes(const uint8_t *buf, size_t len)".into(),
        rust_contract_sig: "pub fn sum_bytes(buf: &[u8]) -> u32".into(),
        source_ref: "buf.c:1".into(),
        source_body: "uint32_t sum_bytes(const uint8_t *buf, size_t len) { uint32_t s = 0; for (size_t i = 0; i < len; i++) { s += buf[i]; } return s; }".into(),
        callees: vec![],
        features: FeatureVector {
            has_pointers: true,
            buffer_length_pairs: 1,
            harnessable: true,
            ..Default::default()
        },
        status: UnitStatus::Analyzed,
        assurance_level: None,
    };

    let slice_rust = "pub fn sum_bytes(buf: &[u8]) -> u32 { buf.iter().map(|&b| b as u32).sum() }";
    let v_slice = verifier
        .verify(&slice_unit, slice_rust)
        .expect("verify slice");
    assert_eq!(
        v_slice.kind,
        VerdictKind::Equivalent,
        "slice verification failed: {}",
        v_slice.notes
    );

    let str_unit = FunctionUnit {
        id: "fn_count_vowels".into(),
        name: "count_vowels".into(),
        signature: "size_t count_vowels(const char *s)".into(),
        rust_contract_sig: "pub fn count_vowels(s: &str) -> usize".into(),
        source_ref: "str.c:1".into(),
        source_body: "size_t count_vowels(const char *s) { size_t c = 0; for (size_t i = 0; s[i] != '\\0'; i++) { char ch = s[i]; if (ch=='a'||ch=='e'||ch=='i'||ch=='o'||ch=='u') c++; } return c; }".into(),
        callees: vec![],
        features: FeatureVector {
            has_pointers: true,
            has_strings: true,
            harnessable: true,
            ..Default::default()
        },
        status: UnitStatus::Analyzed,
        assurance_level: None,
    };

    let str_rust = "pub fn count_vowels(s: &str) -> usize { s.bytes().filter(|&b| matches!(b, b'a'|b'e'|b'i'|b'o'|b'u')).count() }";
    let v_str = verifier.verify(&str_unit, str_rust).expect("verify str");
    assert_eq!(
        v_str.kind,
        VerdictKind::Equivalent,
        "str verification failed: {}",
        v_str.notes
    );
}

#[test]
fn verifier_triages_c_latent_bug() {
    let tmp = tempdir().expect("create temp dir");
    let verifier = Verifier::new(tmp.path());

    // C function with a planted signed integer overflow UB and OOB read on empty slice
    let buggy_c_unit = FunctionUnit {
        id: "fn_first_byte_or_zero".into(),
        name: "first_byte_or_zero".into(),
        signature: "uint32_t first_byte_or_zero(const uint8_t *buf, size_t len)".into(),
        rust_contract_sig: "pub fn first_byte_or_zero(buf: &[u8]) -> u32".into(),
        source_ref: "buggy.c:1".into(),
        source_body: r#"uint32_t first_byte_or_zero(const uint8_t *buf, size_t len) {
    /* BUG: off-by-one read at buf[len] when len == 0 or out of bounds */
    if (len == 0) {
        return (uint32_t)buf[len];
    }
    return (uint32_t)buf[0];
}"#
        .into(),
        callees: vec![],
        features: FeatureVector {
            has_pointers: true,
            buffer_length_pairs: 1,
            harnessable: true,
            ..Default::default()
        },
        status: UnitStatus::Analyzed,
        assurance_level: None,
    };

    let safe_rust =
        "pub fn first_byte_or_zero(buf: &[u8]) -> u32 { buf.first().copied().unwrap_or(0) as u32 }";
    let verdict = verifier
        .verify(&buggy_c_unit, safe_rust)
        .expect("verify latent bug");
    assert_eq!(
        verdict.kind,
        VerdictKind::CLatentBug,
        "Expected CLatentBug verdict, got {:?}: {}",
        verdict.kind,
        verdict.notes
    );
}
