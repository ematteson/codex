//! Mode bundle composition (Step 1 — governance layer).
//!
//! A `ModeBundle` is the runtime view of a selfwork mode. In v1 it carries
//! only the embedded base invariants and the mode-specific prompt; future
//! steps will add filesystem ACLs (read/write paths), allowed skills, the
//! handoff schema, and the eval manifest.
//!
//! Composition rule (spec §2.2): base invariants are loaded *under* every
//! mode prompt, never overridden by it. `render_system_prompt` enforces
//! this ordering by always emitting invariants first, separated from the
//! mode prompt by a blank line.

const BASE_INVARIANTS: &str = include_str!("../resources/base/invariants.md");
const EXPLORE_PROMPT: &str = include_str!("../resources/modes/explore/prompt.md");

/// The five modes specified for selfwork v1. Plan, Reflect, Program, and
/// Review are declared but their bundles arrive in later migration steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Explore,
    Plan,
    Reflect,
    Program,
    Review,
}

impl Mode {
    pub const fn display_name(self) -> &'static str {
        match self {
            Mode::Explore => "Explore",
            Mode::Plan => "Plan",
            Mode::Reflect => "Reflect",
            Mode::Program => "Program",
            Mode::Review => "Review",
        }
    }

    /// Whether the bundle for this mode is implemented yet. Step 1 only ships
    /// Explore; later steps flip the rest. The CLI uses this to refuse entry
    /// to modes that have not landed.
    pub const fn is_implemented(self) -> bool {
        matches!(self, Mode::Explore)
    }
}

#[derive(Debug, Clone)]
pub struct ModeBundle {
    pub mode: Mode,
    pub base_invariants: &'static str,
    pub mode_prompt: &'static str,
}

impl ModeBundle {
    /// Returns the bundle for `mode` if implemented, or `None` if the bundle
    /// has not landed yet (the CLI should refuse entry in that case).
    pub fn for_mode(mode: Mode) -> Option<Self> {
        let mode_prompt = match mode {
            Mode::Explore => EXPLORE_PROMPT,
            Mode::Plan | Mode::Reflect | Mode::Program | Mode::Review => return None,
        };
        Some(Self {
            mode,
            base_invariants: BASE_INVARIANTS,
            mode_prompt,
        })
    }

    /// Compose the full system prompt: base invariants first (immutable),
    /// then a blank line, then the mode-specific prompt. This ordering is
    /// load-bearing — the mode prompt cannot override anything stated in the
    /// invariants because invariants are fed to the model first.
    pub fn render_system_prompt(&self) -> String {
        let mut out = String::with_capacity(self.base_invariants.len() + self.mode_prompt.len() + 2);
        out.push_str(self.base_invariants.trim_end());
        out.push_str("\n\n");
        out.push_str(self.mode_prompt.trim_start());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn explore_bundle_is_available() {
        let bundle = ModeBundle::for_mode(Mode::Explore).expect("Explore bundle");
        assert_eq!(bundle.mode, Mode::Explore);
        assert!(!bundle.base_invariants.is_empty());
        assert!(!bundle.mode_prompt.is_empty());
    }

    #[test]
    fn unimplemented_modes_return_none() {
        assert!(ModeBundle::for_mode(Mode::Plan).is_none());
        assert!(ModeBundle::for_mode(Mode::Reflect).is_none());
        assert!(ModeBundle::for_mode(Mode::Program).is_none());
        assert!(ModeBundle::for_mode(Mode::Review).is_none());
    }

    #[test]
    fn render_puts_invariants_before_mode_prompt() {
        let rendered = ModeBundle::for_mode(Mode::Explore)
            .expect("Explore bundle")
            .render_system_prompt();
        let invariants_idx = rendered
            .find("# Base Invariants")
            .expect("invariants header present");
        let mode_idx = rendered
            .find("# Mode: Explore")
            .expect("mode header present");
        assert!(
            invariants_idx < mode_idx,
            "invariants must appear before the mode prompt: invariants_idx={invariants_idx}, mode_idx={mode_idx}"
        );
    }

    #[test]
    fn render_includes_load_bearing_invariant_phrases() {
        let rendered = ModeBundle::for_mode(Mode::Explore)
            .expect("Explore bundle")
            .render_system_prompt();
        // These phrases anchor the safety contract — if any goes missing the
        // governance layer is broken regardless of compile success.
        assert!(rendered.contains("No professional impersonation"));
        assert!(rendered.contains("Crisis override"));
        assert!(rendered.contains("Anti-dependency stance"));
        assert!(rendered.contains("No claim to lived experience or spiritual authority"));
        assert!(rendered.contains("User agency preserved"));
    }

    #[test]
    fn render_includes_explore_posture_marker() {
        let rendered = ModeBundle::for_mode(Mode::Explore)
            .expect("Explore bundle")
            .render_system_prompt();
        assert!(rendered.contains("Socratic"));
        assert!(rendered.contains("What I'm hearing"));
        assert!(rendered.contains("Possible patterns"));
        assert!(rendered.contains("Questions worth sitting with"));
        assert!(rendered.contains("Optional next step"));
    }

    #[test]
    fn render_separates_invariants_from_mode_with_blank_line() {
        let rendered = ModeBundle::for_mode(Mode::Explore)
            .expect("Explore bundle")
            .render_system_prompt();
        // Every invariants block ends with the User-agency paragraph; the mode
        // prompt opens with `# Mode: Explore`. The separator must include a
        // blank line so the two markdown blocks render as separate sections.
        assert!(rendered.contains("\n\n# Mode: Explore"));
    }

    #[test]
    fn display_names_match_spec_capitalization() {
        assert_eq!(Mode::Explore.display_name(), "Explore");
        assert_eq!(Mode::Plan.display_name(), "Plan");
        assert_eq!(Mode::Reflect.display_name(), "Reflect");
        assert_eq!(Mode::Program.display_name(), "Program");
        assert_eq!(Mode::Review.display_name(), "Review");
    }

    #[test]
    fn is_implemented_only_returns_true_for_explore_in_step_one() {
        assert!(Mode::Explore.is_implemented());
        assert!(!Mode::Plan.is_implemented());
        assert!(!Mode::Reflect.is_implemented());
        assert!(!Mode::Program.is_implemented());
        assert!(!Mode::Review.is_implemented());
    }
}
