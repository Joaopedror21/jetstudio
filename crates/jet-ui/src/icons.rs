//! Icons drawn in code. They are our own simple geometric glyphs sized and placed like the
//! reference controls; none is traced or extracted from FL Studio artwork.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

fn line(p: &Painter, a: Pos2, b: Pos2, w: f32, c: Color32) {
    p.line_segment([a, b], Stroke::new(w, c));
}

pub fn play(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.add(Shape::convex_polygon(
        vec![
            pos2(m.x - 4.5, m.y - 6.5),
            pos2(m.x + 6.0, m.y),
            pos2(m.x - 4.5, m.y + 6.5),
        ],
        c,
        Stroke::NONE,
    ));
}

pub fn pause(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.rect_filled(
        Rect::from_center_size(m + vec2(-3.0, 0.0), vec2(3.0, 11.0)),
        0.0,
        c,
    );
    p.rect_filled(
        Rect::from_center_size(m + vec2(3.0, 0.0), vec2(3.0, 11.0)),
        0.0,
        c,
    );
}

pub fn stop(p: &Painter, r: Rect, c: Color32) {
    p.rect_filled(Rect::from_center_size(r.center(), vec2(9.0, 9.0)), 1.0, c);
}

pub fn chevron_right(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.add(Shape::convex_polygon(
        vec![
            pos2(m.x - 2.0, m.y - 4.0),
            pos2(m.x + 2.5, m.y),
            pos2(m.x - 2.0, m.y + 4.0),
        ],
        c,
        Stroke::NONE,
    ));
}

pub fn chevron_left_line(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    line(p, pos2(m.x + 2.5, m.y - 5.0), pos2(m.x - 2.5, m.y), 1.6, c);
    line(p, pos2(m.x - 2.5, m.y), pos2(m.x + 2.5, m.y + 5.0), 1.6, c);
}

pub fn chevron_right_line(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    line(p, pos2(m.x - 2.5, m.y - 5.0), pos2(m.x + 2.5, m.y), 1.6, c);
    line(p, pos2(m.x + 2.5, m.y), pos2(m.x - 2.5, m.y + 5.0), 1.6, c);
}

pub fn chevron_up_line(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    line(p, pos2(m.x - 5.0, m.y + 2.5), pos2(m.x, m.y - 2.5), 1.6, c);
    line(p, pos2(m.x, m.y - 2.5), pos2(m.x + 5.0, m.y + 2.5), 1.6, c);
}

pub fn arrow_up(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.add(Shape::convex_polygon(
        vec![
            pos2(m.x, m.y - 5.0),
            pos2(m.x + 4.0, m.y - 1.0),
            pos2(m.x - 4.0, m.y - 1.0),
        ],
        c,
        Stroke::NONE,
    ));
    p.rect_filled(
        Rect::from_min_max(pos2(m.x - 1.2, m.y - 1.0), pos2(m.x + 1.2, m.y + 5.0)),
        0.0,
        c,
    );
}

pub fn undo(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    let pts: Vec<Pos2> = (0..=10)
        .map(|i| {
            let a = std::f32::consts::PI * (1.25 - 1.1 * i as f32 / 10.0);
            pos2(m.x + 4.0 * a.cos(), m.y + 1.0 - 4.0 * a.sin())
        })
        .collect();
    p.add(Shape::line(pts, Stroke::new(1.8, c)));
    p.add(Shape::convex_polygon(
        vec![
            pos2(m.x - 6.5, m.y - 3.0),
            pos2(m.x - 1.5, m.y - 5.5),
            pos2(m.x - 2.5, m.y - 0.5),
        ],
        c,
        Stroke::NONE,
    ));
}

