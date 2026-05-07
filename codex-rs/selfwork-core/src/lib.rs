//! Library crate for the `selfwork` personal-development CLI.
//!
//! Selfwork is a fork of Codex repurposed for introspection work: exploration,
//! reflection, planning, twelve-step companion work, and longitudinal review.
//! The architectural commitment is *mode is governance, not prompt* — modes
//! are deterministic governance bundles (filesystem ACLs, allowed skills,
//! handoff schemas, evals, safety policy) enforced by the host runtime, not
//! by the model's good behavior.
//!
//! See `selfworkDesignSpec.md` at the repo root for the full v1 design.
//!
//! Step 0: scaffolding only. Subsequent steps fill in mode bundles,
//! governance enforcement, handoff compiler, and the eval harness.

/// Crate-version helper used by the CLI to keep the binary `--version` line
/// in sync with the library it links against. Real functionality lands in
/// later steps.
pub const SELFWORK_VERSION: &str = env!("CARGO_PKG_VERSION");
