//! Higher-level integration test for the persisted-memory SQLite testing path.

#![cfg(feature = "persistence-sqlite")]

mod support;

use support::persisted_memory::{
    PersistedMemoryFixture, assert_no_cross_session_leakage, assert_persisted_session_exists,
    assert_reloaded_history_contains, assert_reloaded_history_len,
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
    assert_reloaded_history_len(&reloaded_alpha, 2);
    assert_reloaded_history_contains(&reloaded_alpha, 0, "alpha: persisted question");
    assert_reloaded_history_contains(&reloaded_alpha, 1, "alpha: persisted answer");
    assert_no_cross_session_leakage(&reloaded_alpha, "beta:");
}
