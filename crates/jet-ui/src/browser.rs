use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use egui::{Align2, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::icons;
use crate::theme;
use crate::widgets::PaintCtx;

/// Browser tabs as they appear in W (labels and centres measured at y 110-134).
const TABS: [(&str, f32, bool); 6] = [
    ("ALL", 40.0, true),
    ("PROJECT", 121.0, false),
    ("PLUGINS", 202.0, false),
    ("PRESETS", 283.0, false),
    ("SOUNDS", 364.0, false),
    ("STARRED", 445.0, false),
];

#[derive(Default)]
pub struct BrowserState {
    pub roots: Vec<PathBuf>,
    pub expanded: BTreeSet<PathBuf>,
    pub selected: Option<PathBuf>,
    pub scroll_y: f32,
    /// Audio file being dragged out of the Browser (dropped on the Channel rack).
    pub dragging: Option<PathBuf>,
    /// Audio file clicked for preview this frame [Manual: browser, sample preview].
    pub preview: Option<PathBuf>,
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

/// `rect` spans W x 0-489, from the desktop top (W y 84) to the bottom.
pub fn paint_browser(paint: &mut PaintCtx<'_>, state: &mut BrowserState, rect: Rect) {
    let t = *paint.tokens;
    let painter = paint.painter();
    let o = rect.min;
    let at =
        |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(o + vec2(x0, y0), o + vec2(x1, y1));
    painter.rect_filled(rect, 0.0, t.browser_body);
    paint
        .probe
        .insert("browser.panel", rect, "panel", true, None);

    // Title bar [W y 84-105].
    let title = at(0.0, 0.0, rect.width(), paint.layout.title_h);
    painter.rect_filled(title, 0.0, t.browser_title);
    let icon = t.browser_title_text;
    icons::chevron_right(&painter, at(6.0, 4.0, 14.0, 18.0), icon);
    icons::arrow_up(&painter, at(20.0, 4.0, 30.0, 18.0), icon);
    icons::undo(&painter, at(34.0, 4.0, 46.0, 18.0), icon);
    painter.text(
        o + vec2(52.0, 11.0),
        Align2::LEFT_CENTER,
        "Browser",
        theme::regular(paint.layout.font_title - 1.5),
        t.browser_title_text,
    );

    // Tab row [W y 106-138], separator [W y 139-141].
    painter.rect_filled(at(0.0, 22.0, rect.width(), 55.0), 0.0, t.browser_tabs);
    painter.rect_filled(at(0.0, 55.0, rect.width(), 58.0), 0.0, t.browser_tabs_line);
    for (label, center, enabled) in TABS {
        let tab = Rect::from_center_size(o + vec2(center, 38.5), vec2(78.0, 26.0));
        let hint = if enabled {
            "All - folders you add to the browser"
        } else {
            "Not implemented yet"
        };
        let _ = paint.hit(&format!("browser.tab.{label}"), tab, "tab", enabled, hint);
        let selected = label == "ALL";
        if selected {
            painter.rect_stroke(
                tab,
                5.0,
                Stroke::new(1.5, t.browser_tab_selected),
                StrokeKind::Inside,
            );
        }
        painter.text(
            tab.center(),
            Align2::CENTER_CENTER,
            label,
            theme::medium(paint.layout.font_tab),
            if selected {
                t.browser_tab_selected
            } else {
                t.browser_tab_text
            },
        );
    }

    // Navigation row [W y 142-170]: history arrows, back, add-folder.
    icons::chevron_left_line(
        &painter,
        at(8.0, 64.0, 20.0, 80.0),
        theme::shade(t.browser_title_text, 0.7),
    );
    icons::chevron_right_line(
        &painter,
        at(20.0, 64.0, 32.0, 80.0),
        theme::shade(t.browser_title_text, 0.7),
    );
    icons::undo(&painter, at(40.0, 64.0, 56.0, 80.0), t.browser_title_text);
    let add = at(455.0, 62.0, 477.0, 82.0);
    if paint
        .hit(
            "browser.add-folder",
            add,
            "button",
            true,
            "Add a folder to the browser (or drop one here)",
        )
        .clicked()
        && let Some(path) = rfd::FileDialog::new().pick_folder()
    {
        state.add_root(path);
    }
    icons::gear(&painter, add, t.browser_title_text);

    let content = Rect::from_min_max(o + vec2(0.0, 88.0), rect.max);
    if paint.ui.rect_contains_pointer(content) {
        let delta = paint.ui.input(|i| i.smooth_scroll_delta.y);
        state.scroll_y = (state.scroll_y - delta).max(0.0);
        let dropped = paint.ui.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            state.add_root(file.path().to_path_buf());
        }
    }
    let rows = state.visible_rows();
    let row_h = paint.layout.browser_row_h;
    if rows.is_empty() {
        painter.text(
            content.left_top() + vec2(14.0, 14.0),
            Align2::LEFT_TOP,
            "Drop a folder here or use the gear to add one.",
            theme::regular(14.0),
            theme::shade(t.browser_text, 0.7),
        );
        return;
    }
    let max_scroll = (rows.len() as f32 * row_h - content.height()).max(0.0);
    state.scroll_y = state.scroll_y.min(max_scroll);
    let start = (state.scroll_y / row_h).floor() as usize;
    let end = (((state.scroll_y + content.height()) / row_h).ceil() as usize).min(rows.len());
    let row_painter = painter.with_clip_rect(content);
    for (index, row) in rows.iter().enumerate().take(end).skip(start) {
        let y = content.top() + index as f32 * row_h - state.scroll_y;
        let row_rect =
            Rect::from_min_size(pos2(content.left(), y), vec2(content.width() - 12.0, row_h));
        let id = format!("browser.row.{index}");
        let response = paint.hit(
            &id,
            row_rect,
            if row.is_dir { "folder" } else { "file" },
            true,
            &row.path.display().to_string(),
        );
        let selected = state.selected.as_ref() == Some(&row.path);
        if selected || response.hovered() {
            row_painter.rect_filled(
                row_rect,
                0.0,
                if selected {
                    t.browser_row_selected
                } else {
                    t.browser_row_hover
                },
            );
        }
        let name = row
            .path
            .file_name()
            .map(|part| part.to_string_lossy().into_owned())
            .unwrap_or_else(|| row.path.display().to_string());
        let x = row_rect.left() + 10.0 + row.depth as f32 * 14.0;
        let icon_rect = Rect::from_min_size(pos2(x, y + 4.0), vec2(14.0, row_h - 8.0));
        if row.is_dir {
            row_painter.rect_filled(
                icon_rect.shrink2(vec2(1.0, 2.0)),
                1.0,
                theme::shade(t.browser_text, 0.55),
            );
        } else {
            row_painter.rect_filled(
                Rect::from_center_size(icon_rect.center(), vec2(8.0, 10.0)),
                0.0,
                theme::shade(t.browser_text, 0.45),
            );
        }
        row_painter.text(
            pos2(x + 20.0, row_rect.center().y),
            Align2::LEFT_CENTER,
            name,
            theme::regular(15.0),
            t.browser_text,
        );
        if response.clicked() {
            state.selected = Some(row.path.clone());
            if row.is_dir && !state.expanded.insert(row.path.clone()) {
                state.expanded.remove(&row.path);
            }
            if !row.is_dir && jet_media::is_supported(&row.path) {
                state.preview = Some(row.path.clone());
            }
        }
        if response.drag_started() && !row.is_dir && jet_media::is_supported(&row.path) {
            state.dragging = Some(row.path.clone());
        }
    }
    if max_scroll > 0.0 {
        let rail = Rect::from_min_max(pos2(content.right() - 10.0, content.top()), content.max);
        crate::widgets::hatch(&row_painter, rail, t.hatch_a, t.hatch_b);
        let thumb_h = (content.height() * content.height() / (rows.len() as f32 * row_h)).max(24.0);
        let thumb_y = content.top() + (content.height() - thumb_h) * (state.scroll_y / max_scroll);
        row_painter.rect_filled(
            Rect::from_min_size(pos2(rail.left(), thumb_y), vec2(rail.width(), thumb_h)),
            1.0,
            t.splitter,
        );
    }
}
