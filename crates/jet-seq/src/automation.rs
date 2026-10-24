//! Automation Clip evaluation [Manual: playlist_automationclip].
//!
//! The curve types are the manual's ten names; their exact shapes are not published, so the
//! formulas below are ours (Hypothesis H-AUT-2) and are pure functions of the segment
//! position, which keeps live playback and offline rendering identical.

use jet_model::{AutoPoint, AutoTarget, AutomationId, ClipSource, CurveShape, Project};
use jet_time::Tick;
use std::f64::consts::PI;

/// Repetitions for the periodic shapes, from the tension (2..=16).
fn cycles(tension: f64) -> f64 {
    (2.0 + ((tension + 1.0) * 7.0).round()).clamp(2.0, 16.0)
}

/// Normalised shape value 0..1 at segment position `x` (0..1).
pub fn shape(shape: CurveShape, tension: i16, x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    let tau = f64::from(tension.clamp(-1000, 1000)) / 1000.0;
    let k = 8f64.powf(tau);
    match shape {
        CurveShape::SingleCurve => x.powf(k),
        CurveShape::AltSingleCurve => 1.0 - (1.0 - x).powf(k),
        CurveShape::DoubleCurve => {
            if x < 0.5 {
                0.5 * (2.0 * x).powf(k)
            } else {
                1.0 - 0.5 * (2.0 * (1.0 - x)).powf(k)
            }
        }
        CurveShape::AltDoubleCurve => {
            if x < 0.5 {
                0.5 * (1.0 - (1.0 - 2.0 * x).powf(k))
            } else {
                0.5 + 0.5 * (2.0 * x - 1.0).powf(k)
            }
        }
        CurveShape::Hold => 0.0,
        CurveShape::Stairs => {
            let n = cycles(tau);
            (x * n).floor() / (n - 1.0).max(1.0)
        }
        CurveShape::SmoothStairs => {
            let n = cycles(tau);
            let step = (x * n).floor();
            let frac = x * n - step;
            let eased = frac * frac * (3.0 - 2.0 * frac);
            ((step + eased) / n).min(1.0)
        }
        CurveShape::Pulse => {
            if (x * cycles(tau)).fract() < 0.5 {
                0.0
            } else {
                1.0
            }
        }
        CurveShape::Wave => 0.5 - 0.5 * (2.0 * PI * x * cycles(tau)).cos(),
        CurveShape::HalfSine => 0.5 - 0.5 * (PI * x).cos(),
    }
    .clamp(0.0, 1.0)
}

/// Envelope value 0..1 at `tick` (relative to the clip data start). Before the first point the
/// first value holds; after the last point the last value holds.
pub fn value_at(points: &[AutoPoint], tick: i64) -> Option<f64> {
    let first = points.first()?;
    if tick <= first.at.0 {
        return Some(f64::from(first.value) / 1000.0);
    }
    let index = points.partition_point(|p| p.at.0 <= tick);
    let Some(next) = points.get(index) else {
        return points.last().map(|p| f64::from(p.value) / 1000.0);
    };
    let prev = &points[index - 1];
    let span = (next.at.0 - prev.at.0).max(1) as f64;
    let x = (tick - prev.at.0) as f64 / span;
    let v0 = f64::from(prev.value) / 1000.0;
    let v1 = f64::from(next.value) / 1000.0;
    Some(v0 + (v1 - v0) * shape(next.shape, next.tension, x))
}

/// One placement of an Automation Clip in the arrangement.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoSegment {
    pub start: i64,
    pub end: i64,
    pub offset: i64,
    pub automation: AutomationId,
}

/// Everything that drives one parameter in Song mode.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoLane {
    pub target: AutoTarget,
    /// Sorted by start.
    pub segments: Vec<AutoSegment>,
    pub points: Vec<(AutomationId, Vec<AutoPoint>, jet_model::AutoSettings)>,
    /// Ticks per beat (the clip LFO speed is in cycles per beat).
    pub beat_ticks: i64,
}

