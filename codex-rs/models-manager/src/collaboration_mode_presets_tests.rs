use super::*;
use pretty_assertions::assert_eq;

#[test]
fn preset_names_use_mode_display_names() {
    assert_eq!(plan_preset().name, ModeKind::Plan.display_name());
    assert_eq!(default_preset().name, ModeKind::Default.display_name());
    assert_eq!(explore_preset().name, ModeKind::Explore.display_name());
    assert_eq!(plan_preset().model, None);
    assert_eq!(
        plan_preset().reasoning_effort,
        Some(Some(ReasoningEffort::Medium))
    );
    assert_eq!(default_preset().model, None);
    assert_eq!(default_preset().reasoning_effort, None);
    assert_eq!(explore_preset().model, None);
    assert_eq!(
        explore_preset().reasoning_effort,
        Some(Some(ReasoningEffort::XHigh))
    );
}

#[test]
fn explore_preset_uses_explore_template() {
    let instructions = explore_preset()
        .developer_instructions
        .expect("explore preset should include instructions")
        .expect("explore instructions should be set");
    let normalized = instructions.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(normalized.contains("Collaboration Mode: Explore"));
    // Boundaries: read-only, notebook-only writes, no silent implementation.
    assert!(normalized.contains("Do not implement"));
    assert!(normalized.contains(".codex/notebook/"));
    assert!(normalized.contains("execution-ready handoff"));
    // The four properties this mode exists for. Each is a behaviour, not a
    // phrase — reword freely, but do not drop one silently.
    assert!(normalized.contains("Truth-seeking"), "truth-seeking section");
    assert!(
        normalized.contains("Prefer the check that could disconfirm you"),
        "falsification bias"
    );
    assert!(normalized.contains("Candour"), "anti-sycophancy section");
    assert!(
        normalized.contains("Disagree when the evidence points the other way"),
        "willingness to disagree"
    );
    assert!(
        normalized.contains("are claims, not facts"),
        "cross-checking other agents' work"
    );
    assert!(
        normalized.contains("breadth for its own sake is noise"),
        "generative exploration guidance"
    );
    assert!(
        normalized.contains("Exploration need not converge"),
        "no forced convergence"
    );
}

#[test]
fn builtin_presets_includes_explore() {
    let presets = builtin_collaboration_mode_presets();
    assert!(presets.iter().any(|p| p.mode == Some(ModeKind::Explore)));
}

#[test]
fn default_mode_instructions_replace_mode_names_placeholder() {
    let default_instructions = default_preset()
        .developer_instructions
        .expect("default preset should include instructions")
        .expect("default instructions should be set");

    assert!(!default_instructions.contains("{{KNOWN_MODE_NAMES}}"));

    let known_mode_names = format_mode_names(&TUI_VISIBLE_COLLABORATION_MODES);
    let expected_snippet = format!("Known mode names are {known_mode_names}.");
    assert!(default_instructions.contains(&expected_snippet));

    assert!(default_instructions.contains(
        "Use the `request_user_input` tool only when it is listed in the available tools"
    ));
    assert!(
        default_instructions.contains("ask the user directly with a concise plain-text question")
    );
}
