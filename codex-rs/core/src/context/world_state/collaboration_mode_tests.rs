use super::super::PreviousSectionState;
use super::super::test_support::render_section_cases;
use super::*;
use crate::context::world_state::WorldState;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::CollaborationModeMessages;
use pretty_assertions::assert_eq;

#[test]
fn snapshots() {
    use PreviousSectionState::Absent;
    use PreviousSectionState::Known;
    use PreviousSectionState::Unknown;

    let default = collaboration_mode_state(ModeKind::Default, "pair with the user");
    let old_default = collaboration_mode_state(ModeKind::Default, "old instructions");
    let new_default = collaboration_mode_state(ModeKind::Default, "new instructions");
    let plan = collaboration_mode_state(ModeKind::Plan, "make a plan");

    insta::assert_snapshot!(render_section_cases(&[
        (Absent, Absent),
        (Absent, Known(&default)),
        (Known(&default), Known(&default)),
        (Known(&old_default), Known(&new_default)),
        (Known(&default), Known(&plan)),
        (Unknown, Known(&default)),
    ]));
}

#[test]
fn persisted_instructions_are_restored_only_when_missing_from_history() {
    let state = collaboration_mode_state(ModeKind::Default, "pair with the user");
    let retained: ResponseItem = ContextualUserFragment::into(CollaborationModeInstructions {
        instructions: state.instructions.clone().expect("test instructions"),
    });
    let mut world_state = WorldState::default();
    world_state.add_section(state);
    let snapshot = world_state.snapshot();

    assert!(
        world_state
            .render_history_diff(/*previous*/ None, std::slice::from_ref(&retained))
            .is_empty()
    );
    assert_eq!(
        world_state.render_history_diff(Some(&snapshot), &[]).len(),
        1,
    );
    assert!(
        world_state
            .render_history_diff(Some(&snapshot), &[retained])
            .is_empty()
    );
}

#[test]
fn catalog_collaboration_messages_select_mode_variant() {
    let messages = CollaborationModeMessages {
        default: Some("catalog default instructions".to_string()),
        plan: Some("catalog plan instructions".to_string()),
    };

    for (mode, expected) in [
        (ModeKind::Default, "catalog default instructions"),
        (ModeKind::Plan, "catalog plan instructions"),
    ] {
        let state = CollaborationModeState::from_collaboration_mode(
            &collaboration_mode(mode, Some("legacy instructions")),
            Some(&messages),
        );

        assert_eq!(state.instructions.as_deref(), Some(expected));
    }
}

#[test]
fn empty_catalog_collaboration_message_suppresses_legacy_instructions() {
    let messages = CollaborationModeMessages {
        default: None,
        plan: Some(String::new()),
    };
    let state = CollaborationModeState::from_collaboration_mode(
        &collaboration_mode(ModeKind::Plan, Some("legacy plan instructions")),
        Some(&messages),
    );

    assert_eq!(
        state
            .render_diff(PreviousSectionState::Absent)
            .expect("explicit empty collaboration message")
            .render(),
        format!("{COLLABORATION_MODE_OPEN_TAG}{COLLABORATION_MODE_CLOSE_TAG}")
    );
}

#[test]
fn missing_catalog_collaboration_message_uses_legacy_instructions() {
    let messages = CollaborationModeMessages {
        default: Some("catalog default instructions".to_string()),
        plan: None,
    };
    let state = CollaborationModeState::from_collaboration_mode(
        &collaboration_mode(ModeKind::Plan, Some("legacy plan instructions")),
        Some(&messages),
    );

    assert_eq!(
        state.instructions.as_deref(),
        Some("legacy plan instructions")
    );
}

