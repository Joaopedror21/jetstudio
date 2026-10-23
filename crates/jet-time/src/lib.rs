#![forbid(unsafe_code)]

//! Musical time represented with integer ticks and rational conversions.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Tick(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ppq(pub u32);

impl Ppq {
    pub const DEFAULT: Self = Self(96);

    pub fn new(value: u32) -> Result<Self, TimeError> {
        if value == 0 {
            Err(TimeError::InvalidPpq)
        } else {
            Ok(Self(value))
        }
    }

    pub fn validate(self) -> Result<(), TimeError> {
        Self::new(self.0).map(|_| ())
    }
}

impl Default for Ppq {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Tempo in thousandths of a beat per minute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tempo(pub u32);

impl Tempo {
    pub const MIN: u32 = 10_000;
    pub const MAX: u32 = 522_000;
    pub const DEFAULT: Self = Self(120_000);

    pub fn new(milli_bpm: u32) -> Result<Self, TimeError> {
        if !(Self::MIN..=Self::MAX).contains(&milli_bpm) {
            Err(TimeError::InvalidTempo(milli_bpm))
        } else {
            Ok(Self(milli_bpm))
        }
    }

    pub fn validate(self) -> Result<(), TimeError> {
        Self::new(self.0).map(|_| ())
    }
}

impl Default for Tempo {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeSig {
    pub num: u8,
    pub den: u8,
}

impl TimeSig {
    pub fn new(num: u8, den: u8) -> Result<Self, TimeError> {
        if num == 0 || den == 0 || !den.is_power_of_two() {
            Err(TimeError::InvalidTimeSig)
        } else {
            Ok(Self { num, den })
        }
    }

    pub fn validate(self) -> Result<(), TimeError> {
        Self::new(self.num, self.den).map(|_| ())
    }

