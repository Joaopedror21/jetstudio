//! Painted primitives. Nothing here uses egui's stock widgets: every control is drawn with the
//! `Painter` from measured tokens (see `theme.rs`).

use egui::{
    Color32, Id, Mesh, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

use crate::probe::LayoutProbe;
use crate::theme::{Layout, Tokens};

pub struct PaintCtx<'a> {
    pub ui: &'a mut Ui,
    pub tokens: &'a Tokens,
    pub layout: &'a Layout,
    pub probe: &'a mut LayoutProbe,
    pub hint: &'a mut String,
}

/// Vertical gradient through `stops` (evenly spaced, top to bottom).
pub fn vgradient(painter: &Painter, rect: Rect, stops: &[Color32]) {
    if stops.len() < 2 || !rect.is_positive() {
        if let Some(color) = stops.first() {
            painter.rect_filled(rect, 0.0, *color);
        }
        return;
    }
    let mut mesh = Mesh::default();
    let n = stops.len() - 1;
    for (i, color) in stops.iter().enumerate() {
        let y = rect.top() + rect.height() * i as f32 / n as f32;
        mesh.colored_vertex(pos2(rect.left(), y), *color);
        mesh.colored_vertex(pos2(rect.right(), y), *color);
        if i > 0 {
            let base = (i as u32 - 1) * 2;
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 3, base + 2);
        }
    }
    painter.add(mesh);
}

/// Horizontal gradient through `stops` (evenly spaced, left to right).
pub fn hgradient(painter: &Painter, rect: Rect, stops: &[Color32]) {
    if stops.len() < 2 || !rect.is_positive() {
        return;
    }
    let mut mesh = Mesh::default();
    let n = stops.len() - 1;
    for (i, color) in stops.iter().enumerate() {
        let x = rect.left() + rect.width() * i as f32 / n as f32;
        mesh.colored_vertex(pos2(x, rect.top()), *color);
        mesh.colored_vertex(pos2(x, rect.bottom()), *color);
        if i > 0 {
            let base = (i as u32 - 1) * 2;
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 3, base + 2);
        }
    }
    painter.add(mesh);
}

/// Solid fill with a 1 px inside edge.
pub fn framed(painter: &Painter, rect: Rect, radius: f32, fill: Color32, edge: Color32) {
    painter.rect_filled(rect, radius, fill);
    painter.rect_stroke(rect, radius, Stroke::new(1.0, edge), StrokeKind::Inside);
}

/// One-pixel vertical hatching used by FL scrollbars (W x 1548-1566).
pub fn hatch(painter: &Painter, rect: Rect, a: Color32, b: Color32) {
    painter.rect_filled(rect, 0.0, b);
    let mut x = rect.left().floor();
    while x < rect.right() {
        painter.rect_filled(
            Rect::from_min_max(
                pos2(x, rect.top()),
                pos2((x + 1.0).min(rect.right()), rect.bottom()),
            ),
            0.0,
            a,
        );
        x += 2.0;
    }
}

/// The orange "lit" face of an active toolbar button (W 636-665, y 4-36).
pub fn lit_face(painter: &Painter, rect: Rect, t: &Tokens) {
    painter.rect_filled(rect, 3.0, t.lit_edge);
    let inner = rect.shrink(1.0);
    vgradient(
        painter,
        inner,
        &[t.lit_top, t.lit_mid, t.lit_mid, t.lit_top],
    );
    let glow = Rect::from_center_size(
        inner.center(),
        vec2(inner.width() * 0.55, inner.height() * 0.7),
    );
    hgradient(painter, glow, &[t.lit_mid, t.lit_center, t.lit_mid]);
}

/// Unlit square toolbar button: 1 px outer + 1 px inner edge, vertical face gradient.
pub fn button_face(painter: &Painter, rect: Rect, t: &Tokens, hovered: bool, pressed: bool) {
    painter.rect_filled(rect, 3.0, t.btn_outer);
    painter.rect_filled(rect.shrink(1.0), 2.0, t.btn_inner);
    let face = rect.shrink(2.0);
    let (top, bottom) = if pressed {
        (t.btn_face_bottom, t.btn_face_top)
    } else if hovered {
        // UNMEASURED: hover state not captured; one step lighter.
        (
            crate::theme::shade(t.btn_face_top, 1.12),
            crate::theme::shade(t.btn_face_bottom, 1.12),
        )
    } else {
        (t.btn_face_top, t.btn_face_bottom)
    };
    vgradient(painter, face, &[top, bottom]);
}

