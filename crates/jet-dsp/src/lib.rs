#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

//! Independent DSP building blocks: level curves, the Sampler and Synth voices,
//! filters and effects. Nothing here allocates while processing, blocks or does I/O.

pub mod env;
pub mod filter;
pub mod fx;
pub mod glide;
pub mod ins;
pub mod level;
pub mod sample;
pub mod sampler;
pub mod synth;

pub use level::{
    channel_volume_gain, db_to_gain, gain_to_db, mixer_fader_gain, pan_gains, send_gain,
    velocity_gain,
};
pub use sample::SampleData;

/// Equal temperament ratio for a pitch offset in cents.
pub fn cents_ratio(cents: f64) -> f64 {
    (cents / 1200.0).exp2()
}

/// Frequency of a key where 69 is A at 440 Hz (FL's A5 under its C5 = 60 naming).
pub fn key_hz(key: f64) -> f64 {
    440.0 * ((key - 69.0) / 12.0).exp2()
}
