use ferrite::analyze::Analyzer;

#[test]
fn features_detects_loops_and_pointers() {
    let body = r#"
    int compute(int x) {
        int sum = 0;
        for (int i = 0; i < x; i++) {
            sum += i;
        }
        return sum;
    }
    "#;

    let fv = Analyzer::features(body, 0);
    assert!(fv.has_loops, "should detect for loop");
    assert!(!fv.has_pointers, "should not have pointers");
    assert!(fv.harnessable, "scalar loop function is harnessable");
}

#[test]
fn features_harnessable_for_pointer_buffer_functions() {
    // Buffer function using pointer arithmetic (no brackets '[' and no 'char *')
    let body = r#"
    int sum_bytes(const uint8_t *buf, size_t len) {
        int sum = 0;
        for (size_t i = 0; i < len; ++i) {
            sum += *(buf + i);
        }
        return sum;
    }
    "#;

    let fv = Analyzer::features(body, 0);
    assert!(fv.has_pointers, "should detect pointer");
    assert!(fv.has_loops, "should detect loop");
    assert!(
        fv.harnessable,
        "pointer-length buffer functions must be harnessable per verification engine specs"
    );
}
