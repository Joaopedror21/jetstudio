#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

//! Audio device output. The device callback owns a [`Renderer`]; the UI thread talks to
//! it through pre-allocated single-producer/single-consumer queues (ADR-006), and retired
//! plans travel back so they are dropped off the audio thread.

pub mod input;
pub mod midi;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, StreamConfig};
use jet_audio_graph::{Meters, RenderPlan, Renderer, TransportSync};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;
use thiserror::Error;

/// Largest device buffer handled in one callback; bigger buffers are rendered in parts.
const SCRATCH_FRAMES: usize = 8192;
const QUEUE: usize = 64;

#[derive(Debug, Error)]
pub enum PlaybackError {
    #[error("no audio output device is available")]
    NoDevice,
    #[error("audio device configuration failed: {0}")]
    Config(String),
    #[error("could not open the audio stream: {0}")]
    Stream(String),
    #[error("unsupported device sample format {0}")]
    Format(String),
}

/// Frames delivered to the device; the musical clock of the session while playing.
#[derive(Clone)]
pub struct DeviceClock {
    frames: Arc<AtomicU64>,
    rate: u32,
}

impl DeviceClock {
    pub fn new(frames: Arc<AtomicU64>, rate: u32) -> Self {
        Self { frames, rate }
    }
}

impl jet_engine::Clock for DeviceClock {
    fn now_frames(&self) -> u64 {
        self.frames.load(Ordering::Acquire)
    }
    fn rate(&self) -> u32 {
        self.rate
    }
}

/// Callback performance, published with relaxed atomics.
#[derive(Default)]
pub struct Stats {
    /// Last callback time as a fraction of the buffer duration, ×10 000.
    pub load: AtomicU32,
    /// Peak load since last read, ×10 000.
    pub peak_load: AtomicU32,
    /// Callbacks that took longer than their buffer (would be audible dropouts).
    pub overruns: AtomicU64,
    pub callbacks: AtomicU64,
    pub last_buffer_frames: AtomicU32,
    /// Set when the stream reports an error (device removed, etc.).
    pub failed: AtomicBool,
}

impl Stats {
    /// (last, peak) DSP load in percent; the peak resets.
    pub fn take_load(&self) -> (f32, f32) {
        let last = self.load.load(Ordering::Relaxed) as f32 / 100.0;
        let peak = self.peak_load.swap(0, Ordering::Relaxed) as f32 / 100.0;
        (last, peak)
    }
}

enum Message {
    /// A new plan and the transport state it must start with, applied together.
    Plan(Box<RenderPlan>, TransportSync),
    Sync(TransportSync),
    Panic,
    Preview(Arc<jet_dsp::SampleData>),
    StopPreview,
    NoteOn(jet_model::ChannelId, u8, u8),
    NoteOff(jet_model::ChannelId, u8),
}

/// Description of the opened device.
#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub name: String,
    pub rate: u32,
    pub channels: u16,
    pub format: String,
    pub buffer: Option<u32>,
    /// The requested rate/buffer was refused and the device default is in use.
    pub fallback: bool,
}

pub struct AudioOutput {
    _stream: cpal::Stream,
    to_audio: rtrb::Producer<Message>,
    retired: rtrb::Consumer<Box<RenderPlan>>,
    frames: Arc<AtomicU64>,
    meters: Arc<Meters>,
    stats: Arc<Stats>,
    info: DeviceInfo,
    last_sync: Option<TransportSync>,
    /// Producer side of the MIDI queue until a MIDI input takes it.
    midi_sink: Option<rtrb::Producer<midi::LiveNote>>,
}

struct Callback {
    renderer: Renderer,
    inbox: rtrb::Consumer<Message>,
    midi: rtrb::Consumer<midi::LiveNote>,
    outbox: rtrb::Producer<Box<RenderPlan>>,
    waiting: Option<Box<RenderPlan>>,
    scratch: Vec<f32>,
    stats: Arc<Stats>,
    channels: usize,
    rate: u32,
}

