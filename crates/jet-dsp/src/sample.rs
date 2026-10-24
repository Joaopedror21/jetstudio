//! Decoded sample data shared between threads.

/// Planar 32-bit float audio. One or two channels.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleData {
    pub rate: u32,
    pub left: Vec<f32>,
    /// `None` for mono material.
    pub right: Option<Vec<f32>>,
}

impl SampleData {
    pub fn mono(rate: u32, data: Vec<f32>) -> Self {
        Self {
            rate,
            left: data,
            right: None,
        }
    }

    pub fn stereo(rate: u32, left: Vec<f32>, right: Vec<f32>) -> Self {
        Self {
            rate,
            left,
            right: Some(right),
        }
    }

    pub fn frames(&self) -> usize {
        self.left.len()
    }

    pub fn is_stereo(&self) -> bool {
        self.right.is_some()
    }

    /// 4-point, 3rd-order Hermite interpolation at fractional frame `pos`.
    /// Out-of-range neighbours read as silence.
    #[inline]
    pub fn read(&self, pos: f64) -> (f32, f32) {
        let index = pos.floor();
        let frac = (pos - index) as f32;
        let i = index as i64;
        let left = hermite(&self.left, i, frac);
        let right = match &self.right {
            Some(right) => hermite(right, i, frac),
            None => left,
        };
        (left, right)
    }

    /// Peak absolute value over both channels.
    pub fn peak(&self) -> f32 {
        let peak = |data: &[f32]| data.iter().fold(0.0_f32, |acc, v| acc.max(v.abs()));
        peak(&self.left).max(self.right.as_deref().map_or(0.0, peak))
    }
}

#[inline]
fn at(data: &[f32], i: i64) -> f32 {
    if i < 0 {
        0.0
    } else {
        data.get(i as usize).copied().unwrap_or(0.0)
    }
}

#[inline]
fn hermite(data: &[f32], i: i64, t: f32) -> f32 {
    let xm1 = at(data, i - 1);
    let x0 = at(data, i);
    let x1 = at(data, i + 1);
    let x2 = at(data, i + 2);
    let c = (x1 - xm1) * 0.5;
    let v = x0 - x1;
    let w = c + v;
    let a = w + v + (x2 - x0) * 0.5;
    let b = w + a;
    ((a * t - b) * t + c) * t + x0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_hits_sample_points_and_is_smooth_between() {
        let data = SampleData::mono(48_000, vec![0.0, 1.0, 0.0, -1.0, 0.0]);
        assert_eq!(data.read(1.0), (1.0, 1.0));
        assert_eq!(data.read(3.0).0, -1.0);
        let mid = data.read(1.5).0;
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(data.read(100.0).0, 0.0);
    }
}
