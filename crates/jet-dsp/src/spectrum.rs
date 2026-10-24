//! Magnitude spectrum for the analyzer: Hann window and an iterative radix-2 Cooley–Tukey FFT
//! (textbook algorithm; J. W. Cooley and J. W. Tukey, 1965).

/// In-place complex FFT of `re`/`im` (length a power of two).
pub fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    // Bit-reversal permutation.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -std::f64::consts::TAU / len as f64;
        let (w_im, w_re) = angle.sin_cos();
        for start in (0..n).step_by(len) {
            let (mut cur_re, mut cur_im) = (1.0, 0.0);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let t_re = re[b] * cur_re - im[b] * cur_im;
                let t_im = re[b] * cur_im + im[b] * cur_re;
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                let next = cur_re * w_re - cur_im * w_im;
                cur_im = cur_re * w_im + cur_im * w_re;
                cur_re = next;
            }
        }
        len <<= 1;
    }
}

/// Levels in dBFS of bins `0..n/2` of the Hann-windowed `samples` (length a power of two); a
/// full-scale sine reads about 0 dB at its bin.
pub fn spectrum_db(samples: &[f32]) -> Vec<f32> {
    let n = samples.len();
    if n < 2 || !n.is_power_of_two() {
        return Vec::new();
    }
    let mut re: Vec<f64> = samples
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
            f64::from(*v) * w
        })
        .collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im);
    // The Hann window's coherent gain is 0.5; a real sine splits into two bins.
    let scale = 4.0 / n as f64;
    (0..n / 2)
        .map(|k| {
            let magnitude = (re[k] * re[k] + im[k] * im[k]).sqrt() * scale;
            (20.0 * magnitude.max(1e-9).log10()) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_scale_sine_peaks_at_its_bin_near_zero_db() {
        let n = 1024;
        let bin = 64;
        let samples: Vec<f32> = (0..n)
            .map(|i| (std::f64::consts::TAU * bin as f64 * i as f64 / n as f64).sin() as f32)
            .collect();
        let spectrum = spectrum_db(&samples);
        let peak = spectrum
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(peak, bin);
        assert!(spectrum[bin].abs() < 0.1, "{}", spectrum[bin]);
        assert!(spectrum[bin + 10] < -60.0, "far bins are quiet");
    }
}