    pub fn ticks_per_beat(self, ppq: Ppq) -> Result<u32, TimeError> {
        self.validate()?;
        ppq.validate()?;
        let numerator = u64::from(ppq.0) * 4;
        let denominator = u64::from(self.den);
        if !numerator.is_multiple_of(denominator) {
            return Err(TimeError::ResolutionTooLow);
        }
        u32::try_from(numerator / denominator).map_err(|_| TimeError::Overflow)
    }
}

impl Default for TimeSig {
    fn default() -> Self {
        Self { num: 4, den: 4 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TempoPoint {
    pub at: Tick,
    pub tempo: Tempo,
}

/// Points are ordered by tick. M1 projects use one point at tick zero.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TempoMap {
    pub points: Vec<TempoPoint>,
}

impl TempoMap {
    pub fn new(tempo: Tempo) -> Self {
        Self {
            points: vec![TempoPoint { at: Tick(0), tempo }],
        }
    }

    pub fn insert(&mut self, point: TempoPoint) -> Result<(), TimeError> {
        point.tempo.validate()?;
        if point.at.0 < 0 {
            return Err(TimeError::NegativePosition);
        }
        match self.points.binary_search_by_key(&point.at, |p| p.at) {
            Ok(index) => self.points[index] = point,
            Err(index) => self.points.insert(index, point),
        }
        self.validate()
    }

    pub fn validate(&self) -> Result<(), TimeError> {
        if self.points.first().map(|p| p.at) != Some(Tick(0)) {
            return Err(TimeError::InvalidTempoMap);
        }
        let mut previous = None;
        for point in &self.points {
            point.tempo.validate()?;
            if previous.is_some_and(|at| point.at <= at) {
                return Err(TimeError::InvalidTempoMap);
            }
            previous = Some(point.at);
        }
        Ok(())
    }

    pub fn tempo_at(&self, at: Tick) -> Result<Tempo, TimeError> {
        self.validate()?;
        let index = self
            .points
            .partition_point(|point| point.at <= at)
            .saturating_sub(1);
        self.points
            .get(index)
            .map(|point| point.tempo)
            .ok_or(TimeError::InvalidTempoMap)
    }

    pub fn ticks_to_frames(&self, tick: Tick, ppq: Ppq, rate: u32) -> Result<i64, TimeError> {
        self.validate()?;
        ppq.validate()?;
        if rate == 0 {
            return Err(TimeError::InvalidRate);
        }
        if tick.0 < 0 {
            return ticks_to_frames(tick, ppq, self.points[0].tempo, rate);
        }
        let mut frames: i128 = 0;
        for (index, point) in self.points.iter().enumerate() {
            if tick <= point.at {
                break;
            }
            let next = self.points.get(index + 1).map_or(tick, |p| p.at.min(tick));
            let span = next.0 - point.at.0;
            let numerator = i128::from(span) * i128::from(rate) * 60_000;
            let denominator = i128::from(ppq.0) * i128::from(point.tempo.0);
            frames += round_div(numerator, denominator);
        }
        Ok(saturating_i64(frames))
    }
}

impl Default for TempoMap {
    fn default() -> Self {
        Self::new(Tempo::DEFAULT)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeError {
    InvalidPpq,
    InvalidTempo(u32),
    InvalidTimeSig,
    InvalidRate,
    InvalidTempoMap,
    NegativePosition,
    ResolutionTooLow,
    Overflow,
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPpq => write!(f, "PPQ must be positive"),
            Self::InvalidTempo(value) => {
                write!(f, "tempo {value} is outside 10000..=522000 milli-BPM")
            }
            Self::InvalidTimeSig => write!(
                f,
                "time signature must have a positive numerator and power-of-two denominator"
            ),
            Self::InvalidRate => write!(f, "sample rate must be positive"),
            Self::InvalidTempoMap => {
                write!(f, "tempo map needs ordered points starting at tick zero")
            }
            Self::NegativePosition => write!(f, "position must be nonnegative"),
            Self::ResolutionTooLow => write!(f, "PPQ cannot represent this subdivision"),
            Self::Overflow => write!(f, "time value exceeds supported range"),
        }
    }
}

impl std::error::Error for TimeError {}

/// Round to the nearest integer, with exact half cases away from zero.
fn round_div(numerator: i128, denominator: i128) -> i128 {
    let whole = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder.unsigned_abs().saturating_mul(2) >= denominator as u128 {
        whole + if numerator < 0 { -1 } else { 1 }
    } else {
        whole
    }
}

fn saturating_i64(value: i128) -> i64 {
    if value > i128::from(i64::MAX) {
        i64::MAX
    } else if value < i128::from(i64::MIN) {
        i64::MIN
    } else {
        value as i64
    }
}

pub fn ticks_to_frames(tick: Tick, ppq: Ppq, tempo: Tempo, rate: u32) -> Result<i64, TimeError> {
    ppq.validate()?;
    tempo.validate()?;
    if rate == 0 {
        return Err(TimeError::InvalidRate);
    }
    let numerator = i128::from(tick.0) * i128::from(rate) * 60_000;
    let denominator = i128::from(ppq.0) * i128::from(tempo.0);
    Ok(saturating_i64(round_div(numerator, denominator)))
}

pub fn frames_to_ticks(frames: i64, ppq: Ppq, tempo: Tempo, rate: u32) -> Result<Tick, TimeError> {
    ppq.validate()?;
    tempo.validate()?;
    if rate == 0 {
        return Err(TimeError::InvalidRate);
    }
    let numerator = i128::from(frames) * i128::from(ppq.0) * i128::from(tempo.0);
    let denominator = i128::from(rate) * 60_000;
    Ok(Tick(saturating_i64(round_div(numerator, denominator))))
}

pub fn ticks_to_duration(tick: Tick, ppq: Ppq, tempo: Tempo) -> Result<Duration, TimeError> {
    ppq.validate()?;
    tempo.validate()?;
    if tick.0 < 0 {
        return Err(TimeError::NegativePosition);
    }
    let numerator = i128::from(tick.0) * 60_000_000_000_000;
    let denominator = i128::from(ppq.0) * i128::from(tempo.0);
    let nanos = round_div(numerator, denominator);
    let nanos = u64::try_from(nanos).map_err(|_| TimeError::Overflow)?;
    Ok(Duration::from_nanos(nanos))
}

pub fn duration_to_ticks(duration: Duration, ppq: Ppq, tempo: Tempo) -> Result<Tick, TimeError> {
    ppq.validate()?;
    tempo.validate()?;
    let nanos = i128::try_from(duration.as_nanos()).map_err(|_| TimeError::Overflow)?;
    let numerator = nanos
        .saturating_mul(i128::from(ppq.0))
        .saturating_mul(i128::from(tempo.0));
    Ok(Tick(saturating_i64(round_div(
        numerator,
        60_000_000_000_000,
    ))))
}

pub fn format_bar_step_tick(
    tick: Tick,
    ppq: Ppq,
    time_sig: TimeSig,
    steps_per_beat: u8,
) -> Result<String, TimeError> {
    if tick.0 < 0 {
        return Err(TimeError::NegativePosition);
    }
    if steps_per_beat == 0 {
        return Err(TimeError::ResolutionTooLow);
    }
    let beat = i64::from(time_sig.ticks_per_beat(ppq)?);
    if beat % i64::from(steps_per_beat) != 0 {
        return Err(TimeError::ResolutionTooLow);
    }
    let step = beat / i64::from(steps_per_beat);
    let bar = beat * i64::from(time_sig.num);
    let bar_index = (tick.0 / bar).saturating_add(1);
    let within = tick.0 % bar;
    let step_index = within / step + 1;
    let remaining = within % step;
    Ok(format!("{bar_index}:{step_index:02}:{remaining:02}"))
}

pub fn format_min_sec_centisec(tick: Tick, ppq: Ppq, tempo: Tempo) -> Result<String, TimeError> {
    let elapsed = ticks_to_duration(tick, ppq, tempo)?;
    let centis = elapsed.as_millis() / 10;
    let minutes = centis / 6_000;
    let seconds = centis / 100 % 60;
    Ok(format!("{minutes:02}:{seconds:02}:{:02}", centis % 100))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_beat_is_half_a_second_at_48k() {
        let frames = ticks_to_frames(Tick(96), Ppq::DEFAULT, Tempo::DEFAULT, 48_000);
        assert_eq!(frames, Ok(24_000));
        assert_eq!(
            frames_to_ticks(24_000, Ppq::DEFAULT, Tempo::DEFAULT, 48_000),
            Ok(Tick(96))
        );
        assert_eq!(
            ticks_to_duration(Tick(96), Ppq::DEFAULT, Tempo::DEFAULT),
            Ok(Duration::from_millis(500))
        );
        assert_eq!(
            duration_to_ticks(Duration::from_millis(500), Ppq::DEFAULT, Tempo::DEFAULT),
            Ok(Tick(96))
        );
    }

    #[test]
    fn conversions_do_not_accumulate_rounding_error() {
        for tick in 0..=100_000 {
            let frames = ticks_to_frames(Tick(tick), Ppq::DEFAULT, Tempo::DEFAULT, 48_000);
            assert_eq!(frames, Ok(tick * 250));
        }
    }

    #[test]
    fn a_tempo_change_integrates_both_segments() {
        let mut map = TempoMap::new(Tempo(120_000));
        assert_eq!(
            map.insert(TempoPoint {
                at: Tick(96),
                tempo: Tempo(60_000)
            }),
            Ok(())
        );
        assert_eq!(
            map.ticks_to_frames(Tick(192), Ppq::DEFAULT, 48_000),
            Ok(72_000)
        );
    }

    #[test]
    fn rejects_invalid_inputs_and_handles_extremes() {
        assert_eq!(Ppq::new(0), Err(TimeError::InvalidPpq));
        assert_eq!(Tempo::new(9_999), Err(TimeError::InvalidTempo(9_999)));
        assert_eq!(
            ticks_to_frames(Tick(0), Ppq::DEFAULT, Tempo::DEFAULT, 0),
            Err(TimeError::InvalidRate)
        );
        assert_eq!(
            ticks_to_frames(Tick(i64::MAX), Ppq::DEFAULT, Tempo::DEFAULT, u32::MAX),
            Ok(i64::MAX)
        );
        assert_eq!(
            duration_to_ticks(Duration::MAX, Ppq(u32::MAX), Tempo(Tempo::MAX)),
            Ok(Tick(i64::MAX))
        );
    }
}
