//! Model-agnostic LLM provider abstraction (ADR 0001 & ADR 0011).

use crate::models::Tokens;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub function_name: Option<String>,
    pub prompt: String,
    pub model: Option<String>,
    pub temperature: f32,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    pub text: String,
    pub tokens: Option<Tokens>,
    pub cost_usd: f64,
    pub latency_ms: u64,
}

pub trait LlmProvider: Send + Sync {
    /// Primary structured completion call returning text, token usage, cost, and latency.
    fn complete_request(&self, req: &CompletionRequest) -> anyhow::Result<Completion> {
        let start = Instant::now();
        let text = self.complete(&req.prompt)?;
        Ok(Completion {
            text,
            tokens: None,
            cost_usd: 0.0,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Backwards-compatible shim for callers passing only a raw prompt string.
    fn complete(&self, prompt: &str) -> anyhow::Result<String>;

    /// Provider identifier (`"replay"`, `"mock"`, or `"openai"`).
    fn name(&self) -> &'static str {
        "custom"
    }
}

/// Offline-first provider that serves human-written reference Rust translations from
/// `fixtures/replay/<fn_name>.rs.md` (ADR 0011).
pub struct ReplayProvider {
    fixtures_dir: PathBuf,
    call_counts: Mutex<HashMap<String, usize>>,
}

impl ReplayProvider {
    pub fn new(fixtures_dir: impl AsRef<Path>) -> Self {
        Self {
            fixtures_dir: fixtures_dir.as_ref().to_path_buf(),
            call_counts: Mutex::new(HashMap::new()),
        }
    }

    fn resolve_fn_name(&self, req: &CompletionRequest) -> Option<String> {
        if let Some(ref name) = req.function_name {
            return Some(name.clone());
        }
        // Fallback: check if any `<stem>.rs.md` in fixtures_dir appears in prompt
        if let Ok(entries) = std::fs::read_dir(&self.fixtures_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if let Some(stem) = file_name.strip_suffix(".rs.md") {
                    if req.prompt.contains(stem) {
                        return Some(stem.to_string());
                    }
                }
            }
        }
        None
    }
}

impl Default for ReplayProvider {
    fn default() -> Self {
        Self::new("fixtures/replay")
    }
}

impl LlmProvider for ReplayProvider {
    fn name(&self) -> &'static str {
        "replay"
    }

    fn complete_request(&self, req: &CompletionRequest) -> anyhow::Result<Completion> {
        let start = Instant::now();
        let fn_name = self.resolve_fn_name(req).ok_or_else(|| {
            anyhow::anyhow!(
                "ReplayProvider could not determine function name from request in '{}'",
                self.fixtures_dir.display()
            )
        })?;

        let attempt_idx = {
            let mut guard = self
                .call_counts
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let entry = guard.entry(fn_name.clone()).or_insert(0);
            let current = *entry;
            *entry += 1;
            current
        };

        // Support multi-step repair demos: if `<fn_name>.attempt1.rs.md` exists on first call, use it
        let candidate_attempt =
            self.fixtures_dir
                .join(format!("{}.attempt{}.rs.md", fn_name, attempt_idx + 1));
        let path = if candidate_attempt.exists() {
            candidate_attempt
        } else {
            self.fixtures_dir.join(format!("{}.rs.md", fn_name))
        };

        if !path.exists() {
            anyhow::bail!(
                "Replay fixture not found: expected '{}' for function '{}'",
                path.display(),
                fn_name
            );
        }

        let text = std::fs::read_to_string(&path)?;
        Ok(Completion {
            text,
            tokens: Some(Tokens {
                prompt: 0,
                completion: 0,
                total: 0,
            }),
            cost_usd: 0.0,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        let req = CompletionRequest {
            function_name: None,
            prompt: prompt.to_string(),
            model: None,
            temperature: 0.0,
            max_tokens: 1024,
        };
        Ok(self.complete_request(&req)?.text)
    }
}

/// A mock provider for deterministic tests, offline benchmarking, and vertical slice execution.
pub struct MockProvider {
    canned_responses: Mutex<HashMap<String, String>>,
}

impl Default for MockProvider {
    fn default() -> Self {
        let mut canned = HashMap::new();

        // Built-in response for clamp
        canned.insert(
            "clamp".to_string(),
            r#"Here is the idiomatic, safe Rust translation of `clamp`:

```rust
pub fn clamp(val: i32, min: i32, max: i32) -> i32 {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}
```
"#
            .to_string(),
        );

        Self {
            canned_responses: Mutex::new(canned),
        }
    }
}

impl MockProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_response(self, fn_name: &str, response: &str) -> Self {
        self.canned_responses
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(fn_name.to_string(), response.to_string());
        self
    }
}

impl LlmProvider for MockProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn complete_request(&self, req: &CompletionRequest) -> anyhow::Result<Completion> {
        let start = Instant::now();
        let text = if let Some(ref fn_name) = req.function_name {
            let guard = self
                .canned_responses
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(resp) = guard.get(fn_name) {
                resp.clone()
            } else {
                drop(guard);
                self.complete(&req.prompt)?
            }
        } else {
            self.complete(&req.prompt)?
        };