pub fn arrow_right(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.rect_filled(
        Rect::from_min_max(pos2(m.x - 7.0, m.y - 2.5), pos2(m.x + 1.0, m.y + 2.5)),
        0.0,
        c,
    );
    p.add(Shape::convex_polygon(
        vec![
            pos2(m.x + 1.0, m.y - 6.0),
            pos2(m.x + 7.0, m.y),
            pos2(m.x + 1.0, m.y + 6.0),
        ],
        c,
        Stroke::NONE,
    ));
}

pub fn plus(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    line(p, pos2(m.x - 4.5, m.y), pos2(m.x + 4.5, m.y), 2.0, c);
    line(p, pos2(m.x, m.y - 4.5), pos2(m.x, m.y + 4.5), 2.0, c);
}

/// Browser shortcut: folder with branch arrows.
pub fn browser(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.rect_stroke(
        Rect::from_min_size(pos2(m.x - 8.0, m.y - 9.0), vec2(5.0, 4.0)),
        0.0,
        Stroke::new(1.3, c),
        StrokeKind::Middle,
    );
    line(
        p,
        pos2(m.x - 6.0, m.y - 5.0),
        pos2(m.x - 6.0, m.y + 7.0),
        1.3,
        c,
    );
    for dy in [0.0, 6.0] {
        line(
            p,
            pos2(m.x - 6.0, m.y + dy),
            pos2(m.x - 1.0, m.y + dy),
            1.3,
            c,
        );
        chevron_right(
            p,
            Rect::from_center_size(pos2(m.x + 2.0, m.y + dy), vec2(6.0, 8.0)),
            c,
        );
        chevron_right(
            p,
            Rect::from_center_size(pos2(m.x + 6.0, m.y + dy), vec2(6.0, 8.0)),
            c,
        );
    }
}

/// Channel Rack shortcut: two columns of slots.
pub fn channel_rack(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    for col in [-5.5, 4.5] {
        for row in [-7.0, -1.0, 5.0] {
            p.rect_stroke(
                Rect::from_min_size(pos2(m.x + col - 4.0, m.y + row - 2.0), vec2(8.0, 4.0)),
                0.0,
                Stroke::new(1.3, c),
                StrokeKind::Middle,
            );
        }
    }
}

/// Piano roll shortcut: keys with two notes.
pub fn piano_roll(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.rect_filled(
        Rect::from_min_size(pos2(m.x - 8.0, m.y - 8.0), vec2(5.0, 16.0)),
        0.0,
        c,
    );
    p.rect_filled(
        Rect::from_min_size(pos2(m.x - 1.0, m.y - 3.0), vec2(6.0, 3.0)),
        0.0,
        c,
    );
    p.rect_filled(
        Rect::from_min_size(pos2(m.x + 3.0, m.y - 8.0), vec2(5.0, 3.0)),
        0.0,
        c,
    );
    p.rect_filled(
        Rect::from_min_size(pos2(m.x - 1.0, m.y + 3.0), vec2(4.0, 3.0)),
        0.0,
        c,
    );
}

/// Playlist shortcut: arrangement blocks under a marker.
pub fn playlist(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.rect_stroke(
        Rect::from_min_size(pos2(m.x - 9.0, m.y - 5.0), vec2(18.0, 13.0)),
        0.0,
        Stroke::new(1.3, c),
        StrokeKind::Middle,
    );
    p.add(Shape::convex_polygon(
        vec![
            pos2(m.x - 3.0, m.y - 9.0),
            pos2(m.x + 3.0, m.y - 9.0),
            pos2(m.x, m.y - 6.0),
        ],
        c,
        Stroke::NONE,
    ));
    line(p, pos2(m.x - 6.0, m.y), pos2(m.x - 1.0, m.y), 1.6, c);
    line(
        p,
        pos2(m.x + 1.0, m.y + 4.0),
        pos2(m.x + 6.0, m.y + 4.0),
        1.6,
        c,
    );
}

