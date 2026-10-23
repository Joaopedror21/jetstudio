use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use egui::{Align2, FontId, Id, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::widgets::PaintCtx;

#[derive(Default)]
pub struct BrowserState {
    pub roots: Vec<PathBuf>,
    pub expanded: BTreeSet<PathBuf>,
    pub selected: Option<PathBuf>,
    pub scroll_y: f32,
}

impl BrowserState {
    pub fn add_root(&mut self, path: PathBuf) {
        if path.is_dir() && !self.roots.contains(&path) {
            self.expanded.insert(path.clone());
            self.roots.push(path);
            self.roots.sort();
        }
    }

    fn visible_rows(&self) -> Vec<BrowserRow> {
        let mut rows = Vec::new();
        for root in &self.roots {
            self.append_rows(root, 0, &mut rows);
        }
        rows
    }

    fn append_rows(&self, path: &Path, depth: usize, rows: &mut Vec<BrowserRow>) {
        if rows.len() >= 2000 || depth > 16 {
            return;
        }
        let is_dir = path.is_dir();
        rows.push(BrowserRow {
            path: path.to_path_buf(),
            depth,
            is_dir,
        });
        if !is_dir || !self.expanded.contains(path) {
            return;
        }
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        let mut children: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|item| item.path()))
            .collect();
        children.sort_by(|a, b| {
            b.is_dir()
                .cmp(&a.is_dir())
                .then_with(|| a.file_name().cmp(&b.file_name()))
        });
        for child in children {
            self.append_rows(&child, depth + 1, rows);
            if rows.len() >= 2000 {
                break;
            }
        }
    }
}

struct BrowserRow {
    path: PathBuf,
    depth: usize,
    is_dir: bool,
}

pub fn paint_browser(paint: &mut PaintCtx<'_>, state: &mut BrowserState, rect: Rect) {
    paint.panel(rect, false);
    let header = Rect::from_min_size(rect.min, vec2(rect.width(), paint.layout.window_title_h));
    paint.panel(header, true);
    paint.label(header.shrink2(vec2(5.0, 0.0)), "Browser", false);
    paint
        .probe
        .insert("browser.panel", rect, "panel", true, None);

    let add = Rect::from_min_size(
        pos2(rect.right() - 29.0, rect.top() + 3.0),
        vec2(23.0, 21.0),
    );
    if paint
        .button(
            "browser.add-folder",
            add,
            "+",
            false,
            true,
            "Adicionar pasta local ao Browser",
        )
        .clicked()
        && let Some(path) = rfd::FileDialog::new().pick_folder()
    {
        state.add_root(path);
    }

    let content = Rect::from_min_max(
        pos2(rect.left() + 2.0, header.bottom() + 2.0),
        pos2(rect.right() - 2.0, rect.bottom() - 2.0),
    );
    if paint.ui.rect_contains_pointer(content) {
        let delta = paint.ui.input(|i| i.smooth_scroll_delta.y);
        state.scroll_y = (state.scroll_y - delta).max(0.0);
        let dropped = paint.ui.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            state.add_root(file.path().to_path_buf());
        }
    }

    let rows = state.visible_rows();
    let row_h = paint.layout.row_h;
    let max_scroll = ((rows.len() as f32 * row_h) - content.height()).max(0.0);
    state.scroll_y = state.scroll_y.min(max_scroll);
    if rows.is_empty() {
        paint.ui.painter().text(
            pos2(content.left() + 8.0, content.top() + 22.0),
            Align2::LEFT_TOP,
            "Adicione uma pasta local com +",
            FontId::proportional(paint.layout.font_regular),
            paint.tokens.muted,
        );
        return;
    }

    let start = (state.scroll_y / row_h).floor() as usize;
    let end = (((state.scroll_y + content.height()) / row_h).ceil() as usize).min(rows.len());
    let row_painter = paint.ui.painter().with_clip_rect(content);
    for (index, row) in rows.iter().enumerate().take(end).skip(start) {
        let y = content.top() + index as f32 * row_h - state.scroll_y;
        let row_rect = Rect::from_min_size(pos2(content.left(), y), vec2(content.width(), row_h));
        let id = format!("browser.row.{index}");
        let response = paint.ui.interact(row_rect, Id::new(&id), Sense::click());
        let selected = state.selected.as_ref() == Some(&row.path);
        if selected || response.hovered() {
            row_painter.rect_filled(
                row_rect,
                0.0,
                if selected {
                    paint.tokens.active
                } else {
                    paint.tokens.raised
                },
            );
        }
        let name = row
            .path
            .file_name()
            .map(|part| part.to_string_lossy().into_owned())
            .unwrap_or_else(|| row.path.display().to_string());
        let prefix = if row.is_dir {
            if state.expanded.contains(&row.path) {
                "▾"
            } else {
                "▸"
            }
        } else {
            " "
        };
        row_painter.text(
            pos2(
                row_rect.left() + 7.0 + row.depth as f32 * 13.0,
                row_rect.center().y,
            ),
            Align2::LEFT_CENTER,
            format!("{prefix} {name}"),
            FontId::proportional(paint.layout.font_regular),
            paint.tokens.text,
        );
        paint.probe.insert(
            id,
            row_rect,
            if row.is_dir { "folder" } else { "file" },
            true,
            Some(row.path.display().to_string()),
        );
        if response.clicked() {
            state.selected = Some(row.path.clone());
            if row.is_dir && !state.expanded.insert(row.path.clone()) {
                state.expanded.remove(&row.path);
            }
        }
        if response.hovered() {
            *paint.hint = row.path.display().to_string();
        }
    }
    if max_scroll > 0.0 {
        let rail = Rect::from_min_size(
            pos2(content.right() - 5.0, content.top()),
            vec2(4.0, content.height()),
        );
        let thumb_h = (content.height() * content.height() / (rows.len() as f32 * row_h)).max(24.0);
        let thumb_y = content.top() + (content.height() - thumb_h) * (state.scroll_y / max_scroll);
        row_painter.rect_stroke(
            rail,
            2.0,
            Stroke::new(1.0, paint.tokens.border),
            StrokeKind::Inside,
        );
        row_painter.rect_filled(
            Rect::from_min_size(pos2(rail.left(), thumb_y), vec2(rail.width(), thumb_h)),
            2.0,
            paint.tokens.active,
        );
    }
}
