# Pure Rust Core Orchestrator with Offline Python Analysis

To avoid runtime multi-language dependencies and packaging friction, the Ferrite CLI, orchestrator, and multi-provider LLM client are implemented purely in Rust. Python is reserved strictly for offline tasks (dataset generation, bootstrap confidence intervals, router training, and plotting), exporting its learned router as a static JSON artifact that the Rust binary consumes.
