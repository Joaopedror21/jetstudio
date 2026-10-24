#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

//! The render graph shared by live playback and offline bounce: channels → mixer
//! tracks (FX, pan, fader, sends) → Master. Identical input produces identical output
//! in both paths because both drive the same [`Renderer`].

mod plan;
mod render;

pub use plan::{AutoApply, ChannelPlan, Instrument, PlanLane, RenderPlan, TRACK_COUNT, build_plan};
pub use render::{MAX_VOICES, Renderer, TransportSync};

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

/// Largest block processed at once; longer device buffers are split.
pub const MAX_BLOCK: usize = 1024;

/// Samples kept for the oscilloscope (mono sum of the Master output).
pub const SCOPE_LEN: usize = 2048;
/// Frames kept from the analyzer's tapped track.
pub const TAP_LEN: usize = 2048;

/// Peak meters per mixer track, the Master oscilloscope and the voice count, written by the
/// audio thread and read by the UI.
pub struct Meters {
    peaks: Vec<[AtomicU32; 2]>,
    scope: Vec<AtomicU32>,
    scope_pos: AtomicUsize,
    voices: AtomicU32,
    /// Mixer index whose post-fader output the analyzer reads (`usize::MAX` = none).
    tap_track: AtomicUsize,
    tap: Vec<[AtomicU32; 2]>,
    tap_pos: AtomicUsize,
}

impl Default for Meters {
    fn default() -> Self {
        Self {
            peaks: (0..TRACK_COUNT)
                .map(|_| [AtomicU32::new(0), AtomicU32::new(0)])
                .collect(),
            scope: (0..SCOPE_LEN).map(|_| AtomicU32::new(0)).collect(),
            scope_pos: AtomicUsize::new(0),
            voices: AtomicU32::new(0),
            tap_track: AtomicUsize::new(usize::MAX),
            tap: (0..TAP_LEN)
                .map(|_| [AtomicU32::new(0), AtomicU32::new(0)])
                .collect(),
            tap_pos: AtomicUsize::new(0),
        }
    }
}

impl Meters {
    pub fn record_scope(&self, interleaved: &[f32]) {
        let mut pos = self.scope_pos.load(Ordering::Relaxed);
        for frame in interleaved.as_chunks::<2>().0 {
            let mono = (frame[0] + frame[1]) * 0.5;
            if let Some(slot) = self.scope.get(pos % SCOPE_LEN) {
                slot.store(mono.to_bits(), Ordering::Relaxed);
            }
            pos = pos.wrapping_add(1);
        }
        self.scope_pos.store(pos, Ordering::Release);
    }

    /// The most recent `out.len()` scope samples, oldest first.
    pub fn read_scope(&self, out: &mut [f32]) {
        let end = self.scope_pos.load(Ordering::Acquire);
        let n = out.len().min(SCOPE_LEN);
        for (i, value) in out.iter_mut().take(n).enumerate() {
            let index = end.wrapping_sub(n).wrapping_add(i) % SCOPE_LEN;
            *value = f32::from_bits(self.scope[index].load(Ordering::Relaxed));
        }
    }

    /// Choose the mixer track the analyzer reads (`None` stops the tap).
    pub fn set_tap(&self, track: Option<usize>) {
        self.tap_track
            .store(track.unwrap_or(usize::MAX), Ordering::Relaxed);
    }

    pub fn tap_track(&self) -> Option<usize> {
        let track = self.tap_track.load(Ordering::Relaxed);
        (track != usize::MAX).then_some(track)
    }

    /// Append frames of the tapped track (audio thread; no allocation).
    pub fn record_tap(&self, left: &[f32], right: &[f32]) {
        let mut pos = self.tap_pos.load(Ordering::Relaxed);
        for (l, r) in left.iter().zip(right) {
            let slot = &self.tap[pos % TAP_LEN];
            slot[0].store(l.to_bits(), Ordering::Relaxed);
            slot[1].store(r.to_bits(), Ordering::Relaxed);
            pos = pos.wrapping_add(1);
        }
        self.tap_pos.store(pos, Ordering::Release);
    }

    /// The latest `left.len()` tapped frames, oldest first.
    pub fn read_tap(&self, left: &mut [f32], right: &mut [f32]) {
        let end = self.tap_pos.load(Ordering::Acquire);
        let n = left.len().min(right.len()).min(TAP_LEN);
        for i in 0..n {
            let slot = &self.tap[end.wrapping_sub(n).wrapping_add(i) % TAP_LEN];
            left[i] = f32::from_bits(slot[0].load(Ordering::Relaxed));
            right[i] = f32::from_bits(slot[1].load(Ordering::Relaxed));
        }
    }

