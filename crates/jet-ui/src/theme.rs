use egui::Color32;

/// Temporary legible values until a versioned reference capture is measured.
#[derive(Clone, Copy)]
pub struct Tokens {
    pub desktop: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub control: Color32,
    pub active: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub disabled: Color32,
    pub grid: Color32,
}

impl Default for Tokens {
    fn default() -> Self {
        // UNMEASURED: neutral placeholders, not derived from the reference.
        Self {
            desktop: Color32::from_gray(22),
            surface: Color32::from_gray(34),
            raised: Color32::from_gray(47),
            control: Color32::from_gray(65),
            active: Color32::from_gray(125),
            border: Color32::from_gray(88),
            text: Color32::from_gray(240),
            muted: Color32::from_gray(190),
            disabled: Color32::from_gray(112),
            grid: Color32::from_gray(57),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Layout {
    pub toolbar_h: f32,
    pub browser_w: f32,
    pub window_title_h: f32,
    pub row_h: f32,
    pub step_w: f32,
    pub snap_distance: f32,
    pub font_small: f32,
    pub font_regular: f32,
    pub font_large: f32,
}

impl Default for Layout {
    fn default() -> Self {
        // UNMEASURED: functional starter geometry, not FL Studio measurements.
        Self {
            toolbar_h: 92.0,
            browser_w: 220.0,
            window_title_h: 28.0,
            row_h: 25.0,
            step_w: 22.0,
            snap_distance: 12.0,
            font_small: 11.0,
            font_regular: 12.0,
            font_large: 18.0,
        }
    }
}
