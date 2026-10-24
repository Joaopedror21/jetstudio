//! Design tokens measured from the reference captures.
//!
//! `[W x,y]` = sampled from the master capture W (`reference/screenshots/Captura de tela
//! 2026-10-23 202602.png`, 1567x1029, FL Studio 2026 Trial on Windows, default theme, new project;
//! SHA-256 3ebdd2e3…). Flat regions use the median of a box, gradients a pixel profile, text the
//! stem colour of glyphs (W uses ClearType; we render greyscale). `[B]` = Channel Rack close-up,
//! scaled to W by 1/1.49 (measured step pitch 23 px in B vs 15.4 px expected in W). `[A]` = the
//! macOS capture scaled to W by 1.79. Geometry is in W pixels: rendering at 1567x1029 and zoom 1.0
//! is directly comparable with W. `UNMEASURED` = no reference evidence. See docs/UI_VISUAL_AUDIT.md.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

#[derive(Clone, Copy)]
pub struct Tokens {
    // Toolbar.
    pub toolbar: Color32,
    pub toolbar_line: Color32,
    pub menu_panel: Color32,
    pub menu_panel_edge: Color32,
    pub menu_text: Color32,
    pub hint_panel: Color32,
    pub hint_text: Color32,
    pub btn_outer: Color32,
    pub btn_inner: Color32,
    pub btn_face_top: Color32,
    pub btn_face_bottom: Color32,
    pub btn_icon: Color32,
    pub lit_edge: Color32,
    pub lit_top: Color32,
    pub lit_mid: Color32,
    pub lit_center: Color32,
    pub transport_face_top: Color32,
    pub transport_face_bottom: Color32,
    pub transport_edge: Color32,
    pub transport_shadow: Color32,
    pub song_off: Color32,
    pub record_ring: Color32,
    pub record_dot: Color32,
    pub lcd_bg: Color32,
    pub lcd_edge: Color32,
    pub lcd_text: Color32,
    pub time_bg: Color32,
    pub time_edge: Color32,
    pub time_text: Color32,
    pub panel_dark: Color32,
    pub panel_darker: Color32,
    pub panel_edge: Color32,
    pub field_dark: Color32,
    pub field_dark_edge: Color32,
    pub field_text: Color32,
    pub win_btn_face: Color32,
    pub win_btn_edge: Color32,
    pub win_btn_glyph: Color32,
    pub disabled_glyph: Color32,
    // Desktop and windows.
    pub desktop: Color32,
    pub desktop_edge: Color32,
    pub desktop_highlight: Color32,
    pub title_bg: Color32,
    pub title_text: Color32,
    pub title_icon: Color32,
    pub hatch_a: Color32,
    pub hatch_b: Color32,
    // Browser.
    pub browser_title: Color32,
    pub browser_title_text: Color32,
    pub browser_tabs: Color32,
    pub browser_tabs_line: Color32,
    pub browser_tab_text: Color32,
    pub browser_tab_selected: Color32,
    pub browser_body: Color32,
    pub browser_text: Color32,
    pub browser_row_hover: Color32,
    pub browser_row_selected: Color32,
    pub splitter: Color32,
    pub splitter_edge: Color32,
    // Playlist.
    pub pl_title: Color32,
    pub pl_strip: Color32,
    pub pl_strip_edge: Color32,
    pub pl_strip_shadow: Color32,
    pub ruler_bg: Color32,
    pub ruler_text: Color32,
    pub ruler_edge: Color32,
    pub grid_a: Color32,
    pub grid_b: Color32,
    pub beat_a: Color32,
    pub beat_b: Color32,
    pub bar_a: Color32,
    pub bar_b: Color32,
    pub row_line: Color32,
    pub row_line_edge: Color32,
    pub track_face: Color32,
    pub track_face_top: Color32,
    pub track_edge: Color32,
    pub track_highlight: Color32,
    pub track_text: Color32,
    pub track_gutter: Color32,
    pub hdr_label: Color32,
    pub led_ring: Color32,
    pub led_dark: Color32,
    pub led_on: Color32,
    pub led_off: Color32,
    pub picker_bg: Color32,
    pub picker_item: Color32,
    pub picker_item_top: Color32,
    pub picker_text: Color32,
    pub picker_marker: Color32,
    pub playhead: Color32,
    pub clip_default: Color32,
    // Channel Rack.
    pub rack_bg: Color32,
    pub knob_body: Color32,
    pub knob_tick: Color32,
    pub mixbox: Color32,
    pub mixbox_text: Color32,
    pub channel_default: Color32,
    pub channel_text: Color32,
    pub select_on: Color32,
    pub select_off: Color32,
    pub step_edge: Color32,
    pub step_off_top: Color32,
    pub step_off: Color32,
    pub step_off_bottom: Color32,
    pub step_alt_top: Color32,
    pub step_alt: Color32,
    pub step_alt_bottom: Color32,
    pub step_on_top: Color32,
    pub step_on: Color32,
    pub step_on_bottom: Color32,
    pub step_alt_on: Color32,
    pub step_notch: Color32,
}