/// Mixer shortcut: two faders.
pub fn mixer(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    for x in [-4.0, 4.0] {
        line(
            p,
            pos2(m.x + x, m.y - 9.0),
            pos2(m.x + x, m.y + 9.0),
            1.3,
            c,
        );
        p.rect_stroke(
            Rect::from_center_size(
                pos2(m.x + x, m.y + if x < 0.0 { -2.0 } else { 3.0 }),
                vec2(6.0, 4.0),
            ),
            0.0,
            Stroke::new(1.3, c),
            StrokeKind::Middle,
        );
    }
}

pub fn mic(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.rect_filled(
        Rect::from_center_size(m + vec2(0.0, -3.0), vec2(6.0, 11.0)),
        3.0,
        c,
    );
    let pts: Vec<Pos2> = (0..=8)
        .map(|i| {
            let a = std::f32::consts::PI * i as f32 / 8.0;
            pos2(m.x + 5.5 * a.cos(), m.y + 1.0 + 4.5 * a.sin())
        })
        .collect();
    p.add(Shape::line(pts, Stroke::new(1.3, c)));
    line(p, pos2(m.x, m.y + 5.5), pos2(m.x, m.y + 9.0), 1.3, c);
    line(
        p,
        pos2(m.x - 3.5, m.y + 9.0),
        pos2(m.x + 3.5, m.y + 9.0),
        1.3,
        c,
    );
}

pub fn magnet(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    let pts: Vec<Pos2> = (0..=10)
        .map(|i| {
            let a = std::f32::consts::PI * i as f32 / 10.0;
            pos2(m.x + 5.0 * a.cos(), m.y - 1.0 - 5.0 * a.sin())
        })
        .collect();
    p.add(Shape::line(pts, Stroke::new(3.0, c)));
    for x in [-5.0, 5.0] {
        line(
            p,
            pos2(m.x + x, m.y - 1.0),
            pos2(m.x + x, m.y + 5.0),
            3.0,
            c,
        );
    }
}

pub fn pencil(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    line(
        p,
        pos2(m.x - 5.0, m.y + 5.0),
        pos2(m.x + 4.0, m.y - 4.0),
        3.5,
        c,
    );
    line(
        p,
        pos2(m.x - 6.5, m.y + 6.5),
        pos2(m.x - 5.0, m.y + 5.0),
        1.5,
        c,
    );
}

pub fn brush(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    line(
        p,
        pos2(m.x - 1.0, m.y + 1.0),
        pos2(m.x + 5.0, m.y - 5.0),
        2.0,
        c,
    );
    p.circle_filled(pos2(m.x - 3.0, m.y + 3.0), 3.0, c);
}

pub fn delete(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.circle_stroke(m, 6.0, Stroke::new(1.6, c));
    line(
        p,
        pos2(m.x - 4.0, m.y + 4.0),
        pos2(m.x + 4.0, m.y - 4.0),
        1.6,
        c,
    );
}

pub fn speaker(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.add(Shape::convex_polygon(
        vec![
            pos2(m.x - 6.0, m.y - 2.5),
            pos2(m.x - 3.0, m.y - 2.5),
            pos2(m.x + 1.0, m.y - 6.0),
            pos2(m.x + 1.0, m.y + 6.0),
            pos2(m.x - 3.0, m.y + 2.5),
            pos2(m.x - 6.0, m.y + 2.5),
        ],
        c,
        Stroke::NONE,
    ));
}

pub fn mute(p: &Painter, r: Rect, c: Color32) {
    speaker(p, r.translate(vec2(-2.0, 0.0)), c);
    let m = r.center();
    line(
        p,
        pos2(m.x + 2.0, m.y - 2.5),
        pos2(m.x + 7.0, m.y + 2.5),
        1.4,
        c,
    );
    line(
        p,
        pos2(m.x + 2.0, m.y + 2.5),
        pos2(m.x + 7.0, m.y - 2.5),
        1.4,
        c,
    );
}