impl Callback {
    fn drain(&mut self) {
        // A retired plan that could not be returned blocks further swaps until it can.
        if let Some(plan) = self.waiting.take()
            && let Err(rtrb::PushError::Full(plan)) = self.outbox.push(plan)
        {
            self.waiting = Some(plan);
        }
        while let Ok(note) = self.midi.pop() {
            match note {
                midi::LiveNote::On {
                    channel,
                    key,
                    velocity,
                } => self
                    .renderer
                    .live_note_on(jet_model::ChannelId(channel), key, velocity),
                midi::LiveNote::Off { channel, key } => self
                    .renderer
                    .live_note_off(jet_model::ChannelId(channel), key),
            }
        }
        while self.waiting.is_none() {
            let Ok(message) = self.inbox.pop() else {
                break;
            };
            match message {
                Message::Plan(plan, sync) => {
                    let old = self.renderer.swap_plan(plan);
                    self.renderer.sync(sync);
                    if let Err(rtrb::PushError::Full(old)) = self.outbox.push(old) {
                        self.waiting = Some(old);
                    }
                }
                Message::Sync(sync) => self.renderer.sync(sync),
                Message::Panic => self.renderer.panic(),
                // The UI keeps its own reference, so dropping this Arc never frees audio here.
                Message::Preview(sample) => self.renderer.preview(sample),
                Message::StopPreview => self.renderer.stop_preview(),
                Message::NoteOn(channel, key, velocity) => {
                    self.renderer.live_note_on(channel, key, velocity);
                }
                Message::NoteOff(channel, key) => self.renderer.live_note_off(channel, key),
            }
        }
    }

    fn fill<T: Copy>(&mut self, data: &mut [T], convert: impl Fn(f32) -> T, zero: T) {
        let started = Instant::now();
        self.drain();
        let channels = self.channels.max(1);
        let total = data.len() / channels;
        let mut done = 0;
        while done < total {
            let n = (total - done).min(SCRATCH_FRAMES);
            let scratch = &mut self.scratch[..n * 2];
            self.renderer.process(scratch);
            let out = &mut data[done * channels..(done + n) * channels];
            for (frame, stereo) in out
                .chunks_exact_mut(channels)
                .zip(scratch.as_chunks::<2>().0.iter())
            {
                for (c, sample) in frame.iter_mut().enumerate() {
                    *sample = match c {
                        // Hard clip at 0 dBFS on the device path only; renders keep float overs.
                        0 => convert(stereo[0].clamp(-1.0, 1.0)),
                        1 => convert(stereo[1].clamp(-1.0, 1.0)),
                        _ => zero,
                    };
                }
                if channels == 1 {
                    frame[0] = convert(((stereo[0] + stereo[1]) * 0.5).clamp(-1.0, 1.0));
                }
            }
            done += n;
        }
        let elapsed = started.elapsed().as_secs_f64();
        let budget = total as f64 / f64::from(self.rate.max(1));
        let load = if budget > 0.0 {
            (elapsed / budget * 10_000.0) as u32
        } else {
            0
        };
        self.stats.load.store(load, Ordering::Relaxed);
        self.stats.peak_load.fetch_max(load, Ordering::Relaxed);
        if load > 10_000 {
            self.stats.overruns.fetch_add(1, Ordering::Relaxed);
        }
        self.stats.callbacks.fetch_add(1, Ordering::Relaxed);
        self.stats
            .last_buffer_frames
            .store(total as u32, Ordering::Relaxed);
    }
}

/// Requested device settings [Manual: envsettings_audio, "Device", "Sample Rate",
/// "Buffer Length"]. `None` fields use the device defaults.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputRequest {
    /// Output device name as listed by [`output_devices`].
    pub device: Option<String>,
    pub rate: Option<u32>,
    pub buffer_frames: Option<u32>,
}

fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|d| d.name().to_owned())
        .unwrap_or_else(|_| "Audio output".to_owned())
}

/// Names of the output devices of the default host (WASAPI on Windows).
pub fn output_devices() -> Vec<String> {
    cpal::default_host()
        .output_devices()
        .map(|devices| devices.map(|d| device_name(&d)).collect())
        .unwrap_or_default()
}

impl AudioOutput {
    /// Open an output device. If the requested rate or buffer is refused (shared-mode devices
    /// only run at their own rate), the device's default configuration is used and
    /// [`DeviceInfo::fallback`] says so. `initial` builds the first plan for a sample rate.
    pub fn open(
        request: OutputRequest,
        initial: impl Fn(u32) -> RenderPlan,
    ) -> Result<Self, PlaybackError> {
        let host = cpal::default_host();
        let device = match &request.device {
            Some(wanted) => host
                .output_devices()
                .ok()
                .and_then(|mut devices| devices.find(|d| device_name(d) == *wanted))
                .or_else(|| host.default_output_device()),
            None => host.default_output_device(),
        }
        .ok_or(PlaybackError::NoDevice)?;
        let supported = device
            .default_output_config()
            .map_err(|e| PlaybackError::Config(e.to_string()))?;
        let format = supported.sample_format();
        let default_config: StreamConfig = supported.config();
        let mut config = default_config;
        if let Some(rate) = request.rate {
            config.sample_rate = rate;
        }
        if let Some(frames) = request.buffer_frames {
            config.buffer_size = BufferSize::Fixed(frames);
        }
        match Self::start(&device, config, format, &initial) {
            Ok(output) => Ok(output),
            Err(_) if config != default_config => {
                let mut output = Self::start(&device, default_config, format, &initial)?;
                output.info.fallback = true;
                Ok(output)
            }
            Err(error) => Err(error),
        }
    }

    fn start(
        device: &cpal::Device,
        config: StreamConfig,
        format: SampleFormat,
        initial: &dyn Fn(u32) -> RenderPlan,
    ) -> Result<Self, PlaybackError> {
        let rate = config.sample_rate;
        let channels = config.channels;
        let frames = Arc::new(AtomicU64::new(0));
        let meters = Arc::new(Meters::default());
        let stats = Arc::new(Stats::default());
        let (to_audio, inbox) = rtrb::RingBuffer::new(QUEUE);
        let (outbox, retired) = rtrb::RingBuffer::new(QUEUE);
        let (midi_sink, midi_source) = rtrb::RingBuffer::new(256);
        let callback = Callback {
            renderer: Renderer::new(
                Box::new(initial(rate)),
                Arc::clone(&frames),
                Arc::clone(&meters),
            ),
            inbox,
            midi: midi_source,
            outbox,
            waiting: None,
            scratch: vec![0.0; SCRATCH_FRAMES * 2],
            stats: Arc::clone(&stats),
            channels: usize::from(channels),
            rate,
        };
        let error_stats = Arc::clone(&stats);
        let on_error = move |_error: cpal::Error| {
            error_stats.failed.store(true, Ordering::Relaxed);
        };
        let stream = build(device, config, format, callback, on_error)?;
        stream
            .play()
            .map_err(|e| PlaybackError::Stream(e.to_string()))?;
        Ok(Self {
            _stream: stream,
            to_audio,
            retired,
            frames,
            meters,
            stats,
            info: DeviceInfo {
                name: device_name(device),
                rate,
                channels,
                format: format!("{format:?}"),
                buffer: match config.buffer_size {
                    BufferSize::Fixed(frames) => Some(frames),
                    BufferSize::Default => None,
                },
                fallback: false,
            },
            last_sync: None,
            midi_sink: Some(midi_sink),
        })
    }

