//! Audio I/O helpers for the voice agent example.
//!
//! - [`record_wav`] — captures PCM audio from the default microphone and returns it
//!   encoded as WAV bytes, suitable for sending to the Deepgram ASR API.
//! - [`play_mp3`] — decodes and plays an MP3 byte buffer (e.g. ElevenLabs output)
//!   through the default output device, blocking until playback completes.

use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

// ── Recording ────────────────────────────────────────────────────────────────

/// Capture audio from the default microphone for `duration_secs` seconds and
/// return the result encoded as WAV bytes.
///
/// The WAV is written at the device's native sample rate and channel count so
/// that Deepgram receives an intact audio file without resampling artifacts.
pub fn record_wav(duration_secs: u64) -> Result<Vec<u8>> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("No default input device found — is a microphone connected?")?;

    let config = device
        .default_input_config()
        .context("Failed to get default input config")?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    // Shared buffer that the input stream writes into.
    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let samples_writer = Arc::clone(&samples);

    // Build the input stream — always collect as f32 for simplicity.
    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                if let Ok(mut buf) = samples_writer.lock() {
                    buf.extend_from_slice(data);
                }
            },
            |err| eprintln!("[audio-io] input stream error: {err}"),
            None,
        )
        .context("Failed to build input stream")?;

    stream.play().context("Failed to start input stream")?;
    std::thread::sleep(Duration::from_secs(duration_secs));
    drop(stream); // stops the stream

    // Encode the captured f32 samples as 16-bit PCM WAV.
    let raw_samples = samples
        .lock()
        .expect("sample buffer poisoned")
        .clone();

    encode_wav(&raw_samples, sample_rate, channels)
}

/// Encode a slice of `f32` samples into a WAV byte buffer with 16-bit PCM.
fn encode_wav(samples: &[f32], sample_rate: u32, channels: u16) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer =
            hound::WavWriter::new(&mut cursor, spec).context("Failed to create WAV writer")?;

        for &sample in samples {
            // Clamp to [-1.0, 1.0] then scale to i16.
            let clamped = sample.clamp(-1.0, 1.0);
            let as_i16 = (clamped * i16::MAX as f32) as i16;
            writer
                .write_sample(as_i16)
                .context("Failed to write WAV sample")?;
        }

        writer.finalize().context("Failed to finalize WAV")?;
    }
    Ok(cursor.into_inner())
}

// ── Playback ─────────────────────────────────────────────────────────────────

/// Decode `data` as MP3 (or any format `rodio` supports) and play it through
/// the default output device, blocking until playback is complete.
pub fn play_mp3(data: Vec<u8>) -> Result<()> {
    use rodio::{Decoder, OutputStream, Sink};

    let (_stream, stream_handle) =
        OutputStream::try_default().context("Failed to open audio output device")?;

    let sink = Sink::try_new(&stream_handle).context("Failed to create audio sink")?;

    let cursor = Cursor::new(data);
    let source = Decoder::new(cursor).context("Failed to decode audio — is the data valid MP3?")?;

    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}
