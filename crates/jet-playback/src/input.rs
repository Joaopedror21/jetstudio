//! Audio input for recording [Manual: recording_audio]. Opened only while recording is armed;
//! the device callback copies frames into a pre-allocated queue the UI thread drains.

use crate::PlaybackError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Ten seconds of stereo at 192 kHz: the UI drains every frame, so this only absorbs stalls.
const QUEUE_SAMPLES: usize = 192_000 * 2 * 10;

pub struct AudioInput {
    _stream: cpal::Stream,
    samples: rtrb::Consumer<f32>,
    pub name: String,
    pub rate: u32,
    /// Samples lost because the queue was full (a stalled UI thread).
    pub dropped: Arc<AtomicU64>,
    /// Frames delivered by the last input callback (its buffer length).
    pub buffer_frames: Arc<AtomicU32>,
}

fn push_frames<T: Copy>(
    data: &[T],
    channels: usize,
    to_f32: impl Fn(T) -> f32,
    queue: &mut rtrb::Producer<f32>,
    dropped: &AtomicU64,
    buffer: &AtomicU32,
) {
    let channels = channels.max(1);
    buffer.store((data.len() / channels) as u32, Ordering::Relaxed);
    for frame in data.chunks_exact(channels) {
        let l = to_f32(frame[0]);
        let r = if channels > 1 { to_f32(frame[1]) } else { l };
        if queue.slots() < 2 {
            dropped.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        let _ = queue.push(l);
        let _ = queue.push(r);
    }
}

impl AudioInput {
    /// Open the default input device with its default configuration.
    pub fn open() -> Result<Self, PlaybackError> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or(PlaybackError::NoDevice)?;
        let name = device
            .description()
            .map(|d| d.name().to_owned())
            .unwrap_or_else(|_| "Audio input".to_owned());
        let supported = device
            .default_input_config()
            .map_err(|e| PlaybackError::Config(e.to_string()))?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.config();
        let channels = usize::from(config.channels);
        let (mut producer, consumer) = rtrb::RingBuffer::new(QUEUE_SAMPLES);
        let dropped = Arc::new(AtomicU64::new(0));
        let buffer = Arc::new(AtomicU32::new(0));
        let (d, b) = (Arc::clone(&dropped), Arc::clone(&buffer));
        let on_error = |_e: cpal::Error| {};
        let map = |e: cpal::Error| PlaybackError::Stream(e.to_string());
        let stream = match format {
            SampleFormat::F32 => device
                .build_input_stream(
                    config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        push_frames(data, channels, |v| v, &mut producer, &d, &b);
                    },
                    on_error,
                    None,
                )
                .map_err(map)?,
            SampleFormat::I16 => device
                .build_input_stream(
                    config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        push_frames(
                            data,
                            channels,
                            |v| f32::from(v) / 32768.0,
                            &mut producer,
                            &d,
                            &b,
                        );
                    },
                    on_error,
                    None,
                )
                .map_err(map)?,
            SampleFormat::I32 => device
                .build_input_stream(
                    config,
                    move |data: &[i32], _: &cpal::InputCallbackInfo| {
                        push_frames(
                            data,
                            channels,
                            |v| (f64::from(v) / 2_147_483_648.0) as f32,
                            &mut producer,
                            &d,
                            &b,
                        );
                    },
                    on_error,
                    None,
                )
                .map_err(map)?,
            other => return Err(PlaybackError::Format(format!("{other:?}"))),
        };
        stream
            .play()
            .map_err(|e| PlaybackError::Stream(e.to_string()))?;
        Ok(Self {
            _stream: stream,
            samples: consumer,
            name,
            rate: config.sample_rate,
            dropped,
            buffer_frames: buffer,
        })
    }

    /// Move everything captured so far (interleaved stereo) into `out`.
    pub fn drain(&mut self, out: &mut Vec<f32>) {
        let available = self.samples.slots();
        if let Ok(chunk) = self.samples.read_chunk(available) {
            let (a, b) = chunk.as_slices();
            out.extend_from_slice(a);
            out.extend_from_slice(b);
            chunk.commit_all();
        }
    }

    /// Throw away what was captured before recording starts.
    pub fn discard(&mut self) {
        let available = self.samples.slots();
        if let Ok(chunk) = self.samples.read_chunk(available) {
            chunk.commit_all();
        }
    }
}
