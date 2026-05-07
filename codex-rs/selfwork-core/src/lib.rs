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
//! Step 1 (current): governance layer. Base invariants and mode prompts ship
//! as embedded resources via `include_str!`; [`ModeBundle::render_system_prompt`]
//! composes them in the order spec §2.2 requires (invariants on top, mode
//! prompt below). Subsequent steps add filesystem ACLs, handoff schemas,
//! and the eval harness.

pub mod modes;
pub mod workspace;

pub use modes::Mode;
pub use modes::ModeBundle;
pub use workspace::SelfworkRoot;
pub use workspace::discover_root;

/// Crate-version helper used by the CLI to keep the binary `--version` line
/// in sync with the library it links against.
pub const SELFWORK_VERSION: &str = env!("CARGO_PKG_VERSION");