impl PaintCtx<'_> {
    pub fn painter(&self) -> Painter {
        self.ui.painter().clone()
    }

    /// Registers an interactive rectangle (probe + hint) and returns its response.
    pub fn hit(
        &mut self,
        id: &str,
        rect: Rect,
        role: &'static str,
        enabled: bool,
        hint: &str,
    ) -> Response {
        let response = self.ui.interact(
            rect,
            Id::new(id),
            if enabled {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        );
        self.probe.insert(id, rect, role, enabled, None);
        if response.hovered() && !hint.is_empty() {
            *self.hint = hint.to_owned();
        }
        response
    }

    /// Square toolbar button with an icon painter; `lit` draws the orange active face.
    pub fn tool_button(
        &mut self,
        id: &str,
        rect: Rect,
        lit: bool,
        enabled: bool,
        hint: &str,
        icon: impl FnOnce(&Painter, Rect, Color32),
    ) -> Response {
        let response = self.hit(id, rect, "button", enabled, hint);
        let painter = self.painter();
        if lit {
            lit_face(&painter, rect, self.tokens);
        } else {
            button_face(
                &painter,
                rect,
                self.tokens,
                enabled && response.hovered(),
                enabled && response.is_pointer_button_down_on(),
            );
        }
        let color = if !enabled {
            self.tokens.disabled_glyph
        } else if lit {
            crate::theme::shade(self.tokens.lit_edge, 0.55)
        } else {
            self.tokens.btn_icon
        };
        icon(&painter, rect, color);
        response
    }

    /// Light LCD field (tempo, pattern selector): W 437-490 x 8-33.
    pub fn lcd(&self, rect: Rect) {
        let painter = self.painter();
        framed(
            &painter,
            rect,
            2.0,
            self.tokens.lcd_bg,
            self.tokens.lcd_edge,
        );
    }

    /// Small channel-rack knob: dark disc with a light tick at `value` (0..1, 7 to 5 o'clock).
    pub fn knob(&self, center: Pos2, radius: f32, value: f32) {
        let painter = self.painter();
        painter.circle_filled(
            center,
            radius + 1.0,
            crate::theme::shade(self.tokens.knob_body, 0.75),
        );
        painter.circle_filled(center, radius, self.tokens.knob_body);
        let angle = (-135.0 + 270.0 * value.clamp(0.0, 1.0)).to_radians();
        let dir = vec2(angle.sin(), -angle.cos());
        painter.line_segment(
            [center + dir * radius * 0.35, center + dir * radius * 0.9],
            Stroke::new(1.6, self.tokens.knob_tick),
        );
    }

    /// Green activity/mute LED (W 745-755 at y 185).
    pub fn led(&self, center: Pos2, radius: f32, on: bool) {
        let painter = self.painter();
        painter.circle_filled(center, radius, self.tokens.led_ring);
        painter.circle_filled(
            center,
            radius - 1.0,
            if on {
                self.tokens.led_dark
            } else {
                self.tokens.led_off
            },
        );
        if on {
            painter.circle_filled(center, radius - 2.0, self.tokens.led_on);
        }
    }

    /// Minimize/restore/close style button used by the main window and child windows.
    pub fn window_button(
        &mut self,
        id: &str,
        rect: Rect,
        glyph: WindowGlyph,
        framed_face: bool,
        hint: &str,
    ) -> Response {
        let response = self.hit(id, rect, "button", true, hint);
        let painter = self.painter();
        if framed_face {
            framed(
                &painter,
                rect,
                2.0,
                self.tokens.win_btn_face,
                self.tokens.win_btn_edge,
            );
        } else if response.hovered() {
            // UNMEASURED: child-window button hover.
            painter.rect_filled(rect, 1.0, crate::theme::shade(self.tokens.title_bg, 1.25));
        }
        let c = rect.center();
        let color = self.tokens.win_btn_glyph;
        let s = Stroke::new(2.0, color);
        match glyph {
            WindowGlyph::Minimize => {
                painter.line_segment(
                    [pos2(c.x - 4.0, c.y + 2.0), pos2(c.x + 4.0, c.y + 2.0)],
                    Stroke::new(3.0, color),
                );
            }
            WindowGlyph::Restore => {
                painter.rect_stroke(
                    Rect::from_center_size(c + vec2(-1.0, 1.0), vec2(9.0, 8.0)),
                    0.0,
                    Stroke::new(1.5, color),
                    StrokeKind::Middle,
                );
                painter.line_segment(
                    [pos2(c.x - 2.0, c.y - 5.0), pos2(c.x + 5.0, c.y - 5.0)],
                    Stroke::new(1.5, color),
                );
                painter.line_segment(
                    [pos2(c.x + 5.0, c.y - 5.0), pos2(c.x + 5.0, c.y + 2.0)],
                    Stroke::new(1.5, color),
                );
            }
            WindowGlyph::Maximize => {
                painter.rect_stroke(
                    Rect::from_center_size(c, vec2(10.0, 9.0)),
                    0.0,
                    Stroke::new(1.5, color),
                    StrokeKind::Middle,
                );
            }
            WindowGlyph::Close => {
                painter.line_segment([pos2(c.x - 4.5, c.y - 4.5), pos2(c.x + 4.5, c.y + 4.5)], s);
                painter.line_segment([pos2(c.x - 4.5, c.y + 4.5), pos2(c.x + 4.5, c.y - 4.5)], s);
            }
        }
        response
    }
}

