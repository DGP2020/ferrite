# Dual-Layer Translation with Deterministic FFI Generation

Allowing the LLM to write C-ABI signatures produces unidiomatic, raw-pointer-polluted Rust, while manual unsafe lifting is high-risk. We constrain the LLM to generate idiomatic, safe Rust, and use deterministic code generation templates to produce the companion `extern "C"` FFI wrappers and conversion logic shared by both the differential fuzz harness and L4 integration.
