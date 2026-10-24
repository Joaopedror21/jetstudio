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
    /// Search text [Manual: browser, "Search (Ctrl+F) - Search all folders for files"].
    pub search: String,
    /// The search field has keyboard focus.
    pub search_focus: bool,
    /// Rows found for `search` (the query they were found for, and the rows).
    found: Option<(String, Vec<BrowserRow>)>,
    /// Index of the current match for F3 (next match).
    current: Option<usize>,
}

impl BrowserState {
    pub fn add_root(&mut self, path: PathBuf) {
        if path.is_dir() && !self.roots.contains(&path) {
            self.expanded.insert(path.clone());
            self.roots.push(path);
            self.roots.sort();
        }
    }

    fn visible_rows(&mut self) -> Vec<BrowserRow> {
        let query = self.search.trim().to_lowercase();
        if !query.is_empty() {
            if self.found.as_ref().is_none_or(|(q, _)| *q != query) {
                let rows = search_rows(&self.roots, &query, SEARCH_LIMIT);
                self.found = Some((query, rows));
                self.current = None;
            }
            return self
                .found
                .as_ref()
                .map_or_else(Vec::new, |(_, rows)| rows.clone());
        }
        self.found = None;
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
            matched: false,
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrowserRow {
    path: PathBuf,
    depth: usize,
    is_dir: bool,
    /// A search match (rows shown only as the parents of matches are not).
    matched: bool,
}

/// Entries visited at most per search, so a huge folder cannot stall the UI frame.
const SEARCH_LIMIT: usize = 50_000;

/// Files and folders under `roots` whose names contain `query` (lower case), each shown under its
/// parent folders: "Searches for files and folders using non-case sensitive searching";
/// "Searches include sub-folders"; "Show parent folders in search results" [Manual: browser].
fn search_rows(roots: &[PathBuf], query: &str, limit: usize) -> Vec<BrowserRow> {
    fn walk(
        path: &Path,
        depth: usize,
        query: &str,
        budget: &mut usize,
        out: &mut Vec<BrowserRow>,
    ) -> bool {
        if *budget == 0 || depth > 16 {
            return false;
        }
        *budget -= 1;
        let is_dir = path.is_dir();
        let matched = depth > 0
            && path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().to_lowercase().contains(query));
        let at = out.len();
        out.push(BrowserRow {
            path: path.to_path_buf(),
            depth,
            is_dir,
            matched,
        });
        let mut any = matched;
        if is_dir && let Ok(entries) = fs::read_dir(path) {
            let mut children: Vec<PathBuf> =
                entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
            children.sort_by(|a, b| {
                b.is_dir()
                    .cmp(&a.is_dir())
                    .then_with(|| a.file_name().cmp(&b.file_name()))
            });
            for child in children {
                any |= walk(&child, depth + 1, query, budget, out);
            }
        }
        if !any && depth > 0 {
            out.truncate(at); // nothing found here: drop this row and its subtree
        }
        any
    }
    let mut out = Vec::new();
    let mut budget = limit;
    for root in roots {
        let at = out.len();
        if !walk(root, 0, query, &mut budget, &mut out) {
            out.truncate(at);
        }
    }
    out
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

    // Search bar at the bottom [Manual: browser, "The search tools are located at the bottom of
    // the Browser by default"]. Geometry is ours (UNMEASURED).
    let bar = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 30.0), rect.max);
    painter.rect_filled(bar, 0.0, t.browser_tabs);
    let field = bar.shrink2(vec2(8.0, 4.0));
    let field_response = paint.hit(
        "browser.search",
        field,
        "text-field",
        true,
        "Search (Ctrl+F): type to find files and folders; F3 jumps to the next match, Esc cancels",
    );
    if field_response.clicked() {
        state.search_focus = true;
    } else if paint.ui.input(|i| i.pointer.any_pressed()) && !field_response.hovered() {
        state.search_focus = false;
    }
    crate::widgets::framed(
        &painter,
        field,
        2.0,
        t.field_dark,
        if state.search_focus {
            t.browser_tab_selected
        } else {
            t.field_dark_edge
        },
    );
    let shown = if state.search.is_empty() && !state.search_focus {
        "Search".to_owned()
    } else if state.search_focus {
        format!("{}|", state.search)
    } else {
        state.search.clone()
    };
    painter.with_clip_rect(field).text(
        pos2(field.left() + 6.0, field.center().y),
        Align2::LEFT_CENTER,
        shown,
        theme::regular(14.0),
        if state.search.is_empty() {
            theme::shade(t.field_text, 0.6)
        } else {
            t.field_text
        },
    );
    if state.search_focus {
        for event in paint.ui.input(|i| i.events.clone()) {
            match event {
                egui::Event::Text(typed) => state.search.push_str(&typed),
                egui::Event::Key {
                    key: egui::Key::Backspace,
                    pressed: true,
                    ..
                } => {
                    state.search.pop();
                }
                // "To cancel a search, press Esc."
                egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                } => {
                    state.search.clear();
                    state.search_focus = false;
                }
                _ => {}
            }
        }
    }
    let f3 = paint.ui.input(|i| i.key_pressed(egui::Key::F3));

    let content = Rect::from_min_max(o + vec2(0.0, 88.0), pos2(rect.right(), bar.top()));
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
    // "Use (F3) to skip to the next matching item": select it and scroll it into view.
    if f3 && !state.search.trim().is_empty() {
        let matches: Vec<usize> = (0..rows.len()).filter(|i| rows[*i].matched).collect();
        if !matches.is_empty() {
            let next = state
                .current
                .and_then(|c| matches.iter().position(|m| *m == c))
                .map_or(0, |p| (p + 1) % matches.len());
            let row = matches[next];
            state.current = Some(row);
            state.selected = Some(rows[row].path.clone());
            state.scroll_y = (row as f32 * row_h - content.height() / 2.0).max(0.0);
        }
    }
    if rows.is_empty() && !state.search.trim().is_empty() {
        painter.text(
            content.left_top() + vec2(14.0, 14.0),
            Align2::LEFT_TOP,
            "Nothing found.",
            theme::regular(14.0),
            theme::shade(t.browser_text, 0.7),
        );
        return;
    }
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
            if row.matched {
                t.browser_tab_selected
            } else {
                t.browser_text
            },
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn search_finds_names_in_sub_folders_case_insensitively_with_their_parents() {
        let base = std::env::temp_dir().join(format!("jet-browser-search-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("Drums").join("Kicks")).unwrap();
        fs::create_dir_all(base.join("Synths")).unwrap();
        fs::write(
            base.join("Drums").join("Kicks").join("Big KICK 01.wav"),
            b"",
        )
        .unwrap();
        fs::write(base.join("Drums").join("snare.wav"), b"").unwrap();
        fs::write(base.join("Synths").join("pad.wav"), b"").unwrap();

        let rows = search_rows(std::slice::from_ref(&base), "kick", 1000);
        let names: Vec<(String, bool)> = rows
            .iter()
            .map(|r| {
                (
                    r.path.file_name().unwrap().to_string_lossy().into_owned(),
                    r.matched,
                )
            })
            .collect();
        let root_name = base.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(
            names,
            vec![
                (root_name, false),
                ("Drums".to_owned(), false),
                ("Kicks".to_owned(), true),
                ("Big KICK 01.wav".to_owned(), true),
            ],
            "parents shown, unrelated files and folders hidden"
        );
        assert!(search_rows(std::slice::from_ref(&base), "zzz", 1000).is_empty());
        let _ = fs::remove_dir_all(&base);
    }
}
