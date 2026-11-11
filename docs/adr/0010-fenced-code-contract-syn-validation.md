# ADR-0010: Fenced Markdown Code Block Contract with AST Signature Enforcement

**Date**: 2026-10-22
**Status**: accepted
**Deciders**: Ferrite Architecture Team

## Context

When prompting LLMs across multiple providers (hosted APIs such as Anthropic, OpenAI, Google, or local servers like vLLM and Ollama), enforcing structured output formats is challenging. OpenAI-style JSON schema modes often fail on multiline source code containing quotes, raw backslashes, escape sequences, or regex literals, resulting in JSON parse errors or silent truncation.

Additionally, LLMs frequently take creative liberties with signatures: renaming functions, altering parameter order, returning unrequested types, or introducing unapproved `unsafe` blocks. If unchecked, these violations propagate to `rustc`, producing confusing compiler error diagnostics that waste repair cycles.

## Decision

Standardize model response contracts on plain markdown fenced ````rust```` blocks containing only the safe Rust function implementation.

Before any compilation is attempted, Ferrite extracts the code block and performs pre-flight Abstract Syntax Tree (AST) validation using the `syn` crate:
1. **Signature Contract Check**: Inspects the parsed `syn::ItemFn` to confirm that the function name, parameter count, parameter types, and return type match the expected signature contract exactly.
2. **Safety Audit**: Traverses the AST with a `syn::visit::Visit` visitor to count `unsafe` blocks and keywords. Any unapproved `unsafe` block triggers immediate rejection before compiler invocation.
3. **Structured Diagnostic Feedback**: If AST validation fails, Ferrite reports the exact contract violation back to the model or repair loop.

## Alternatives Considered

### Alternative 1: Strict JSON Schema Structured Outputs
- **Pros**: Constrains model generation at token decoding time on supported providers.
- **Cons**: Not supported uniformly across local models (Ollama, vLLM) or alternative providers; prone to JSON string escaping corruption on multiline code.
- **Why not**: Violates provider agnosticism and creates fragility when parsing complex Rust code.

### Alternative 2: Direct Compilation via `rustc` without Pre-Flight AST Inspection
- **Pros**: Saves the extra parsing step with `syn`.
- **Cons**: Compiler error messages for signature mismatches or unintended `unsafe` blocks are verbose and slow to generate; lacks structural insight into whether the model followed the contract.
- **Why not**: Slower feedback loop and fails to enforce Ferrite's core guarantee that safe translations contain zero `unsafe` blocks.

## Consequences

### Positive
- Works seamlessly across all LLM providers and local models without vendor-specific JSON schema extensions.
- Fast, in-memory validation of function signature and safety invariants before running external compiler commands.
- Clear, actionable error messages sent to the repair loop when a model drifts from the planned contract.

### Negative
- Adds `syn` and `quote` as compile-time dependencies to the Ferrite binary.
- Minor string sanitization required if models wrap responses in conversational preamble before the fenced block.

### Risks
- Code containing brand-new Rust syntax features not yet supported by the pinned `syn` version could fail parsing; mitigated by using the latest stable `syn` 2.0 release.