impl AutoLane {
    /// Value 0..1 at absolute song `tick`; `None` before the first clip, where the control's own
    /// setting applies. After a clip ends its last value holds (Hypothesis H-AUT-1).
    pub fn value_at(&self, tick: i64) -> Option<f64> {
        let index = self.segments.partition_point(|s| s.start <= tick);
        let segment = self.segments.get(index.checked_sub(1)?)?;
        let (_, points, settings) = self
            .points
            .iter()
            .find(|(id, ..)| *id == segment.automation)?;
        let local = tick.min(segment.end - 1).max(segment.start) - segment.start + segment.offset;
        // "Time - Controls the speed at which the Automation Clip plays back".
        let local = local * i64::from(settings.time_milli) / 1000;
        let mut value = value_at(points, local)?;
        let lfo = settings.lfo;
        if lfo.on {
            let phase = (local as f64 * f64::from(lfo.speed_milli)
                / 1000.0
                / self.beat_ticks.max(1) as f64)
                .rem_euclid(1.0);
            let wave = lfo_wave(lfo, phase) * f64::from(lfo.level) / 1000.0;
            // Hypothesis H-AUT-4: multiplied, the LFO scales the envelope between 0 and itself;
            // added, it offsets the envelope by up to half the range.
            value = if lfo.multiply {
                value * (0.5 + 0.5 * wave)
            } else {
                value + 0.5 * wave
            }
            .clamp(0.0, 1.0);
        }
        // "Min/Max - ... a scaling factor that re-ranges the output".
        let (min, max) = (f64::from(settings.min), f64::from(settings.max));
        Some((min + (max - min) * value) / 1000.0)
    }
}

/// The clip LFO's wave (-1..=1) at `phase` (0..1): the tension morphs square -> sine ->
/// triangle -> narrow pulse; skew warps the phase; the pulse width sets the duty of the square
/// and pulse shapes.
pub fn lfo_wave(lfo: jet_model::AutoLfo, phase: f64) -> f64 {
    let skew = (0.5 * (1.0 + f64::from(lfo.skew) / 1000.0)).clamp(0.001, 0.999);
    let phase = if phase < skew {
        0.5 * phase / skew
    } else {
        0.5 + 0.5 * (phase - skew) / (1.0 - skew)
    };
    let width = f64::from(lfo.pulse_width) / 1000.0;
    // The triangle peaks where the sine does (a quarter cycle in).
    let from_peak = (phase - 0.25).rem_euclid(1.0);
    let shapes = [
        if phase < width { 1.0 } else { -1.0 },
        (phase * std::f64::consts::TAU).sin(),
        1.0 - 4.0 * from_peak.min(1.0 - from_peak),
        if phase < width * 0.25 { 1.0 } else { -1.0 },
    ];
    // Position 0..3 along the four shapes.
    let at = (f64::from(lfo.tension) / 1000.0 + 1.0) * 1.5;
    let i = (at.floor() as usize).min(2);
    let f = at - i as f64;
    shapes[i] * (1.0 - f) + shapes[i + 1] * f
}

/// The id under which a pattern's event data joins the lanes (outside the real ids' range).
pub fn event_id(pattern: jet_model::PatternId) -> AutomationId {
    AutomationId(u64::MAX - pattern.0)
}

