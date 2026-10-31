//! Ferrite: verified C-to-Rust migration agent.
//!
//! Pipeline: analyze -> plan -> translate -> verify -> integrate -> report.
//! The verification engine is the heart: differential fuzzing of a C
//! implementation (compiled with ASan/UBSan) against a Rust translation,
//! with ASan triage to distinguish divergences from latent C bugs.

pub mod analyze;
pub mod bench;
pub mod dashboard;
pub mod integrate;
pub mod models;
pub mod plan;
pub mod policy;
pub mod provider;
pub mod translate;
pub mod verify;

pub use models::{
    AssuranceLevel, Attempt, FunctionReport, FunctionUnit, LibraryConfig, MigrationUnit, Tokens,
    Verdict, VerdictKind,
};
