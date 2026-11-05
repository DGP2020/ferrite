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

#[test]
fn analyzer_extracts_call_graph_and_callees() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let c_file = tmp.path().join("graph.c");
    let content = r#"
    int helper_b(int x) {
        return x * 2;
    }

    int helper_c(int y) {
        return y + 1;
    }

    int caller_a(int z) {
        // Calls both helper_b and helper_c
        return helper_b(z) + helper_c(z);
    }
    "#;
    std::fs::write(&c_file, content).expect("write c file");

    let analyzer = Analyzer;
    let units = analyzer.analyze_file(&c_file).expect("analyze file");
    assert_eq!(units.len(), 3);

    let unit_a = units.iter().find(|u| u.name == "caller_a").expect("find caller_a");
    assert_eq!(unit_a.callees.len(), 2);
    assert!(unit_a.callees.contains(&"helper_b".to_string()));
    assert!(unit_a.callees.contains(&"helper_c".to_string()));
    assert_eq!(unit_a.features.callee_count, 2);

    let unit_b = units.iter().find(|u| u.name == "helper_b").expect("find helper_b");
    assert_eq!(unit_b.callees.len(), 0);
    assert_eq!(unit_b.features.callee_count, 0);

    // Plan bottom-up topological sort on extracted units
    let planned = ferrite::plan::Planner::order(units);
    let pos_a = planned.iter().position(|u| u.name == "caller_a").unwrap();
    let pos_b = planned.iter().position(|u| u.name == "helper_b").unwrap();
    let pos_c = planned.iter().position(|u| u.name == "helper_c").unwrap();

    assert!(pos_b < pos_a, "helper_b must be planned before caller_a");
    assert!(pos_c < pos_a, "helper_c must be planned before caller_a");
}
