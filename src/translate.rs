//! LLM-based translation of MigrationUnits (ADR 0004 & ADR 0010).

use std::sync::Arc;
use syn::visit::Visit;

use crate::models::{Attempt, CompileResult, MigrationUnit, Stage};
use crate::policy::ModelPolicy;
use crate::provider::{CompletionRequest, LlmProvider};

pub struct Translator<P: ModelPolicy> {
    policy: P,
    provider: Arc<dyn LlmProvider>,
}

impl<P: ModelPolicy> Translator<P> {
    pub fn new(policy: P, provider: Arc<dyn LlmProvider>) -> Self {
        Self { policy, provider }
    }

    /// Produce a single translation attempt for a migration unit.
    pub fn translate_one(
        &self,
        unit: &MigrationUnit,
        history: &[Attempt],
    ) -> anyhow::Result<Attempt> {
        let tier = self.policy.select(unit, Stage::Translate, history);
        let prompt = self.build_prompt(unit);
        let prompt_hash = format!("{:x}", sha2::Sha256::digest(prompt.as_bytes()));

        let req = CompletionRequest {
            function_name: Some(unit.name.clone()),
            prompt,
            model: None,
            temperature: 0.0,
            max_tokens: 2048,
        };
        let completion = self.provider.complete_request(&req)?;
        let raw_response = completion.text;
        let latency_ms = completion.latency_ms;
        let tokens = completion.tokens;
        let cost_usd = completion.cost_usd;

        // Extract fenced ```rust ... ``` code block (ADR 0010)
        let extracted_code = extract_rust_code(&raw_response)?;

        // Validate syntax and signature using `syn` (ADR 0010)
        let (ast_valid, _unsafe_count) = validate_rust_ast(&extracted_code, &unit.name)?;
        if !ast_valid {
            return Ok(Attempt {
                id: uuid(),
                function_id: unit.id.clone(),
                tier,
                stage: Stage::Translate,
                prompt_hash,
                rust_code: extracted_code,
                compile_result: CompileResult {
                    ok: false,
                    errors: vec![format!(
                        "Generated code did not contain function matching contract '{}'",
                        unit.name
                    )],
                    warnings: vec![],
                },
                fuzz_result: None,
                verdict: None,
                tokens,
                cost_usd: Some(cost_usd),
                latency_ms: Some(latency_ms),
                attempt_number: history.len() as u32 + 1,
            });
        }

        // Run compile gate (rustc lib check)
        let compile_result = check_rust_compiles(&extracted_code)?;

        Ok(Attempt {
            id: uuid(),
            function_id: unit.id.clone(),
            tier,
            stage: Stage::Translate,
            prompt_hash,
            rust_code: extracted_code,
            compile_result,
            fuzz_result: None,
            verdict: None,
            tokens,
            cost_usd: Some(cost_usd),
            latency_ms: Some(latency_ms),
            attempt_number: history.len() as u32 + 1,
        })
    }

    pub fn build_prompt(&self, unit: &MigrationUnit) -> String {
        format!(
            r#"You are an expert systems programmer translating C code to idiomatic, safe Rust.

C Signature:
{c_sig}

C Body:
{c_body}

Target Rust Signature Contract:
{rust_contract}

Instructions:
1. Translate the function into 100% safe, idiomatic Rust.
2. Avoid `unsafe` blocks. Do not use raw pointers.
3. Return ONLY a single ```rust ... ``` code block containing the function definition.
4. Do not include extra conversational text.
"#,
            c_sig = unit.signature,
            c_body = unit.source_body,
            rust_contract = unit.rust_contract_sig,
        )
    }
}

use sha2::Digest;

/// Extracts the Rust code snippet enclosed in ```rust ... ``` or ``` ... ```
pub fn extract_rust_code(response: &str) -> anyhow::Result<String> {
    if let Some(start) = response.find("```rust") {
        let code_start = start + 7;
        if let Some(end) = response[code_start..].find("```") {
            return Ok(response[code_start..code_start + end].trim().to_string());
        }
    } else if let Some(start) = response.find("```") {
        let code_start = start + 3;
        if let Some(end) = response[code_start..].find("```") {
            return Ok(response[code_start..code_start + end].trim().to_string());
        }
    }

    // If no fence, assume entire response is code if it starts with 'pub fn' or 'fn'
    if response.trim().starts_with("pub fn") || response.trim().starts_with("fn") {
        return Ok(response.trim().to_string());
    }

    anyhow::bail!("No markdown code block found in LLM response")
}

struct AstVisitor {
    expected_fn_name: String,
    found_expected_fn: bool,
    unsafe_count: u32,
}

impl<'ast> Visit<'ast> for AstVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if node.sig.ident == self.expected_fn_name {
            self.found_expected_fn = true;
        }
        if node.sig.unsafety.is_some() {
            self.unsafe_count += 1;
        }
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        self.unsafe_count += 1;
        syn::visit::visit_expr_unsafe(self, node);
    }
}

/// Parses the code with `syn` to check validity and audit unsafe usage (ADR 0010).
pub fn validate_rust_ast(code: &str, expected_name: &str) -> anyhow::Result<(bool, u32)> {
    let file = syn::parse_file(code)?;
    let mut visitor = AstVisitor {
        expected_fn_name: expected_name.to_string(),
        found_expected_fn: false,
        unsafe_count: 0,
    };
    visitor.visit_file(&file);

    Ok((visitor.found_expected_fn, visitor.unsafe_count))
}

/// Runs `rustc --crate-type lib --emit=metadata` to confirm compilation.
pub fn check_rust_compiles(code: &str) -> anyhow::Result<CompileResult> {
    let temp_dir = tempfile::tempdir()?;
    let file_path = temp_dir.path().join("lib.rs");
    std::fs::write(&file_path, code)?;

    let output = std::process::Command::new("rustc")
        .arg("--crate-type")
        .arg("lib")
        .arg("--emit=metadata")
        .arg(&file_path)
        .arg("--out-dir")
        .arg(temp_dir.path())
        .output();

    match output {
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            if out.status.success() {
                Ok(CompileResult {
                    ok: true,
                    errors: vec![],
                    warnings: if stderr.is_empty() {
                        vec![]
                    } else {
                        vec![stderr]
                    },
                })
            } else {
                Ok(CompileResult {
                    ok: false,
                    errors: vec![stderr],
                    warnings: vec![],
                })
            }
        }
        Err(e) => Ok(CompileResult {
            ok: false,
            errors: vec![format!("Failed to invoke rustc: {}", e)],
            warnings: vec![],
        }),
    }
}

fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:032x}", d)
}
