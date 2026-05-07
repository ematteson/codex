//! Per-project shared notebook for the Explore/Default mode handoff.
//!
//! Discovers `<repo>/.codex/notebook/` by walking up from the current working
//! directory, then exposes simple read helpers that the core session uses to
//! inject the notebook index and active topics into developer instructions.
//! Writes are performed by the model via `apply_patch`; this crate only
//! provides the directory layout helpers and read parsing.

mod index;
mod path;
mod scratch;
mod topic;

pub use index::Index;
pub use index::IndexEntry;
pub use index::load_index;
pub use path::NotebookRoot;
pub use path::discover_notebook_root;
pub use scratch::ensure_scratch_dir;
pub use scratch::scratch_dir;
pub use topic::Topic;
pub use topic::load_topic;
