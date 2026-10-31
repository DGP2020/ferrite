use ferrite::models::{AssuranceLevel, VerdictKind};
use ferrite::verify::stateful::{RingBufOp, StatefulVerifier};
use tempfile::tempdir;

#[test]
fn test_stateful_ringbuffer_reaches_l3_and_detects_state_divergence() {
    let tmp = tempdir().expect("tempdir");
    let stateful = StatefulVerifier::new(tmp.path());

    // 1. Verify random op sequence (push, pop, len, is_empty, clear) against reference Rust RingBuffer
    let ops = StatefulVerifier::generate_op_sequence(0x1234_5678, 500);
    assert_eq!(ops.len(), 500);
    assert!(ops.iter().any(|op| matches!(op, RingBufOp::Push(_))));
    assert!(ops.iter().any(|op| matches!(op, RingBufOp::Pop)));

    let verdict = stateful
        .verify_ringbuffer_l3(false)
        .expect("stateful verification should succeed");
    assert_eq!(verdict.kind, VerdictKind::Equivalent, "{}", verdict.notes);
    assert_eq!(verdict.assurance_level, Some(AssuranceLevel::L3));
    assert!(verdict.executions >= 1000);

    // 2. Inject a state mutation fault in Rust implementation and confirm L3 catches it as Diverged
    let bad_verdict = stateful
        .verify_ringbuffer_l3(true)
        .expect("stateful verification on faulty impl");
    assert_eq!(bad_verdict.kind, VerdictKind::Diverged);
}