pub fn slip(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    line(p, pos2(m.x - 4.0, m.y), pos2(m.x + 4.0, m.y), 2.0, c);
    p.add(Shape::convex_polygon(
        vec![
            pos2(m.x - 8.0, m.y),
            pos2(m.x - 3.0, m.y - 5.0),
            pos2(m.x - 3.0, m.y + 5.0),
        ],
        c,
        Stroke::NONE,
    ));
    p.add(Shape::convex_polygon(
        vec![
            pos2(m.x + 8.0, m.y),
            pos2(m.x + 3.0, m.y + 5.0),
            pos2(m.x + 3.0, m.y - 5.0),
        ],
        c,
        Stroke::NONE,
    ));
}

pub fn slice(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.add(Shape::convex_polygon(
        vec![
            pos2(m.x - 7.0, m.y - 6.0),
            pos2(m.x + 4.0, m.y - 6.0),
            pos2(m.x + 1.0, m.y + 1.0),
        ],
        c,
        Stroke::NONE,
    ));
    p.rect_filled(
        Rect::from_min_size(pos2(m.x + 1.0, m.y - 2.0), vec2(4.0, 8.0)),
        1.0,
        c,
    );
}

pub fn select(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner = pos2(m.x + 6.0 * dx, m.y + 6.0 * dy);
        line(p, corner, corner + vec2(-3.0 * dx, 0.0), 1.6, c);
        line(p, corner, corner + vec2(0.0, -3.0 * dy), 1.6, c);
    }
}

pub fn zoom(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.circle_stroke(m + vec2(-1.5, -1.5), 4.5, Stroke::new(1.6, c));
    line(p, m + vec2(2.0, 2.0), m + vec2(6.0, 6.0), 2.2, c);
}

pub fn playback(p: &Painter, r: Rect, c: Color32) {
    speaker(p, r.translate(vec2(-3.0, 0.0)), c);
    let m = r.center();
    line(
        p,
        pos2(m.x + 3.0, m.y - 4.0),
        pos2(m.x + 3.0, m.y + 4.0),
        1.6,
        c,
    );
    line(
        p,
        pos2(m.x + 6.0, m.y - 5.0),
        pos2(m.x + 6.0, m.y + 5.0),
        1.6,
        c,
    );
}

/// Speaker with an output arrow, in front of window titles ("Playlist", "Channel rack").
pub fn title_speaker(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.add(Shape::line(
        vec![
            pos2(m.x - 4.0, m.y - 2.5),
            pos2(m.x - 1.0, m.y - 2.5),
            pos2(m.x + 2.0, m.y - 6.0),
            pos2(m.x + 2.0, m.y + 6.0),
            pos2(m.x - 1.0, m.y + 2.5),
            pos2(m.x - 4.0, m.y + 2.5),
            pos2(m.x - 4.0, m.y - 2.5),
        ],
        Stroke::new(1.4, c),
    ));
    chevron_right(
        p,
        Rect::from_center_size(pos2(m.x + 5.5, m.y), vec2(6.0, 9.0)),
        c,
    );
}

/// Piano-keys glyph used for the "notes" source button.
pub fn keys(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.rect_stroke(
        Rect::from_center_size(m, vec2(14.0, 10.0)),
        0.0,
        Stroke::new(1.5, c),
        StrokeKind::Middle,
    );
    for x in [-3.5, 0.0, 3.5] {
        line(
            p,
            pos2(m.x + x, m.y - 5.0),
            pos2(m.x + x, m.y + 1.0),
            1.5,
            c,
        );
    }
    line(
        p,
        pos2(m.x - 7.0, m.y + 5.5),
        pos2(m.x + 7.0, m.y + 5.5),
        1.5,
        c,
    );
}

pub fn waveform(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    for (i, h) in [3.0, 6.0, 9.0, 6.0, 3.0].iter().enumerate() {
        let x = m.x - 6.0 + i as f32 * 3.0;
        line(p, pos2(x, m.y - h / 2.0), pos2(x, m.y + h / 2.0), 1.5, c);
    }
    line(p, pos2(m.x - 9.0, m.y), pos2(m.x + 9.0, m.y), 1.0, c);
}

