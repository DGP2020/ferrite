# Source-Level Shim Injection with Per-Library ferrite.toml Configuration

Linker wrapping (`--wrap`) fails on internal static calls within the same translation unit, and archive symbol overriding is compiler- and platform-fragile. For L4 hybrid integration, Ferrite replaces C function bodies with explicit delegation shims calling exported Rust symbols, and relies on per-library build and test commands configured in `ferrite.toml` rather than attempting universal build-system synthesis.
