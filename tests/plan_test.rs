use ferrite::models::{FeatureVector, FunctionUnit, UnitStatus};
use ferrite::plan::Planner;

fn make_unit(id: &str, callees: Vec<&str>) -> FunctionUnit {
    FunctionUnit {
        id: id.to_string(),
        name: id.to_string(),
        signature: format!("void {}()", id),
        rust_contract_sig: format!("pub fn {}()", id),
        source_ref: "file.c:1".into(),
        source_body: "body".into(),
        callees: callees.into_iter().map(String::from).collect(),
        features: FeatureVector::default(),
        status: UnitStatus::Analyzed,
        assurance_level: None,
    }
}

#[test]
fn planner_orders_dependencies_bottom_up() {
    // Dependency DAG:
    // A -> B -> (C, D)
    // A has 1 callee (B).
    // B has 2 callees (C, D).
    // C and D have 0 callees.
    //
    // Bottom-up order requires:
    // C and D first, THEN B, THEN A.
    // (A must NEVER appear before B).
    let unit_a = make_unit("A", vec!["B"]);
    let unit_b = make_unit("B", vec!["C", "D"]);
    let unit_c = make_unit("C", vec![]);
    let unit_d = make_unit("D", vec![]);

    // Pass in arbitrary initial order: [A, B, C, D]
    let ordered = Planner::order(vec![unit_a, unit_b, unit_c, unit_d]);

    let pos_a = ordered.iter().position(|u| u.id == "A").unwrap();
    let pos_b = ordered.iter().position(|u| u.id == "B").unwrap();
    let pos_c = ordered.iter().position(|u| u.id == "C").unwrap();
    let pos_d = ordered.iter().position(|u| u.id == "D").unwrap();

    assert!(
        pos_b < pos_a,
        "Callee B (pos {pos_b}) must come before caller A (pos {pos_a})"
    );
    assert!(
        pos_c < pos_b,
        "Callee C (pos {pos_c}) must come before caller B (pos {pos_b})"
    );
    assert!(
        pos_d < pos_b,
        "Callee D (pos {pos_d}) must come before caller B (pos {pos_b})"
    );

    // All units should now have status Planned
    for u in &ordered {
        assert_eq!(u.status, UnitStatus::Planned);
    }
}