impl Default for Tokens {
    fn default() -> Self {
        Self {
            toolbar: hex(0x596267),               // [W 1250-1450,5-75] sd 0
            toolbar_line: hex(0x424B50),          // [W 1250, y 0/41/81]
            menu_panel: hex(0x5F7374),            // [W 250,5-13]
            menu_panel_edge: hex(0x415556),       // [W 250,4]
            menu_text: hex(0xD3F6E8),             // [W menu glyph stems]
            hint_panel: hex(0x3E464B),            // [W 150-250,48-75]
            hint_text: hex(0xAEB4AF),             // [W "(Trial)" glyphs]
            btn_outer: hex(0x2A3338),             // [W 669,10]
            btn_inner: hex(0x323B40),             // [W 670,10]
            btn_face_top: hex(0x444D52),          // [W row-2 button 306,55]
            btn_face_bottom: hex(0x525B60),       // [W row-2 button 325,55]
            btn_icon: hex(0xD9E1E5),              // [W row-2 icons]
            lit_edge: hex(0xAE5716),              // [W 637,4]
            lit_top: hex(0xF28B3D),               // [W 637,6-9]
            lit_mid: hex(0xFFA153),               // [W 637,16-26]
            lit_center: hex(0xFFD88A),            // [W 648-655,10]
            transport_face_top: hex(0x3E464B),    // [W 345,6-16]
            transport_face_bottom: hex(0x30383D), // [W 345,29-34]
            transport_edge: hex(0x434B50),        // [W 339,10]
            transport_shadow: hex(0x172025),      // [W 345,36]
            song_off: hex(0x3A4247),              // [W 321,20-23]
            record_ring: hex(0x495257),           // [W 420,11-30]
            record_dot: hex(0xF9625A),            // [W 420,17-23]
            lcd_bg: hex(0xC0CFD6),                // [W 439-488,10]
            lcd_edge: hex(0x7E8D94),              // [W 445,8]
            lcd_text: hex(0x22333A),              // [W LCD glyph stems, 1st pct]
            time_bg: hex(0x536566),               // [W 505-515,6-10]
            time_edge: hex(0x3E5051),             // [W 500,10]
            time_text: hex(0xD2EEF0),             // [W clock glyph stems]
            panel_dark: hex(0x3E464B),            // [W 770-850,8-14]
            panel_darker: hex(0x30383D),          // [W 1040-1100,26-30]
            panel_edge: hex(0x2F373C),            // [W 768,10]
            field_dark: hex(0x32373B),            // [W 557-615,55] snap selector
            field_dark_edge: hex(0x2E373C),       // [W 555,55]
            field_text: hex(0xB6BFBE),            // [W "Line" glyphs]
            win_btn_face: hex(0x2B3439),          // [W 1520,13-27]
            win_btn_edge: hex(0x4E575C),          // [W 1520,11]
            win_btn_glyph: hex(0xD3DBDF),         // [W 1509-1513,24]
            disabled_glyph: hex(0x7C868B),        // UNMEASURED: no disabled glyph visible in W
            desktop: hex(0x3F484D),               // [W 1000,84-105] FL desktop/title tone
            desktop_edge: hex(0x2A3338),          // [W 1250,82]
            desktop_highlight: hex(0x475055),     // [W 1250,83]
            title_bg: hex(0x3F484D),              // [W 1000-1400,86-100]
            title_text: hex(0xCED5D8),            // [W playlist title glyphs]
            title_icon: hex(0xD3D8DC),            // [W playlist title-bar icons]
            hatch_a: hex(0x384146),               // [W 1550,300] scrollbar hatching
            hatch_b: hex(0x323A3F),               // [W 1551,300]
            browser_title: hex(0x434C51),         // [W 110-300,88-100]
            browser_title_text: hex(0x829096),    // [W "Browser" glyphs]
            browser_tabs: hex(0x1F282D),          // [W 180-320,110-116]
            browser_tabs_line: hex(0x2C353A),     // [W 300,139-141]
            browser_tab_text: hex(0x8A9396),      // [W "ALL" glyphs, ClearType-neutralised]
            browser_tab_selected: hex(0xDA833A),  // [W "SOUNDS" glyphs and outline]
            browser_body: hex(0x253035),          // [W 300-400,145-150]
            browser_text: hex(0xB9C2C6), // UNMEASURED: W shows online content, not the folder tree
            browser_row_hover: hex(0x2E3B41), // UNMEASURED
            browser_row_selected: hex(0x3C4B52), // UNMEASURED
            splitter: hex(0x434C51),     // [W 492-497,300]
            splitter_edge: hex(0x2D363B), // [W 490,95]
            pl_title: hex(0x3F484D),     // [W 1000,84-105]
            pl_strip: hex(0x4F5D64),     // [W 1200,108-124]
            pl_strip_edge: hex(0x636C71), // [W 1200,107]
            pl_strip_shadow: hex(0x353E43), // [W 1200,106]
            ruler_bg: hex(0x212D34),     // [W 1000,128-144]
            ruler_text: hex(0x82888C),   // [W bar-number glyphs]
            ruler_edge: hex(0x1B272E),   // [W 1200,146]
            grid_a: hex(0x34444E),       // [W 773-977,175] bars 1-4
            grid_b: hex(0x2E3E48),       // [W 979-1129,175] bars 5-8
            beat_a: hex(0x22323C),       // [W 785,175]
            beat_b: hex(0x1C2C36),       // [W 991,175]
            bar_a: hex(0x10202A),        // [W 824,175]
            bar_b: hex(0x0A1A24),        // [W 978,175]
            row_line: hex(0x182832),     // [W 1200,194]
            row_line_edge: hex(0x2A3A44), // [W 1200,193]
            track_face: hex(0x495357),   // [W 700,160-197]
            track_face_top: hex(0x525459), // [W 700,151-158]
            track_edge: hex(0x1D262B),   // [W 660,175]
            track_highlight: hex(0x525B60), // [W 661,175]
            track_text: hex(0x788084),   // [W "Track 1" glyphs]
            track_gutter: hex(0x333F46), // [W 762-770,175]
            hdr_label: hex(0xB8C7AF),    // [W "PAT" glyphs]
            led_ring: hex(0x787F82),     // [W 745,185]
            led_dark: hex(0x438007),     // [W 747,185]
            led_on: hex(0xABE15B),       // [W 749-751,185]
            led_off: hex(0x3B4246),      // UNMEASURED
            picker_bg: hex(0x313C45),    // [W 510-630,300-800]
            picker_item: hex(0x495358),  // [W 560,159-175]
            picker_item_top: hex(0x50595E), // [W 560,151-158]
            picker_text: hex(0xCDD3D6),  // [W "Pattern 1" glyphs]
            picker_marker: hex(0xE64363), // [W 503-505,165]
            playhead: hex(0x8CE06A),     // UNMEASURED: no playhead visible in W
            clip_default: hex(0x6E8C9A), // UNMEASURED: no clip visible in W
            rack_bg: hex(0x4E5C65),      // [B 300,255]
            knob_body: hex(0x394249),    // [B 54-59,277]
            knob_tick: hex(0xA5B0B6),    // [B 77,277]
            mixbox: hex(0x4B565C),       // [B 97-112,277]
            mixbox_text: hex(0x9AA6AC),  // UNMEASURED: B hides the mixer number behind a menu
            channel_default: hex(0x5F6C74), // UNMEASURED: default channel button colour not visible
            channel_text: hex(0xE6EBED), // UNMEASURED
            select_on: hex(0x9ACB4E),    // UNMEASURED: B shows a green-outlined selector (284-290)
            select_off: hex(0x3E484E),   // [B 284-289,277]
            step_edge: hex(0x222B30),    // [B 341,285]
            step_off_top: hex(0x585F67), // [B 341,268]
            step_off: hex(0x4B525A),     // [B 341,270-280]
            step_off_bottom: hex(0x434A52), // [B 341,281]
            step_alt_top: hex(0x795D5B), // [B 411,268]
            step_alt: hex(0x614543),     // [B 411,274-281]
            step_alt_bottom: hex(0x563F3E), // [B 411,282]
            step_on_top: hex(0xAAC6D2),  // [B 316,268]
            step_on: hex(0x93AFBB),      // [B 316,275]
            step_on_bottom: hex(0x7F97A2), // [B 316,282]
            step_alt_on: hex(0xD09C9D),  // [B 588-594,265-275]
            step_notch: hex(0x1B2429),   // [B 341,265]
        }
    }
}