/// Automation for what plays from `source`: in Song mode the Automation Clips and the event data
/// of the Pattern Clips; in Pattern mode the pattern's event data ("Event Automation remains active
/// in either Pattern or Song mode" [Manual: recording_automation]).
pub fn lanes_for(project: &Project, source: crate::Source) -> Vec<AutoLane> {
    let mut lanes = match source {
        crate::Source::Song => lanes(project),
        crate::Source::Pattern(_) => Vec::new(),
    };
    let beat_ticks = project.settings.beat_ticks().unwrap_or(96);
    let mut add = |pattern: &jet_model::Pattern, segment: AutoSegment| {
        for lane in &pattern.events {
            let Some(first) = lane.points.first() else {
                continue;
            };
            let points: Vec<AutoPoint> = std::iter::once((Tick(0), first.1))
                .chain(lane.points.iter().copied())
                .map(|(at, value)| AutoPoint {
                    at,
                    value,
                    tension: 0,
                    shape: CurveShape::Hold,
                })
                .collect();
            let id = segment.automation;
            match lanes.iter_mut().find(|l| l.target == lane.target) {
                Some(existing) => {
                    existing.segments.push(segment.clone());
                    if !existing.points.iter().any(|(pid, ..)| *pid == id) {
                        existing
                            .points
                            .push((id, points, jet_model::AutoSettings::default()));
                    }
                }
                None => lanes.push(AutoLane {
                    target: lane.target,
                    segments: vec![segment.clone()],
                    points: vec![(id, points, jet_model::AutoSettings::default())],
                    beat_ticks,
                }),
            }
        }
    };
    match source {
        crate::Source::Pattern(id) => {
            if let Some(pattern) = project.pattern(id) {
                let length = crate::pattern_length(project, pattern);
                add(
                    pattern,
                    AutoSegment {
                        start: 0,
                        end: length,
                        offset: 0,
                        automation: event_id(id),
                    },
                );
            }
        }
        crate::Source::Song => {
            if let Some(arrangement) = project.active() {
                for clip in &arrangement.clips {
                    let ClipSource::Pattern(id) = clip.source else {
                        continue;
                    };
                    let muted = clip.muted
                        || arrangement
                            .tracks
                            .iter()
                            .find(|t| t.id == clip.track)
                            .is_none_or(|t| t.mute);
                    if muted {
                        continue;
                    }
                    if let Some(pattern) = project.pattern(id) {
                        add(
                            pattern,
                            AutoSegment {
                                start: clip.start.0,
                                end: clip.end().0,
                                offset: clip.offset.0,
                                automation: event_id(id),
                            },
                        );
                    }
                }
            }
        }
    }
    for lane in &mut lanes {
        lane.segments.sort_by_key(|s| s.start);
    }
    lanes.sort_by_key(|l| l.target);
    lanes
}

/// Automation lanes for Song mode (Pattern mode plays no Playlist clips).
pub fn lanes(project: &Project) -> Vec<AutoLane> {
    let Some(arrangement) = project.active() else {
        return Vec::new();
    };
    let mut lanes: Vec<AutoLane> = Vec::new();
    for clip in &arrangement.clips {
        let ClipSource::Automation(id) = clip.source else {
            continue;
        };
        let track_muted = arrangement
            .tracks
            .iter()
            .find(|t| t.id == clip.track)
            .is_none_or(|t| t.mute);
        let Some(automation) = project.automation(id) else {
            continue;
        };
        if clip.muted || track_muted {
            continue;
        }
        let segment = AutoSegment {
            start: clip.start.0,
            end: clip.end().0,
            offset: clip.offset.0,
            automation: id,
        };
        match lanes.iter_mut().find(|l| l.target == automation.target) {
            Some(lane) => {
                lane.segments.push(segment);
                if !lane.points.iter().any(|(pid, ..)| *pid == id) {
                    lane.points
                        .push((id, automation.points.clone(), automation.settings));
                }
            }
            None => lanes.push(AutoLane {
                target: automation.target,
                segments: vec![segment],
                points: vec![(id, automation.points.clone(), automation.settings)],
                beat_ticks: project.settings.beat_ticks().unwrap_or(96),
            }),
        }
    }
    for lane in &mut lanes {
        lane.segments.sort_by_key(|s| s.start);
    }
    lanes.sort_by_key(|l| l.target);
    lanes
}

#[cfg(test)]
mod tests {
    use super::*;
    use jet_time::Tick;

    fn point(at: i64, value: u16, shape: CurveShape, tension: i16) -> AutoPoint {
        AutoPoint {
            at: Tick(at),
            value,
            tension,
            shape,
        }
    }

