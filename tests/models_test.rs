use ferrite::models::{Attempt, CompileResult, Stage, Tier, Tokens};

#[test]
fn attempt_roundtrips_through_json_with_tokens() {
    let tokens = Tokens {
        prompt: 150,
        completion: 42,
        total: 192,
    };

    let attempt = Attempt {
        id: "test-attempt-1".into(),
        function_id: "fn_add".into(),
        tier: Tier::Super,
        stage: Stage::Translate,
        prompt_hash: "abc123hash".into(),
        rust_code: "pub fn add(a: i32, b: i32) -> i32 { a + b }".into(),
        compile_result: CompileResult {
            ok: true,
            errors: vec![],
            warnings: vec![],
        },
        fuzz_result: None,
        verdict: None,
        tokens: Some(tokens.clone()),
        cost_usd: Some(0.0015),
        latency_ms: Some(340),
        attempt_number: 1,
    };

    let serialized = serde_json::to_string(&attempt).expect("serialization should succeed");
    let deserialized: Attempt =
        serde_json::from_str(&serialized).expect("deserialization should succeed");

    assert_eq!(deserialized.id, "test-attempt-1");
    assert_eq!(deserialized.tokens, Some(tokens));
    assert_eq!(deserialized.stage, Stage::Translate);
    assert_eq!(deserialized.tier, Tier::Super);
}