#[test]
fn legacy_collaboration_mode_snapshots_refresh_catalog_messages_once() {
    let previous = serde_json::from_str::<CollaborationModeSnapshot>("\"default\"")
        .expect("legacy collaboration mode snapshot");

    for instructions in ["catalog instructions", ""] {
        let messages = CollaborationModeMessages {
            default: Some(instructions.to_string()),
            plan: None,
        };
        let state = CollaborationModeState::from_collaboration_mode(
            &collaboration_mode(ModeKind::Default, Some("stale legacy instructions")),
            Some(&messages),
        );

        assert_eq!(
            state
                .render_diff(PreviousSectionState::Known(&previous))
                .expect("legacy snapshot should refresh collaboration instructions")
                .render(),
            format!("{COLLABORATION_MODE_OPEN_TAG}{instructions}{COLLABORATION_MODE_CLOSE_TAG}")
        );
        assert!(
            state
                .render_diff(PreviousSectionState::Known(&state.snapshot()))
                .is_none()
        );
    }
}

fn collaboration_mode(mode: ModeKind, instructions: Option<&str>) -> CollaborationMode {
    CollaborationMode {
        mode,
        settings: Settings {
            model: "test-model".to_string(),
            reasoning_effort: None,
            developer_instructions: instructions.map(str::to_string),
        },
    }
}

fn collaboration_mode_state(mode: ModeKind, instructions: &str) -> CollaborationModeState {
    CollaborationModeState::from_collaboration_mode(
        &collaboration_mode(mode, Some(instructions)),
        /*catalog_messages*/ None,
    )
}

/// Every TUI-selectable mode must deliver its own template as the
/// `<collaboration_mode>` developer block. This is the wiring the TUI relies on:
/// preset mask -> CollaborationMode settings -> world-state section -> fragment.
#[test]
fn each_builtin_preset_renders_its_own_template() {
    use codex_models_manager::collaboration_mode_presets::builtin_collaboration_mode_presets;

    let expected_heading = |mode: ModeKind| match mode {
        ModeKind::Plan => "# Plan Mode (Conversational)",
        ModeKind::Default => "# Collaboration Mode: Default",
        ModeKind::Explore => "# Collaboration Mode: Explore",
    };

    let presets = builtin_collaboration_mode_presets();
    assert_eq!(presets.len(), 3, "one preset per TUI-visible mode");

    for mask in presets {
        let mode = mask.mode.expect("preset carries a mode");
        let applied = collaboration_mode(ModeKind::Default, None).apply_mask(&mask);
        assert_eq!(applied.mode, mode, "mask sets the mode");

        let rendered = CollaborationModeState::from_collaboration_mode(
            &applied,
            /*catalog_messages*/ None,
        )
        .render_diff(PreviousSectionState::Absent)
        .expect("mode instructions must be emitted")
        .render();

        assert!(
            rendered.starts_with(COLLABORATION_MODE_OPEN_TAG)
                && rendered.ends_with(COLLABORATION_MODE_CLOSE_TAG),
            "{mode:?} instructions must be wrapped in collaboration_mode tags"
        );
        assert!(
            rendered.contains(expected_heading(mode)),
            "{mode:?} must render its own template, got: {}",
            &rendered[..rendered.len().min(120)]
        );
        for other in [ModeKind::Plan, ModeKind::Default, ModeKind::Explore] {
            if other != mode {
                assert!(
                    !rendered.contains(expected_heading(other)),
                    "{mode:?} must not leak the {other:?} template"
                );
            }
        }
    }
}

/// `CollaborationModeMessages` has no Explore field, so the backend catalog can
/// neither override nor (with an empty string) suppress Explore's instructions
/// the way it can for Default and Plan. Explore is always locally defined.
#[test]
fn explore_ignores_catalog_messages() {
    let messages = CollaborationModeMessages {
        default: Some(String::new()),
        plan: Some(String::new()),
    };
    let state = CollaborationModeState::from_collaboration_mode(
        &collaboration_mode(ModeKind::Explore, Some("local explore instructions")),
        Some(&messages),
    );

    assert_eq!(
        state.instructions.as_deref(),
        Some("local explore instructions")
    );
}
