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

pub mod bootstrap;
pub mod handoff;
pub mod journal;
pub mod manifest;
pub mod modes;
pub mod runtime;
pub mod state;
pub mod workspace;

pub use bootstrap::BootstrapReport;
pub use bootstrap::bootstrap_workspace;
pub use handoff::HandoffExtension;
pub use handoff::HandoffRiskFlag;
pub use handoff::MigrationObject;
pub use handoff::compile_handoff;
pub use handoff::handoff_schema_json;
pub use handoff::render_handoff_developer_instructions;
pub use handoff::write_explore_session_summary;
pub use handoff::write_migration;
pub use journal::JournalEntry;
pub use journal::ensure_journal_entry;
pub use manifest::ModeManifest;
pub use manifest::load_mode_manifest;
pub use manifest::mode_manifest_yaml;
pub use manifest::permission_profile_for_manifest;
pub use manifest::permission_profile_for_mode;
pub use modes::Mode;
pub use modes::ModeBundle;
pub use runtime::CodexRuntimeBuilder;
pub use runtime::OneShotOutput;
pub use runtime::SelfworkSession;
pub use runtime::SelfworkTurn;
pub use runtime::run_one_shot;
pub use state::ActiveMode;
pub use state::RiskFlags;
pub use state::SessionState;
pub use state::load_active_mode;
pub use state::load_risk_flags;
pub use state::load_session_state;
pub use state::save_active_mode;
pub use state::save_session_state;
pub use workspace::SelfworkRoot;
pub use workspace::discover_root;

/// Crate-version helper used by the CLI to keep the binary `--version` line
/// in sync with the library it links against.
pub const SELFWORK_VERSION: &str = env!("CARGO_PKG_VERSION");