/// Geometry in W pixels (see module docs).
#[derive(Clone, Copy)]
pub struct Layout {
    pub toolbar_h: f32,
    pub browser_w: f32,
    pub splitter_w: f32,
    pub title_h: f32,
    pub snap_distance: f32,
    // Playlist.
    pub picker_w: f32,
    pub picker_scroll_w: f32,
    pub track_header_w: f32,
    pub track_gutter_w: f32,
    pub strip_top: f32,
    pub strip_h: f32,
    pub ruler_top: f32,
    pub ruler_h: f32,
    pub tracks_top: f32,
    pub track_h: f32,
    pub bar_w: f32,
    pub vscroll_w: f32,
    // Channel Rack.
    pub rack_row_h: f32,
    pub step_pitch: f32,
    pub step_w: f32,
    pub step_h: f32,
    // Browser.
    pub browser_row_h: f32,
    // Type.
    pub font_menu: f32,
    pub font_title: f32,
    pub font_small: f32,
    pub font_tab: f32,
    pub font_lcd: f32,
    pub font_clock: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            toolbar_h: 84.0,       // [W toolbar y 0-81, desktop edge 82-83]
            browser_w: 490.0,      // [W browser x 0-489]
            splitter_w: 10.0,      // [W x 490-499]
            title_h: 22.0,         // [W y 84-105]
            snap_distance: 8.0,    // UNMEASURED
            picker_w: 139.0,       // [W x 500-638]
            picker_scroll_w: 21.0, // [W x 639-659]
            track_header_w: 101.0, // [W x 660-760]
            track_gutter_w: 10.0,  // [W x 761-770]
            strip_top: 24.0,       // [W y 108 - window top 84]
            strip_h: 17.0,         // [W y 108-124]
            ruler_top: 44.0,       // [W y 128 - 84]
            ruler_h: 17.0,         // [W y 128-144]
            tracks_top: 64.0,      // [W y 148 - 84]
            track_h: 48.0,         // [W rows 148, 196, 244]
            bar_w: 51.5,           // [W bar lines 772, 824, 875, 927, 978]
            vscroll_w: 20.0,       // [W x 1547-1566]
            rack_row_h: 29.0,      // [B 42.5 px pitch / 1.49; A 16.7 x 1.79 = 29.9]
            step_pitch: 15.5,      // [B 23 / 1.49]
            step_w: 11.5,          // [B 17 / 1.49]
            step_h: 17.5,          // [B 259-285 / 1.49]
            browser_row_h: 22.0,   // UNMEASURED: folder tree not visible in W
            font_menu: 17.0,       // [W cap height 12 px (y 14-25) with Barlow Condensed]
            font_title: 15.5,      // [W "Playlist" cap height 10-11 px]
            font_small: 9.5,       // [W NOTE/CHAN/PAT labels]
            font_tab: 14.0,        // [W tab labels]
            font_lcd: 18.0,        // [W "130" cap height 13 px]
            font_clock: 40.0,      // [W "1:01" digit height 25 px]
        }
    }
}

