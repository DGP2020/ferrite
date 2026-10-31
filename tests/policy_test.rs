use ferrite::models::{
    Attempt, CompileResult, FeatureVector, FunctionUnit, Stage, Tier, UnitStatus, Verdict,
    VerdictKind,
};
use ferrite::policy::{CascadePolicy, ModelPolicy, StaticPolicy};

fn dummy_unit() -> FunctionUnit {
    FunctionUnit {
        id: "fn_1".into(),
        name: "test_fn".into(),
        signature: "int test_fn(int x)".into(),
        rust_contract_sig: "pub fn test_fn(x: i32) -> i32".into(),
        source_ref: "test.c:1".into(),
        source_body: "int test_fn(int x) { return x * 2; }".into(),
        callees: vec![],
        features: FeatureVector::default(),
        status: UnitStatus::Analyzed,
        assurance_level: None,
    }
}

fn dummy_attempt(stage: Stage, tier: Tier, compile_ok: bool, verdict_kind: VerdictKind) -> Attempt {
    Attempt {
        id: "att_1".into(),
        function_id: "fn_1".into(),
        tier,
        stage,
        prompt_hash: "hash".into(),
        rust_code: "code".into(),
        compile_result: CompileResult {
            ok: compile_ok,
            errors: if compile_ok {
                vec![]
            } else {
                vec!["err".into()]
            },
            warnings: vec![],
        },
        fuzz_result: None,
        verdict: Some(Verdict {
            kind: verdict_kind,
            assurance_level: None,
            executions: 100,
            coverage_c_percent: 50.0,
            coverage_rust_percent: 50.0,
            coverage_plateaued: true,
            miri_ok: Some(true),
            counterexample: None,
            asan_report: None,
            notes: "".into(),
        }),
        tokens: None,
        cost_usd: None,
        latency_ms: None,
        attempt_number: 1,
    }
}

#[test]
fn static_policy_selects_default_and_escalates_repair() {
    let policy = StaticPolicy::default();
    let unit = dummy_unit();

    assert_eq!(policy.select(&unit, Stage::Plan, &[]), Tier::Ultra);
    assert_eq!(policy.select(&unit, Stage::Translate, &[]), Tier::Super);
    assert_eq!(policy.select(&unit, Stage::Seed, &[]), Tier::Nano);
    assert_eq!(policy.select(&unit, Stage::Repair, &[]), Tier::Super);

    // After 1 failed attempt, still Super (default escalate_after is 2)
    let history_one_fail = vec![dummy_attempt(
        Stage::Repair,
        Tier::Super,
        false,
        VerdictKind::CompileFailed,
    )];
    assert_eq!(
        policy.select(&unit, Stage::Repair, &history_one_fail),
        Tier::Super
    );

    // After 2 failed attempts, escalates to Ultra
    let history_two_fails = vec![
        dummy_attempt(
            Stage::Repair,
            Tier::Super,
            false,
            VerdictKind::CompileFailed,
        ),
        dummy_attempt(
            Stage::Repair,
            Tier::Super,
            false,
            VerdictKind::CompileFailed,
        ),
    ];
    assert_eq!(
        policy.select(&unit, Stage::Repair, &history_two_fails),
        Tier::Ultra
    );
}

#[test]
fn cascade_policy_escalates_tier_on_fuzz_divergence() {
    let policy = CascadePolicy::default();
    let unit = dummy_unit();

    // With no history, starts at min_tier (Nano)
    assert_eq!(policy.select(&unit, Stage::Translate, &[]), Tier::Nano);

    // One divergence in translate escalates to Super
    let history_one_diverged = vec![dummy_attempt(
        Stage::Translate,
        Tier::Nano,
        true,
        VerdictKind::Diverged,
    )];
    assert_eq!(
        policy.select(&unit, Stage::Translate, &history_one_diverged),
        Tier::Super
    );
}

#[test]
fn boxed_policy_dispatches_correctly() {
    let boxed: Box<dyn ModelPolicy> = Box::new(CascadePolicy::default());
    let unit = dummy_unit();
    assert_eq!(boxed.select(&unit, Stage::Translate, &[]), Tier::Nano);
}