#[derive(Clone, Copy)]
pub enum WindowGlyph {
    Minimize,
    Restore,
    Maximize,
    Close,
}

/// One row of a painted popup menu.
pub struct MenuItem<'a> {
    pub label: &'a str,
    pub shortcut: &'a str,
    pub enabled: bool,
}

/// Outcome of a popup menu frame.
pub enum MenuResult {
    Open,
    Chosen(usize),
    Dismissed,
}

impl PaintCtx<'_> {
    /// Painted popup menu on the foreground layer, styled like the main menus. Row height and
    /// width are UNMEASURED (no open context menu has been captured yet).
    pub fn popup_menu(&mut self, id: &str, origin: Pos2, items: &[MenuItem<'_>]) -> MenuResult {
        let t = *self.tokens;
        let row_h = 22.0;
        let width = 240.0;
        let screen = self.ui.ctx().content_rect();
        let mut popup = Rect::from_min_size(origin, vec2(width, items.len() as f32 * row_h + 6.0));
        if popup.bottom() > screen.bottom() {
            popup = popup.translate(vec2(0.0, screen.bottom() - popup.bottom()));
        }
        if popup.right() > screen.right() {
            popup = popup.translate(vec2(screen.right() - popup.right(), 0.0));
        }
        let painter = self.ui.painter().clone().with_layer_id(egui::LayerId::new(
            egui::Order::Foreground,
            Id::new(("popup", id)),
        ));
        framed(&painter, popup, 0.0, t.hint_panel, t.btn_outer);
        let mut result = MenuResult::Open;
        for (row, item) in items.iter().enumerate() {
            let rect = Rect::from_min_size(
                pos2(popup.left() + 3.0, popup.top() + 3.0 + row as f32 * row_h),
                vec2(width - 6.0, row_h),
            );
            let response = self.ui.interact(
                rect,
                Id::new((id, "item", row)),
                if item.enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            self.probe.insert(
                format!("{id}.item.{row}"),
                rect,
                "menu-item",
                item.enabled,
                Some(item.label.to_owned()),
            );
            if response.hovered() && item.enabled {
                painter.rect_filled(rect, 0.0, crate::theme::shade(t.menu_panel, 1.15));
            }
            painter.text(
                pos2(rect.left() + 22.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                item.label,
                crate::theme::regular(15.0),
                if item.enabled {
                    t.menu_text
                } else {
                    t.disabled_glyph
                },
            );
            painter.text(
                pos2(rect.right() - 8.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                item.shortcut,
                crate::theme::regular(13.0),
                t.hint_text,
            );
            if response.clicked() {
                result = MenuResult::Chosen(row);
            }
        }
        let outside = self.ui.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|pos| !popup.contains(pos))
        });
        if matches!(result, MenuResult::Open) && outside {
            result = MenuResult::Dismissed;
        }
        result
    }
}
