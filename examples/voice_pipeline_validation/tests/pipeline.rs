//! Integration tests for the voice pipeline validation demo.

use voice_pipeline_validation::{DemoConfig, run_validation_demo};

fn assert_contains_in_order(haystack: &[&str], needles: &[&str]) {
    let mut cursor = 0usize;
    for needle in needles {
        let next = haystack[cursor..]
            .iter()
            .position(|item| item == needle)
            .expect("missing event");
        cursor += next + 1;
    }
}

#[tokio::test]
async fn run_demo_emits_required_stage_markers_in_order() {
    let events = run_validation_demo(DemoConfig::default()).await.unwrap();
    let kinds: Vec<&str> = events.iter().map(|e| e.kind()).collect();

    assert_contains_in_order(
        &kinds,
        &[
            "asr_input_received",
            "transcript_emitted",
            "llm_streaming_started",
            "tts_chunk_streaming_started",
            "first_audio_queued",
            "completed",
        ],
    );
}
