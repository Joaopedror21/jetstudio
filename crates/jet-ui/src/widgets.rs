use egui::{Align2, FontId, Id, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2};

use crate::probe::LayoutProbe;
use crate::theme::{Layout, Tokens};

pub struct PaintCtx<'a> {
    pub ui: &'a mut Ui,
    pub tokens: &'a Tokens,
    pub layout: &'a Layout,
    pub probe: &'a mut LayoutProbe,
    pub hint: &'a mut String,
}

impl PaintCtx<'_> {
    pub fn label(&self, rect: Rect, text: &str, muted: bool) {
        self.ui.painter().text(
            pos2(rect.left() + 5.0, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(self.layout.font_regular),
            if muted {
                self.tokens.muted
            } else {
                self.tokens.text
            },
        );
    }

    pub fn button(
        &mut self,
        id: &str,
        rect: Rect,
        text: &str,
        selected: bool,
        enabled: bool,
        hint: &str,
    ) -> Response {
        let response = self.ui.interact(
            rect,
            Id::new(id),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let fill = if !enabled {
            self.tokens.surface
        } else if response.is_pointer_button_down_on() || selected {
            self.tokens.active
        } else if response.hovered() {
            self.tokens.raised
        } else {
            self.tokens.control
        };
        self.ui.painter().rect_filled(rect, 2.0, fill);
        self.ui.painter().rect_stroke(
            rect,
            2.0,
            Stroke::new(1.0, self.tokens.border),
            StrokeKind::Inside,
        );
        self.ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(self.layout.font_regular),
            if enabled {
                self.tokens.text
            } else {
                self.tokens.disabled
            },
        );
        self.probe
            .insert(id, rect, "button", enabled, Some(text.to_owned()));
        if response.hovered() {
            *self.hint = hint.to_owned();
        }
        response
    }

    pub fn panel(&self, rect: Rect, raised: bool) {
        self.ui.painter().rect_filled(
            rect,
            0.0,
            if raised {
                self.tokens.raised
            } else {
                self.tokens.surface
            },
        );
        self.ui.painter().rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, self.tokens.border),
            StrokeKind::Inside,
        );
    }
}
