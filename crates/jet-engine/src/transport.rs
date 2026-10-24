use jet_time::{Ppq, Tempo, Tick};
use serde::Serialize;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Instant;

pub trait Clock: Send {
    fn now_frames(&self) -> u64;
    fn rate(&self) -> u32;
}

pub struct WallClock {
    start: Instant,
    rate: u32,
}

impl Default for WallClock {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            rate: 48_000,
        }
    }
}

impl Clock for WallClock {
    fn now_frames(&self) -> u64 {
        let nanos = self.start.elapsed().as_nanos();
        (nanos.saturating_mul(u128::from(self.rate)) / 1_000_000_000).min(u128::from(u64::MAX))
            as u64
    }
    fn rate(&self) -> u32 {
        self.rate
    }
}

#[derive(Clone)]
pub struct ManualClock {
    frames: Arc<AtomicU64>,
    rate: u32,
}

impl ManualClock {
    pub fn new(rate: u32) -> Self {
        Self {
            frames: Arc::new(AtomicU64::new(0)),
            rate: rate.max(1),
        }
    }
    pub fn set_frames(&self, frames: u64) {
        self.frames.store(frames, Ordering::Release);
    }
}

impl Clock for ManualClock {
    fn now_frames(&self) -> u64 {
        self.frames.load(Ordering::Acquire)
    }
    fn rate(&self) -> u32 {
        self.rate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PlayMode {
    Pattern,
    Song,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TransportState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TransportSnapshot {
    pub mode: PlayMode,
    pub state: TransportState,
    pub position: Tick,
}

pub struct Transport {
    clock: Box<dyn Clock>,
    mode: PlayMode,
    state: TransportState,
    anchor_tick: Tick,
    anchor_frame: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct TransportCheckpoint {
    mode: PlayMode,
    state: TransportState,
    anchor_tick: Tick,
    anchor_frame: u64,
}

impl Transport {
    pub fn new(clock: Box<dyn Clock>) -> Self {
        Self {
            clock,
            mode: PlayMode::Pattern,
            state: TransportState::Stopped,
            anchor_tick: Tick(0),
            anchor_frame: 0,
        }
    }

    pub(crate) fn checkpoint(&self) -> TransportCheckpoint {
        TransportCheckpoint {
            mode: self.mode,
            state: self.state,
            anchor_tick: self.anchor_tick,
            anchor_frame: self.anchor_frame,
        }
    }

    pub(crate) fn restore(&mut self, checkpoint: TransportCheckpoint) {
        self.mode = checkpoint.mode;
        self.state = checkpoint.state;
        self.anchor_tick = checkpoint.anchor_tick;
        self.anchor_frame = checkpoint.anchor_frame;
    }

    pub fn snapshot(&self, ppq: Ppq, tempo: Tempo) -> TransportSnapshot {
        let mut position = self.anchor_tick;
        if self.state == TransportState::Playing {
            let frames = self.clock.now_frames().saturating_sub(self.anchor_frame);
            let numerator = i128::from(frames)
                .saturating_mul(i128::from(tempo.0))
                .saturating_mul(i128::from(ppq.0));
            let denominator = i128::from(self.clock.rate().max(1)).saturating_mul(60_000);
            let delta =
                (numerator / denominator).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
            position = Tick(position.0.saturating_add(delta));
        }
        TransportSnapshot {
            mode: self.mode,
            state: self.state,
            position,
        }
    }

    pub fn play_stop(&mut self, ppq: Ppq, tempo: Tempo) {
        if self.state == TransportState::Playing {
            self.stop();
        } else {
            // A stopped transport starts where it was put ("You can Left-click to reposition this
            // marker while the song is stopped or playing" [Manual: playlist]); `stop` already
            // returned it to 0.
            self.anchor_frame = self.clock.now_frames();
            self.state = TransportState::Playing;
        }
        let _ = (ppq, tempo);
    }

    pub fn stop(&mut self) {
        self.state = TransportState::Stopped;
        self.anchor_tick = Tick(0); // seek-memory behavior remains a reference observation.
        self.anchor_frame = self.clock.now_frames();
    }

    pub fn pause(&mut self, ppq: Ppq, tempo: Tempo) {
        if self.state == TransportState::Playing {
            self.anchor_tick = self.snapshot(ppq, tempo).position;
            self.state = TransportState::Paused;
        } else if self.state == TransportState::Paused {
            self.anchor_frame = self.clock.now_frames();
            self.state = TransportState::Playing;
        }
    }

    /// Replace the clock (e.g. when an audio device opens), keeping the musical position.
    pub fn set_clock(&mut self, clock: Box<dyn Clock>, ppq: Ppq, tempo: Tempo) {
        let position = self.snapshot(ppq, tempo).position;
        self.clock = clock;
        self.anchor_tick = position;
        self.anchor_frame = self.clock.now_frames();
    }

    /// Unwrapped tick at a device `frame` while playing (the renderer's formula), for stamping
    /// recorded input with the frame it arrived at.
    pub fn tick_at_frame(&self, frame: u64, ppq: Ppq, tempo: Tempo) -> Option<i64> {
        if self.state != TransportState::Playing {
            return None;
        }
        let frames = i128::from(frame) - i128::from(self.anchor_frame);
        let numerator = frames * i128::from(tempo.0) * i128::from(ppq.0);
        let denominator = i128::from(self.clock.rate().max(1)) * 60_000;
        Some(self.anchor_tick.0 + numerator.div_euclid(denominator) as i64)
    }

    pub fn now_frame(&self) -> u64 {
        self.clock.now_frames()
    }

    /// (playing, anchor tick, anchor frame) for mirroring into the renderer.
    pub fn anchor(&self) -> (bool, Tick, u64) {
        (
            self.state == TransportState::Playing,
            self.anchor_tick,
            self.anchor_frame,
        )
    }

    pub fn mode(&self) -> PlayMode {
        self.mode
    }

    pub fn toggle_mode(&mut self) {
        self.mode = if self.mode == PlayMode::Pattern {
            PlayMode::Song
        } else {
            PlayMode::Pattern
        };
    }
    pub fn seek(&mut self, tick: Tick) {
        self.anchor_tick = tick;
        self.anchor_frame = self.clock.now_frames();
    }
}
