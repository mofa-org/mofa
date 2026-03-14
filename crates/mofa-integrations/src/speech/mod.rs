//! Speech adapters root module.

#[cfg(feature = "openai-speech")]
pub mod openai;

#[cfg(feature = "elevenlabs")]
pub mod elevenlabs;

#[cfg(feature = "deepgram")]
pub mod deepgram;