    /// Open every MIDI input port, playing notes straight into this output.
    pub fn open_midi_inputs(&mut self) -> Result<midi::MidiInputs, String> {
        let sink = self
            .midi_sink
            .take()
            .ok_or_else(|| "MIDI inputs are already open".to_owned())?;
        midi::MidiInputs::connect_all(sink, Arc::clone(&self.frames))
    }

    pub fn clock(&self) -> DeviceClock {
        DeviceClock::new(Arc::clone(&self.frames), self.info.rate)
    }

    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    pub fn meters(&self) -> &Arc<Meters> {
        &self.meters
    }

    pub fn stats(&self) -> &Arc<Stats> {
        &self.stats
    }

    pub fn rate(&self) -> u32 {
        self.info.rate
    }

    /// Queue a plan with the transport state to apply alongside it; returns the plan if the
    /// queue is full so the caller can retry.
    pub fn send_plan(
        &mut self,
        plan: Box<RenderPlan>,
        sync: TransportSync,
    ) -> Result<(), Box<RenderPlan>> {
        match self.to_audio.push(Message::Plan(plan, sync)) {
            Ok(()) => {
                self.last_sync = Some(sync);
                Ok(())
            }
            Err(rtrb::PushError::Full(Message::Plan(plan, _))) => Err(plan),
            Err(rtrb::PushError::Full(_)) => Ok(()),
        }
    }

    /// Audition `sample`; the caller keeps a reference alive until it is replaced.
    pub fn preview(&mut self, sample: Arc<jet_dsp::SampleData>) {
        let _ = self.to_audio.push(Message::Preview(sample));
    }

    pub fn stop_preview(&mut self) {
        let _ = self.to_audio.push(Message::StopPreview);
    }

    /// Live note for the preview keyboard and controllers.
    pub fn note_on(&mut self, channel: jet_model::ChannelId, key: u8, velocity: u8) {
        let _ = self.to_audio.push(Message::NoteOn(channel, key, velocity));
    }

    pub fn note_off(&mut self, channel: jet_model::ChannelId, key: u8) {
        let _ = self.to_audio.push(Message::NoteOff(channel, key));
    }

    /// Mirror the engine transport; repeated identical syncs are not sent.
    pub fn sync(&mut self, sync: TransportSync) {
        if self.last_sync == Some(sync) {
            return;
        }
        if self.to_audio.push(Message::Sync(sync)).is_ok() {
            self.last_sync = Some(sync);
        }
    }

    pub fn panic(&mut self) {
        let _ = self.to_audio.push(Message::Panic);
    }

    /// Drop plans the audio thread has retired. Call regularly from the UI thread.
    pub fn collect(&mut self) {
        while let Ok(plan) = self.retired.pop() {
            drop(plan);
        }
    }
}

fn build(
    device: &cpal::Device,
    config: StreamConfig,
    format: SampleFormat,
    mut callback: Callback,
    on_error: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<cpal::Stream, PlaybackError> {
    let map = |e: cpal::Error| PlaybackError::Stream(e.to_string());
    match format {
        SampleFormat::F32 => device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    callback.fill(data, |v| v, 0.0);
                },
                on_error,
                None,
            )
            .map_err(map),
        SampleFormat::I16 => device
            .build_output_stream(
                config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    callback.fill(data, |v| (v * 32767.0) as i16, 0);
                },
                on_error,
                None,
            )
            .map_err(map),
        SampleFormat::I32 => device
            .build_output_stream(
                config,
                move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                    callback.fill(data, |v| (f64::from(v) * 2_147_483_647.0) as i32, 0);
                },
                on_error,
                None,
            )
            .map_err(map),
        SampleFormat::U16 => device
            .build_output_stream(
                config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    callback.fill(data, |v| ((v + 1.0) * 32767.5) as u16, 32768);
                },
                on_error,
                None,
            )
            .map_err(map),
        other => Err(PlaybackError::Format(format!("{other:?}"))),
    }
}
