use super::repair::{map_member_agent_lookup_error, runtime_target_node_hint_is_present};
use super::types::TeamRuntimeStartError;
use crate::path_utils::expand_tilde;
use core::assert_matches;
use sqlx::Error as SqlxError;

#[test]
fn expand_tilde_uses_path_join_for_home_relative_paths() {
    let home = std::env::var("HOME").expect("HOME");
    assert_eq!(
        expand_tilde("~/worktrees"),
        std::path::Path::new(&home)
            .join("worktrees")
            .to_string_lossy()
            .to_string()
    );
}

#[test]
fn runtime_target_node_hint_presence_distinguishes_empty_and_main() {
    assert!(!runtime_target_node_hint_is_present(None));
    assert!(!runtime_target_node_hint_is_present(Some("  ")));
    assert!(runtime_target_node_hint_is_present(Some("main")));
    assert!(runtime_target_node_hint_is_present(Some("node-east")));
}

#[test]
fn member_agent_lookup_maps_row_not_found_to_missing_member_agent() {
    let err = map_member_agent_lookup_error("worker-1", SqlxError::RowNotFound.into());
    let typed = err
        .downcast_ref::<TeamRuntimeStartError>()
        .expect("typed runtime error");
    assert_matches!(typed, TeamRuntimeStartError::MissingMemberAgent(_));
}

#[test]
fn member_agent_lookup_keeps_non_not_found_errors_internal() {
    let err = map_member_agent_lookup_error("worker-1", anyhow::anyhow!("db offline"));
    assert!(err.chain().any(|cause| {
        cause
            .to_string()
            .contains("load team member agent 'worker-1'")
    }));
    assert!(
        err.chain()
            .any(|cause| cause.to_string().contains("db offline"))
    );
}