    pub fn set_voices(&self, voices: u32) {
        self.voices.store(voices, Ordering::Relaxed);
    }

    /// Voices sounding at the end of the last block (the toolbar polyphony readout).
    pub fn voices(&self) -> u32 {
        self.voices.load(Ordering::Relaxed)
    }

    /// Raise the stored peaks; non-negative f32 bit patterns order like their values.
    pub fn record(&self, track: usize, left: &[f32], right: &[f32]) {
        let Some(slot) = self.peaks.get(track) else {
            return;
        };
        let peak = |data: &[f32]| {
            data.iter().fold(
                0.0_f32,
                |acc, v| if v.is_finite() { acc.max(v.abs()) } else { acc },
            )
        };
        slot[0].fetch_max(peak(left).to_bits(), Ordering::Relaxed);
        slot[1].fetch_max(peak(right).to_bits(), Ordering::Relaxed);
    }

    /// Read and reset the peaks of `track` since the last call.
    pub fn take(&self, track: usize) -> (f32, f32) {
        self.peaks.get(track).map_or((0.0, 0.0), |slot| {
            (
                f32::from_bits(slot[0].swap(0, Ordering::Relaxed)),
                f32::from_bits(slot[1].swap(0, Ordering::Relaxed)),
            )
        })
    }
}

/// How the end of an offline render is handled [Manual: fformats_save_export, "Tail"].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tail {
    /// Extend the render until decaying sounds end.
    LeaveRemainder,
    /// Cut at the end of the material.
    Cut,
    /// Mix the decay after the end back into the start.
    Wrap,
}

/// Longest tail rendered after the material ends (our limit).
pub const MAX_TAIL_SECONDS: u32 = 30;
/// Silence threshold that ends a "leave remainder" tail: -100 dBFS held for 100 ms.
const SILENCE: f32 = 1e-5;

/// Render `length_ticks` of the plan starting at tick 0 to interleaved stereo.
pub fn render_offline(plan: RenderPlan, length_ticks: i64, tail: Tail) -> Vec<f32> {
    render_offline_range(plan, 0, length_ticks, tail)
}

/// Render ticks `start..end` (a timeline selection renders "as a single section" [Manual:
/// playlist, "Timeline Selection"]).
pub fn render_offline_range(plan: RenderPlan, start: i64, end: i64, tail: Tail) -> Vec<f32> {
    let length_ticks = (end - start).max(0);
    let rate = plan.rate;
    let frames_per_tick = |ticks: i64| -> usize {
        let numerator = i128::from(ticks.max(0)) * i128::from(rate) * 60_000;
        let denominator = i128::from(plan.tempo_milli) * i128::from(plan.ppq.max(1));
        (numerator / denominator.max(1)) as usize
    };
    let body = frames_per_tick(length_ticks);
    let mut renderer = Renderer::new(
        Box::new(plan),
        Arc::new(AtomicU64::new(0)),
        Arc::new(Meters::default()),
    );
    renderer.stop_events_after(Some(end));
    renderer.sync(TransportSync {
        playing: true,
        anchor_tick: start,
        anchor_frame: 0,
    });
    let mut out = vec![0.0_f32; body * 2];
    renderer.process(&mut out);
    if tail == Tail::Cut {
        return out;
    }
    let max_tail = rate as usize * MAX_TAIL_SECONDS as usize;
    let quiet_needed = rate as usize / 10;
    let mut tail_buf = Vec::new();
    let mut block = vec![0.0_f32; MAX_BLOCK * 2];
    let mut quiet = 0;
    while tail_buf.len() / 2 < max_tail {
        renderer.process(&mut block);
        let loud = block.iter().any(|v| v.abs() > SILENCE);
        tail_buf.extend_from_slice(&block);
        if loud || renderer.active_voices() > 0 {
            quiet = 0;
        } else {
            quiet += MAX_BLOCK;
            if quiet >= quiet_needed {
                break;
            }
        }
    }
    // Trim trailing silence.
    let last = tail_buf
        .as_chunks::<2>()
        .0
        .iter()
        .rposition(|f| f[0].abs() > SILENCE || f[1].abs() > SILENCE)
        .map_or(0, |i| i + 1);
    tail_buf.truncate(last * 2);
    match tail {
        Tail::LeaveRemainder => {
            out.extend_from_slice(&tail_buf);
        }
        Tail::Wrap => {
            for (dst, src) in out.iter_mut().zip(tail_buf.iter()) {
                *dst += *src;
            }
        }
        Tail::Cut => {}
    }
    out
}

#[cfg(test)]
mod tests;