pub const FONT_MEDIUM: &str = "barlow-medium";
pub const FONT_LIGHT: &str = "barlow-light";

/// Barlow Condensed (SIL OFL 1.1, see ATTRIBUTION.md), selected by measured text metrics.
pub fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        (
            "barlow-regular",
            include_bytes!("../../../assets/fonts/BarlowCondensed-Regular.ttf").as_slice(),
        ),
        (
            FONT_MEDIUM,
            include_bytes!("../../../assets/fonts/BarlowCondensed-Medium.ttf").as_slice(),
        ),
        (
            FONT_LIGHT,
            include_bytes!("../../../assets/fonts/BarlowCondensed-Light.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }
    if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
        list.insert(0, "barlow-regular".to_owned());
    }
    for name in [FONT_MEDIUM, FONT_LIGHT] {
        let mut list = vec![name.to_owned()];
        if let Some(fallback) = fonts.families.get(&FontFamily::Proportional) {
            list.extend(fallback.iter().skip(1).cloned());
        }
        fonts.families.insert(FontFamily::Name(name.into()), list);
    }
    fonts
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FONT_MEDIUM.into()))
}

pub fn light(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FONT_LIGHT.into()))
}

pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

/// `0xRRGGBB` from the model, or `fallback`.
pub fn model_color(value: Option<u32>, fallback: Color32) -> Color32 {
    value.map_or(fallback, hex)
}

/// Scales the RGB channels of `color`.
pub fn shade(color: Color32, factor: f32) -> Color32 {
    let f = |c: u8| (f32::from(c) * factor).clamp(0.0, 255.0) as u8;
    Color32::from_rgb(f(color.r()), f(color.g()), f(color.b()))
}
