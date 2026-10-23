use egui::{Color32, Painter, Pos2, Shape, Stroke, pos2};

pub fn play(painter: &Painter, center: Pos2, color: Color32) {
    let points = vec![
        pos2(center.x - 4.0, center.y - 6.0),
        pos2(center.x + 6.0, center.y),
        pos2(center.x - 4.0, center.y + 6.0),
    ];
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

pub fn stop(painter: &Painter, center: Pos2, color: Color32) {
    painter.rect_filled(
        egui::Rect::from_center_size(center, egui::vec2(10.0, 10.0)),
        1.0,
        color,
    );
}

pub fn record(painter: &Painter, center: Pos2, color: Color32) {
    painter.circle_filled(center, 5.0, color);
}
