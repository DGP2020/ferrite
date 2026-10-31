# Fenced Markdown Code Block Contract with AST Signature Enforcement

Structured JSON output for multiline code blocks across diverse LLM providers is prone to escaping errors and schema rejections. The translator prompt instructs models to return only a fenced ````rust```` block containing the safe implementation, and Ferrite parses the response using `syn` to verify that the AST matches the declared signature contract and to audit `unsafe` usage before compilation.
