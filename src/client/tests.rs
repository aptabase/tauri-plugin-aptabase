use super::*;

#[test]
fn build_session_id_is_deterministic() {
    let id1 = build_session_id("machine-123", "A-US-abc", "2026-02-13");
    let id2 = build_session_id("machine-123", "A-US-abc", "2026-02-13");
    assert_eq!(id1, id2);
}

#[test]
fn build_session_id_changes_with_different_date() {
    let id1 = build_session_id("machine-123", "A-US-abc", "2026-02-13");
    let id2 = build_session_id("machine-123", "A-US-abc", "2026-02-14");
    assert_ne!(id1, id2);
}

#[test]
fn build_session_id_changes_with_different_app_key() {
    let id1 = build_session_id("machine-123", "A-US-abc", "2026-02-13");
    let id2 = build_session_id("machine-123", "A-US-xyz", "2026-02-13");
    assert_ne!(id1, id2);
}

#[test]
fn build_session_id_changes_with_different_machine() {
    let id1 = build_session_id("machine-123", "A-US-abc", "2026-02-13");
    let id2 = build_session_id("machine-456", "A-US-abc", "2026-02-13");
    assert_ne!(id1, id2);
}

#[test]
fn build_session_id_returns_truncated_hex() {
    let id = build_session_id("machine-123", "A-US-abc", "2026-02-13");
    assert_eq!(id.len(), 36);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn create_session_id_is_deterministic_across_calls() {
    let id1 = create_session_id("A-US-abc");
    let id2 = create_session_id("A-US-abc");
    assert_eq!(id1, id2);
}
