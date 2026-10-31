# Phase 1 Signature Scope: Primitives, Slices, Strings, and Out-Pointers

To rapidly build and validate the core verification pipeline without getting stalled by struct layout mechanics, Phase 1 strictly targets primitive scalars, bounded buffer slices, NUL-terminated strings, and scalar out-pointers (`T *out` mapped to Rust returns). Structs, unions, function pointers, and varargs are flagged as unsupported until Phase 2.
