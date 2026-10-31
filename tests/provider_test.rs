use ferrite::provider::{
    provider_from_env, CompletionRequest, LlmProvider, MockProvider, ReplayProvider,
};
use tempfile::tempdir;

#[test]
fn test_completion_request_and_shim_on_mock_provider() {
    let provider = MockProvider::new();

    // Legacy complete(&str) shim still works
    let text = provider.complete("clamp").expect("shim should succeed");
    assert!(text.contains("pub fn clamp"));

    // Extended complete_request(&CompletionRequest) returns structured Completion
    let req = CompletionRequest {
        function_name: Some("clamp".to_string()),
        prompt: "Translate clamp to Rust".to_string(),
        model: Some("replay-v1".to_string()),
        temperature: 0.0,
        max_tokens: 1024,
    };
    let comp = provider
        .complete_request(&req)
        .expect("complete_request should succeed");
    assert!(comp.text.contains("pub fn clamp"));
    assert_eq!(comp.cost_usd, 0.0);
}

#[test]
fn test_replay_provider_hit_and_miss() {
    let tmp = tempdir().expect("tempdir");
    let fixture_file = tmp.path().join("my_abs.rs.md");
    std::fs::write(
        &fixture_file,
        "```rust\npub fn my_abs(x: i32) -> i32 {\n    x.wrapping_abs()\n}\n```\n",
    )
    .expect("write fixture");

    let replay = ReplayProvider::new(tmp.path());

    // Hit via function_name in CompletionRequest
    let req = CompletionRequest {
        function_name: Some("my_abs".to_string()),
        prompt: "Translate my_abs".to_string(),
        model: None,
        temperature: 0.0,
        max_tokens: 512,
    };
    let comp = replay.complete_request(&req).expect("replay hit");
    assert!(comp.text.contains("pub fn my_abs"));
    assert_eq!(comp.cost_usd, 0.0);

    // Miss gives a clear error mentioning the missing file
    let miss_req = CompletionRequest {
        function_name: Some("nonexistent_fn".to_string()),
        prompt: "Translate nonexistent_fn".to_string(),
        model: None,
        temperature: 0.0,
        max_tokens: 512,
    };
    let err = replay
        .complete_request(&miss_req)
        .expect_err("missing fixture should error");
    let msg = err.to_string();
    assert!(
        msg.contains("nonexistent_fn.rs.md"),
        "Error message should mention missing fixture file, got: {}",
        msg
    );
}

#[test]
fn test_provider_from_env_selection() {
    let p_replay = provider_from_env(Some("replay")).expect("replay provider");
    assert_eq!(p_replay.name(), "replay");

    let p_mock = provider_from_env(Some("mock")).expect("mock provider");
    assert_eq!(p_mock.name(), "mock");
}

#[test]
fn test_openai_compat_provider_against_local_server() {
    use ferrite::provider::OpenAiCompatProvider;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let body = r#"{
                "choices": [{
                    "message": {
                        "content": "```rust\npub fn add(a: i32, b: i32) -> i32 { a + b }\n```"
                    }
                }],
                "usage": {
                    "prompt_tokens": 25,
                    "completion_tokens": 15,
                    "total_tokens": 40
                }
            }"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });

    let base_url = format!("http://{}", addr);
    let provider = OpenAiCompatProvider::new(&base_url, "test-api-key", "nemotron-super-49b")
        .with_budget_usd(1.0);

    let req = CompletionRequest {
        function_name: Some("add".to_string()),
        prompt: "Translate add".to_string(),
        model: None,
        temperature: 0.0,
        max_tokens: 256,
    };

    let comp = provider
        .complete_request(&req)
        .expect("HTTP request to local server should succeed");
    handle.join().expect("server thread join");

    assert!(comp.text.contains("pub fn add"));
    let tokens = comp.tokens.expect("usage tokens parsed");
    assert_eq!(tokens.prompt, 25);
    assert_eq!(tokens.completion, 15);
    assert_eq!(tokens.total, 40);
}