    #[test]
    fn clip_lfo_tension_morphs_square_sine_triangle_and_pulse() {
        let lfo = |tension: i16| jet_model::AutoLfo {
            on: true,
            tension,
            ..jet_model::AutoLfo::default()
        };
        // Square at -1000, sine at -333, triangle at 333 (all peak a quarter cycle in).
        assert_eq!(lfo_wave(lfo(-1000), 0.25), 1.0);
        assert_eq!(lfo_wave(lfo(-1000), 0.75), -1.0);
        assert!((lfo_wave(lfo(-333), 0.25) - 1.0).abs() < 1e-3);
        assert!((lfo_wave(lfo(-333), 0.125) - (0.25 * std::f64::consts::PI).sin()).abs() < 1e-2);
        assert!((lfo_wave(lfo(333), 0.125) - 0.5).abs() < 1e-2);
        // The narrow pulse is high only for an eighth of a cycle at 50% width.
        assert_eq!(lfo_wave(lfo(1000), 0.1), 1.0);
        assert_eq!(lfo_wave(lfo(1000), 0.2), -1.0);
    }

    #[test]
    fn single_curve_is_linear_at_zero_tension_and_bends_with_it() {
        let points = [
            point(0, 0, CurveShape::SingleCurve, 0),
            point(100, 1000, CurveShape::SingleCurve, 0),
        ];
        assert_eq!(value_at(&points, 50), Some(0.5));
        assert_eq!(value_at(&points, -10), Some(0.0));
        assert_eq!(value_at(&points, 500), Some(1.0));
        let bent = [
            point(0, 0, CurveShape::SingleCurve, 0),
            point(100, 1000, CurveShape::SingleCurve, 500),
        ];
        assert!(value_at(&bent, 50).is_some_and(|v| v < 0.5));
    }

    #[test]
    fn every_shape_stays_in_range_and_meets_its_end_points() {
        for shape_kind in [
            CurveShape::SingleCurve,
            CurveShape::DoubleCurve,
            CurveShape::AltSingleCurve,
            CurveShape::AltDoubleCurve,
            CurveShape::Hold,
            CurveShape::Stairs,
            CurveShape::SmoothStairs,
            CurveShape::Pulse,
            CurveShape::Wave,
            CurveShape::HalfSine,
        ] {
            for tension in [-1000, 0, 1000] {
                for i in 0..=100 {
                    let v = shape(shape_kind, tension, f64::from(i) / 100.0);
                    assert!((0.0..=1.0).contains(&v), "{shape_kind:?} {tension} {i}");
                }
            }
            assert_eq!(
                shape(shape_kind, 0, 0.0).min(1.0),
                shape(shape_kind, 0, 0.0)
            );
        }
        assert_eq!(shape(CurveShape::Hold, 0, 0.99), 0.0);
        assert!((shape(CurveShape::DoubleCurve, 300, 0.5) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn lanes_hold_the_last_value_after_a_clip_and_none_before() {
        let mut project = Project::new();
        let channel = project.ids.channel();
        project
            .channels
            .push(jet_model::Channel::sampler(channel, "Kick"));
        let id = project.ids.automation();
        project.automations.push(jet_model::AutomationClip {
            id,
            name: "Kick - Volume".into(),
            target: AutoTarget::ChannelVolume(channel),
            points: vec![
                point(0, 0, CurveShape::SingleCurve, 0),
                point(384, 1000, CurveShape::SingleCurve, 0),
            ],
            initial: 781,
            settings: jet_model::AutoSettings::default(),
        });
        let track = project.ids.track();
        project.arrangements[0]
            .tracks
            .push(jet_model::PlaylistTrack::new(track, "Automation"));
        let clip = project.ids.clip();
        let mut instance = jet_model::ClipInstance::pattern(
            clip,
            track,
            project.patterns[0].id,
            Tick(384),
            Tick(384),
        );
        instance.source = ClipSource::Automation(id);
        project.arrangements[0].clips.push(instance);
        assert_eq!(project.validate(), Ok(()));
        let lanes = lanes(&project);
        assert_eq!(lanes.len(), 1);
        assert_eq!(
            lanes[0].value_at(100),
            None,
            "before the clip the knob applies"
        );
        assert_eq!(lanes[0].value_at(384 + 192), Some(0.5));
        let after = lanes[0].value_at(5000).unwrap_or(0.0);
        assert!(after > 0.99, "the last value holds after the clip: {after}");
    }
}
