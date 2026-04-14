//! Higher-level integration test for the persisted-memory SQLite testing path.

#![cfg(feature = "persistence-sqlite")]

mod support;

use support::persisted_memory::{
    PersistedMemoryFixture, assert_artifact_history_contains, assert_artifact_history_len,
    assert_artifact_no_cross_session_leakage, assert_missing_persisted_session,
    assert_persisted_session_exists, build_persisted_memory_artifact,
};

#[tokio::test]
async fn persisted_memory_reload_keeps_expected_history_and_excludes_other_session_content() {
    // This is the first higher-level persisted-memory test path outside module-scoped unit tests.
    let fixture = PersistedMemoryFixture::new("persisted-memory-integration.db");
    let alpha_session_id = fixture.new_session_id();
    let beta_session_id = fixture.new_session_id();
    let writer_store = fixture.open_store().await;

    fixture
        .write_session(
            writer_store.clone(),
            alpha_session_id,
            &[
                ("user", "alpha: persisted question"),
                ("assistant", "alpha: persisted answer"),
            ],
        )
        .await;
    fixture
        .write_session(
            writer_store,
            beta_session_id,
            &[
                ("user", "beta: separate session question"),
                ("assistant", "beta: separate session answer"),
            ],
        )
        .await;

    let reloaded_alpha = fixture
        .reload_session(fixture.open_store().await, alpha_session_id)
        .await;

    assert_persisted_session_exists(&reloaded_alpha);
    let artifact = build_persisted_memory_artifact(alpha_session_id, &reloaded_alpha);
    assert_eq!(artifact.session_id, alpha_session_id);
    assert_artifact_history_len(&artifact, 2);
    assert_artifact_history_contains(&artifact, 0, "alpha: persisted question");
    assert_artifact_history_contains(&artifact, 1, "alpha: persisted answer");
    assert_artifact_no_cross_session_leakage(&artifact, "beta:");
}

#[tokio::test]
async fn persisted_memory_reload_missing_session_returns_not_found() {
    let fixture = PersistedMemoryFixture::new("persisted-memory-missing-session.db");
    let missing_session_id = fixture.new_session_id();

    let result = fixture
        .reload_session_result(fixture.open_store().await, missing_session_id)
        .await;

    assert_missing_persisted_session(result);
}

#[tokio::test]
async fn persisted_memory_reopen_boundary_preserves_history() {
    let fixture = PersistedMemoryFixture::new("persisted-memory-reopen-boundary.db");
    let session_id = fixture.new_session_id();

    let writer_store = fixture.open_store().await;
    fixture
        .write_session(
            writer_store,
            session_id,
            &[
                ("user", "boundary: first user message"),
                ("assistant", "boundary: first assistant reply"),
            ],
        )
        .await;

    // Reopen through a fresh store handle to model a restart boundary.
    let reloaded = fixture
        .reload_session(fixture.open_store().await, session_id)
        .await;

    assert_persisted_session_exists(&reloaded);
    let artifact = build_persisted_memory_artifact(session_id, &reloaded);
    assert_eq!(artifact.session_id, session_id);
    assert_artifact_history_len(&artifact, 2);
    assert_artifact_history_contains(&artifact, 0, "boundary: first user message");
    assert_artifact_history_contains(&artifact, 1, "boundary: first assistant reply");
}