        Ok(Completion {
            text,
            tokens: Some(Tokens {
                prompt: (req.prompt.len() / 4) as u64,
                completion: 32,
                total: (req.prompt.len() / 4) as u64 + 32,
            }),
            cost_usd: 0.0,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        let guard = self
            .canned_responses
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for (key, response) in guard.iter() {
            if prompt.contains(key) {
                return Ok(response.clone());
            }
        }

        // Generic fallback for pure functions
        Ok(r#"```rust
pub fn sample_fn(x: i32) -> i32 {
    x
}
```"#
            .to_string())
    }
}

/// OpenAI-compatible HTTP provider supporting local servers and remote endpoints.
pub struct OpenAiCompatProvider {
    api_base: String,
    api_key: String,
    default_model: String,
    timeout: Duration,
    max_retries: u32,
    budget_usd: f64,
    spent_usd: Mutex<f64>,
}

impl OpenAiCompatProvider {
    pub fn new(api_base: &str, api_key: &str, default_model: &str) -> Self {
        Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            default_model: default_model.to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 2,
            budget_usd: 5.0,
            spent_usd: Mutex::new(0.0),
        }
    }

    pub fn with_budget_usd(mut self, budget_usd: f64) -> Self {
        self.budget_usd = budget_usd;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn post_chat_completions(&self, payload: &serde_json::Value) -> anyhow::Result<String> {
        let url = format!("{}/chat/completions", self.api_base);
        let stripped = url
            .strip_prefix("http://")
            .or_else(|| url.strip_prefix("https://"))
            .unwrap_or(&url);
        let (host_port, path) = match stripped.find('/') {
            Some(idx) => (&stripped[..idx], &stripped[idx..]),
            None => (stripped, "/chat/completions"),
        };
        let host_port_addr = if host_port.contains(':') {
            host_port.to_string()
        } else {
            format!("{}:80", host_port)
        };

        let body_str = serde_json::to_string(payload)?;
        let mut last_err = None;

        for _attempt in 0..=self.max_retries {
            match TcpStream::connect(&host_port_addr) {
                Ok(mut stream) => {
                    let _ = stream.set_read_timeout(Some(self.timeout));
                    let _ = stream.set_write_timeout(Some(self.timeout));
                    let req_http = format!(
                        "POST {} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        path,
                        host_port,
                        self.api_key,
                        body_str.len(),
                        body_str
                    );
                    if let Err(e) = stream.write_all(req_http.as_bytes()) {
                        last_err = Some(anyhow::anyhow!("HTTP write error: {}", e));
                        continue;
                    }
                    let mut raw_resp = String::new();
                    if let Err(e) = stream.read_to_string(&mut raw_resp) {
                        last_err = Some(anyhow::anyhow!("HTTP read error: {}", e));
                        continue;
                    }
                    if let Some(split_idx) = raw_resp.find("\r\n\r\n") {
                        let headers = &raw_resp[..split_idx];
                        let body = &raw_resp[split_idx + 4..];
                        if headers.starts_with("HTTP/1.1 200")
                            || headers.starts_with("HTTP/1.0 200")
                        {
                            return Ok(body.to_string());
                        } else {
                            last_err = Some(anyhow::anyhow!(
                                "Non-200 HTTP response from {}: {}",
                                url,
                                headers.lines().next().unwrap_or("unknown")
                            ));
                        }
                    } else {
                        last_err = Some(anyhow::anyhow!("Malformed HTTP response from {}", url));
                    }
                }
                Err(e) => {
                    last_err = Some(anyhow::anyhow!(
                        "Connection to {} failed: {}",
                        host_port_addr,
                        e
                    ));
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("HTTP request failed")))
    }
}

impl LlmProvider for OpenAiCompatProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn complete_request(&self, req: &CompletionRequest) -> anyhow::Result<Completion> {
        {
            let spent = *self
                .spent_usd
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if spent >= self.budget_usd {
                anyhow::bail!(
                    "OpenAiCompatProvider budget cap exceeded (${:.4} >= ${:.4})",
                    spent,
                    self.budget_usd
                );
            }
        }

        let start = Instant::now();
        let model = req
            .model
            .as_deref()
            .unwrap_or(&self.default_model)
            .to_string();

        let payload = serde_json::json!({
            "model": model,
            "temperature": req.temperature,
            "max_tokens": req.max_tokens,
            "messages": [
                { "role": "user", "content": req.prompt }
            ]
        });

        let raw_json = self.post_chat_completions(&payload)?;
        let parsed: serde_json::Value = serde_json::from_str(&raw_json)?;

        let text = parsed["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing choices[0].message.content in response"))?
            .to_string();

        let tokens = parsed.get("usage").map(|usage| Tokens {
            prompt: usage["prompt_tokens"].as_u64().unwrap_or(0),
            completion: usage["completion_tokens"].as_u64().unwrap_or(0),
            total: usage["total_tokens"].as_u64().unwrap_or(0),
        });

        let call_cost = tokens
            .as_ref()
            .map(|t| (t.total as f64) * 0.000_001)
            .unwrap_or(0.0001);

        {
            let mut spent = self
                .spent_usd
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *spent += call_cost;
        }

        Ok(Completion {
            text,
            tokens,
            cost_usd: call_cost,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        let req = CompletionRequest {
            function_name: None,
            prompt: prompt.to_string(),
            model: None,
            temperature: 0.0,
            max_tokens: 1024,
        };
        Ok(self.complete_request(&req)?.text)
    }
}

/// Factory creating the configured `LlmProvider` from CLI override or `FERRITE_PROVIDER` env var.
pub fn provider_from_env(cli_override: Option<&str>) -> anyhow::Result<Arc<dyn LlmProvider>> {
    let env_val = std::env::var("FERRITE_PROVIDER").ok();
    let choice = cli_override
        .or(env_val.as_deref())
        .unwrap_or("replay")
        .to_lowercase();

    match choice.as_str() {
        "replay" => {
            let dir = std::env::var("FERRITE_REPLAY_DIR")
                .unwrap_or_else(|_| "fixtures/replay".to_string());
            Ok(Arc::new(ReplayProvider::new(dir)))
        }
        "mock" => Ok(Arc::new(MockProvider::new())),
        "openai" => {
            let base = std::env::var("FERRITE_API_BASE")
                .unwrap_or_else(|_| "http://127.0.0.1:8000/v1".to_string());
            let key = std::env::var("FERRITE_API_KEY").unwrap_or_default();
            let model = std::env::var("FERRITE_MODEL")
                .unwrap_or_else(|_| "nvidia/Llama-3.3-Nemotron-Super-49B-v1".to_string());
            Ok(Arc::new(OpenAiCompatProvider::new(&base, &key, &model)))
        }
        other => anyhow::bail!(
            "Unknown provider '{}': expected 'replay', 'mock', or 'openai'",
            other
        ),
    }
}