pub fn automation(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    line(
        p,
        pos2(m.x - 7.0, m.y + 4.0),
        pos2(m.x + 6.0, m.y - 4.0),
        1.4,
        c,
    );
    p.circle_stroke(pos2(m.x - 7.0, m.y + 4.0), 2.0, Stroke::new(1.2, c));
    p.circle_stroke(pos2(m.x + 6.0, m.y - 4.0), 2.0, Stroke::new(1.2, c));
}

pub fn metronome(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    let pts: Vec<Pos2> = (0..=10)
        .map(|i| {
            let a = std::f32::consts::PI * (0.15 + 0.7 * i as f32 / 10.0);
            pos2(m.x + 10.0 * a.cos(), m.y + 6.0 - 8.0 * a.sin())
        })
        .collect();
    p.add(Shape::line(pts, Stroke::new(1.4, c)));
    line(p, pos2(m.x, m.y + 4.0), pos2(m.x + 2.0, m.y - 7.0), 1.6, c);
}

pub fn countdown(p: &Painter, r: Rect, c: Color32, font: egui::FontId) {
    p.text(r.center(), egui::Align2::CENTER_CENTER, "3.2", font, c);
}

pub fn loop_record(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.circle_stroke(m + vec2(3.0, 0.0), 5.0, Stroke::new(1.6, c));
    p.rect_filled(
        Rect::from_min_size(pos2(m.x - 9.0, m.y - 5.0), vec2(2.0, 10.0)),
        0.0,
        c,
    );
    p.rect_filled(
        Rect::from_min_size(pos2(m.x - 9.0, m.y + 3.0), vec2(6.0, 2.0)),
        0.0,
        c,
    );
}

pub fn help(p: &Painter, r: Rect, c: Color32, font: egui::FontId) {
    p.circle_stroke(r.center(), 8.0, Stroke::new(1.5, c));
    p.text(r.center(), egui::Align2::CENTER_CENTER, "?", font, c);
}

pub fn cart(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.add(Shape::line(
        vec![
            pos2(m.x - 8.0, m.y - 6.0),
            pos2(m.x - 5.0, m.y - 6.0),
            pos2(m.x - 3.0, m.y + 3.0),
            pos2(m.x + 6.0, m.y + 3.0),
            pos2(m.x + 8.0, m.y - 3.0),
            pos2(m.x - 4.0, m.y - 3.0),
        ],
        Stroke::new(1.5, c),
    ));
    p.circle_filled(pos2(m.x - 2.0, m.y + 6.0), 1.5, c);
    p.circle_filled(pos2(m.x + 5.0, m.y + 6.0), 1.5, c);
}

pub fn cloud(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    let s = Stroke::new(1.3, c);
    p.circle_stroke(m + vec2(-1.0, -1.0), 3.5, s);
    p.circle_stroke(m + vec2(3.5, 0.5), 2.5, s);
    line(
        p,
        pos2(m.x - 6.0, m.y + 3.0),
        pos2(m.x + 6.0, m.y + 3.0),
        1.3,
        c,
    );
}

pub fn spin(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.add(Shape::convex_polygon(
        vec![
            pos2(m.x - 3.0, m.y - 1.5),
            pos2(m.x + 3.0, m.y - 1.5),
            pos2(m.x, m.y - 5.0),
        ],
        c,
        Stroke::NONE,
    ));
    p.add(Shape::convex_polygon(
        vec![
            pos2(m.x - 3.0, m.y + 1.5),
            pos2(m.x, m.y + 5.0),
            pos2(m.x + 3.0, m.y + 1.5),
        ],
        c,
        Stroke::NONE,
    ));
}

pub fn gear(p: &Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.circle_stroke(m, 5.0, Stroke::new(3.0, c));
    p.circle_filled(m, 1.5, c);
}
