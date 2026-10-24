use egui::{Pos2, Rect, Vec2, pos2, vec2};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WindowKind {
    ChannelRack,
    Playlist,
    PianoRoll,
    Mixer,
    ChannelSettings,
    AudioSettings,
    GraphEditor,
    AutomationSettings,
}

impl WindowKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::ChannelRack => "Channel rack",
            Self::Playlist => "Playlist",
            Self::PianoRoll => "Piano roll",
            Self::Mixer => "Mixer",
            Self::ChannelSettings => "Channel settings",
            Self::AudioSettings => "Audio settings",
            Self::GraphEditor => "Graph editor",
            Self::AutomationSettings => "Automation clip settings",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::ChannelRack => "channel-rack",
            Self::Playlist => "playlist",
            Self::PianoRoll => "piano-roll",
            Self::Mixer => "mixer",
            Self::ChannelSettings => "channel-settings",
            Self::AudioSettings => "audio-settings",
            Self::GraphEditor => "graph-editor",
            Self::AutomationSettings => "automation-settings",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResizeEdge {
    Left,
    Right,
    Top,
    Bottom,
    BottomRight,
}

#[derive(Clone, Debug)]
pub struct WinState {
    pub kind: WindowKind,
    pub rect: Rect,
    pub visible: bool,
    pub maximized: bool,
    pub restore_rect: Option<Rect>,
}

#[derive(Clone, Debug)]
pub struct Workspace {
    /// Last entry is painted and hit-tested in front.
    pub windows: Vec<WinState>,
    pub focused: Option<WindowKind>,
    pub browser_visible: bool,
    pub browser_width: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum WsAction {
    Toggle(WindowKind),
    Focus(WindowKind),
    Close(WindowKind),
    Move {
        kind: WindowKind,
        to: Pos2,
        snap: bool,
    },
    Resize {
        kind: WindowKind,
        edge: ResizeEdge,
        delta: Vec2,
    },
    Maximize(WindowKind),
    Restore(WindowKind),
    CycleNested,
    ArrangeDefault,
    CloseAll,
    ToggleBrowser,
}

#[derive(Debug, Eq, PartialEq)]
pub enum WsError {
    MissingWindow,
    InvalidRect,
}

impl Workspace {
    pub fn new(desktop: Rect, browser_width: f32) -> Self {
        // [W] A new project shows the Playlist maximized in the desktop and the Channel Rack
        // closed. Restore rectangles are UNMEASURED: no restored window has been captured yet;
        // the Channel Rack width fits its 16 default steps (201 + 16 x 15.5 + margin).
        let playlist_restore = Rect::from_min_size(
            desktop.min + vec2(desktop.width() * 0.05, desktop.height() * 0.05),
            vec2(desktop.width() * 0.9, desktop.height() * 0.85),
        );
        let channel_rect = Rect::from_min_size(
            desktop.min + vec2(300.0, 120.0),
            vec2(
                470.0_f32.min(desktop.width() - 40.0).max(240.0),
                420.0_f32.min(desktop.height() - 40.0).max(160.0),
            ),
        );
        Self {
            windows: vec![
                WinState {
                    kind: WindowKind::Playlist,
                    rect: desktop,
                    visible: true,
                    maximized: true,
                    restore_rect: Some(playlist_restore),
                },
                WinState {
                    kind: WindowKind::ChannelRack,
                    rect: channel_rect,
                    visible: false,
                    maximized: false,
                    restore_rect: None,
                },
                // UNMEASURED: Piano roll and Mixer default rectangles (no restored capture).
                WinState {
                    kind: WindowKind::PianoRoll,
                    rect: Rect::from_min_size(
                        desktop.min + vec2(40.0, 60.0),
                        vec2(
                            (desktop.width() - 80.0).max(240.0),
                            (desktop.height() - 100.0).max(160.0),
                        ),
                    ),
                    visible: false,
                    maximized: false,
                    restore_rect: None,
                },
                WinState {
                    kind: WindowKind::ChannelSettings,
                    // Right-aligned inside the desktop so it never starts off screen.
                    rect: {
                        let size = vec2(
                            460.0_f32.min(desktop.width() - 40.0).max(240.0),
                            (desktop.height() - 80.0).clamp(160.0, 760.0),
                        );
                        let x = (desktop.right() - size.x - 20.0).max(desktop.left());
                        Rect::from_min_size(pos2(x, desktop.top() + 40.0), size)
                    },
                    visible: false,
                    maximized: false,
                    restore_rect: None,
                },
                WinState {
                    kind: WindowKind::AudioSettings,
                    rect: {
                        let size = vec2(
                            760.0_f32.min(desktop.width() - 40.0).max(240.0),
                            (desktop.height() - 80.0).clamp(160.0, 640.0),
                        );
                        Rect::from_min_size(
                            pos2(
                                (desktop.center().x - size.x / 2.0).max(desktop.left()),
                                desktop.top() + 30.0,
                            ),
                            size,
                        )
                    },
                    visible: false,
                    maximized: false,
                    restore_rect: None,
                },
                // UNMEASURED: the Graph editor opens docked under the Channel rack in FL; here it
                // is a child window placed near the bottom of the desktop.
                WinState {
                    kind: WindowKind::GraphEditor,
                    rect: {
                        let size = vec2(
                            620.0_f32.min(desktop.width() - 40.0).max(240.0),
                            240.0_f32.min(desktop.height() - 40.0).max(160.0),
                        );
                        Rect::from_min_size(
                            pos2(
                                desktop.left() + 300.0_f32.min((desktop.width() - size.x).max(0.0)),
                                (desktop.bottom() - size.y - 20.0).max(desktop.top()),
                            ),
                            size,
                        )
                    },
                    visible: false,
                    maximized: false,
                    restore_rect: None,
                },
                WinState {
                    kind: WindowKind::AutomationSettings,
                    // Right-aligned like Channel settings so it never starts off screen.
                    rect: {
                        let size = vec2(
                            420.0_f32.min(desktop.width() - 40.0).max(240.0),
                            (desktop.height() - 80.0).clamp(160.0, 420.0),
                        );
                        let x = (desktop.right() - size.x - 30.0).max(desktop.left());
                        Rect::from_min_size(pos2(x, desktop.top() + 60.0), size)
                    },
                    visible: false,
                    maximized: false,
                    restore_rect: None,
                },
                WinState {
                    kind: WindowKind::Mixer,
                    rect: Rect::from_min_size(
                        desktop.min + vec2(20.0, desktop.height() * 0.35),
                        vec2(
                            (desktop.width() - 40.0).max(240.0),
                            (desktop.height() * 0.62).max(160.0),
                        ),
                    ),
                    visible: false,
                    maximized: false,
                    restore_rect: None,
                },
            ],
            focused: Some(WindowKind::Playlist),
            browser_visible: true,
            browser_width,
        }
    }

    pub fn window(&self, kind: WindowKind) -> Option<&WinState> {
        self.windows.iter().find(|w| w.kind == kind)
    }

    pub fn apply(
        &mut self,
        action: WsAction,
        desktop: Rect,
        snap_distance: f32,
    ) -> Result<(), WsError> {
        let mut next = self.clone();
        next.apply_inner(action, desktop, snap_distance)?;
        next.validate(desktop)?;
        *self = next;
        Ok(())
    }

    fn apply_inner(
        &mut self,
        action: WsAction,
        desktop: Rect,
        snap_distance: f32,
    ) -> Result<(), WsError> {
        match action {
            WsAction::Toggle(kind) => {
                let visible = self.window(kind).ok_or(WsError::MissingWindow)?.visible;
                if visible && self.focused == Some(kind) {
                    self.window_mut(kind)?.visible = false;
                    self.focused = self
                        .windows
                        .iter()
                        .rev()
                        .find(|w| w.visible)
                        .map(|w| w.kind);
                } else {
                    self.window_mut(kind)?.visible = true;
                    self.focus(kind)?;
                }
            }
            WsAction::Focus(kind) => self.focus(kind)?,
            WsAction::Close(kind) => {
                self.window_mut(kind)?.visible = false;
                if self.focused == Some(kind) {
                    self.focused = self
                        .windows
                        .iter()
                        .rev()
                        .find(|w| w.visible)
                        .map(|w| w.kind);
                }
            }
            WsAction::Move { kind, to, snap } => {
                let old = self.window(kind).ok_or(WsError::MissingWindow)?.rect;
                let mut rect = Rect::from_min_size(to, old.size());
                rect = keep_reachable(rect, desktop);
                if snap {
                    rect = self.snap_rect(kind, rect, desktop, snap_distance);
                }
                let win = self.window_mut(kind)?;
                if !win.maximized {
                    win.rect = rect;
                }
            }
            WsAction::Resize { kind, edge, delta } => {
                let win = self.window_mut(kind)?;
                if !win.maximized {
                    let mut rect = win.rect;
                    match edge {
                        ResizeEdge::Left => rect.min.x += delta.x,
                        ResizeEdge::Right => rect.max.x += delta.x,
                        ResizeEdge::Top => rect.min.y += delta.y,
                        ResizeEdge::Bottom => rect.max.y += delta.y,
                        ResizeEdge::BottomRight => {
                            rect.max += delta;
                        }
                    }
                    rect.max.x = rect.max.x.max(rect.min.x + 180.0);
                    rect.max.y = rect.max.y.max(rect.min.y + 100.0);
                    win.rect = keep_reachable(rect, desktop);
                }
            }
            WsAction::Maximize(kind) => {
                let win = self.window_mut(kind)?;
                if !win.maximized {
                    win.restore_rect = Some(win.rect);
                    win.rect = desktop;
                    win.maximized = true;
                }
                self.focus(kind)?;
            }
            WsAction::Restore(kind) => {
                let win = self.window_mut(kind)?;
                if win.maximized {
                    if let Some(rect) = win.restore_rect.take() {
                        win.rect = keep_reachable(rect, desktop);
                    }
                    win.maximized = false;
                }
            }
            WsAction::CycleNested => {
                if let Some(kind) = self
                    .windows
                    .iter()
                    .rev()
                    .find(|w| w.visible && Some(w.kind) != self.focused)
                    .map(|w| w.kind)
                {
                    self.focus(kind)?;
                }
            }
            WsAction::ArrangeDefault => {
                let default = Self::new(desktop, self.browser_width);
                self.windows = default.windows;
                self.focused = default.focused;
            }
            WsAction::CloseAll => {
                for win in &mut self.windows {
                    win.visible = false;
                }
                self.focused = None;
            }
            WsAction::ToggleBrowser => self.browser_visible = !self.browser_visible,
        }
        Ok(())
    }

    fn focus(&mut self, kind: WindowKind) -> Result<(), WsError> {
        let index = self
            .windows
            .iter()
            .position(|w| w.kind == kind)
            .ok_or(WsError::MissingWindow)?;
        let mut win = self.windows.remove(index);
        win.visible = true;
        self.windows.push(win);
        self.focused = Some(kind);
        Ok(())
    }

    fn window_mut(&mut self, kind: WindowKind) -> Result<&mut WinState, WsError> {
        self.windows
            .iter_mut()
            .find(|w| w.kind == kind)
            .ok_or(WsError::MissingWindow)
    }

    fn snap_rect(&self, kind: WindowKind, mut rect: Rect, desktop: Rect, distance: f32) -> Rect {
        let mut x = vec![desktop.left(), desktop.right()];
        let mut y = vec![desktop.top(), desktop.bottom()];
        for win in self.windows.iter().filter(|w| w.visible && w.kind != kind) {
            x.extend([win.rect.left(), win.rect.right()]);
            y.extend([win.rect.top(), win.rect.bottom()]);
        }
        for boundary in x {
            if (rect.left() - boundary).abs() <= distance {
                rect = rect.translate(vec2(boundary - rect.left(), 0.0));
                break;
            }
            if (rect.right() - boundary).abs() <= distance {
                rect = rect.translate(vec2(boundary - rect.right(), 0.0));
                break;
            }
        }
        for boundary in y {
            if (rect.top() - boundary).abs() <= distance {
                rect = rect.translate(vec2(0.0, boundary - rect.top()));
                break;
            }
            if (rect.bottom() - boundary).abs() <= distance {
                rect = rect.translate(vec2(0.0, boundary - rect.bottom()));
                break;
            }
        }
        keep_reachable(rect, desktop)
    }

    fn validate(&self, desktop: Rect) -> Result<(), WsError> {
        if self
            .windows
            .iter()
            .any(|w| !w.rect.is_finite() || w.rect.width() < 1.0 || w.rect.height() < 1.0)
        {
            return Err(WsError::InvalidRect);
        }
        if self
            .windows
            .iter()
            .filter(|w| w.kind == WindowKind::Playlist)
            .count()
            != 1
            || self
                .windows
                .iter()
                .filter(|w| w.kind == WindowKind::ChannelRack)
                .count()
                != 1
            || self
                .focused
                .is_some_and(|kind| !self.window(kind).is_some_and(|w| w.visible))
            || !desktop.is_finite()
        {
            return Err(WsError::InvalidRect);
        }
        Ok(())
    }
}

fn keep_reachable(rect: Rect, desktop: Rect) -> Rect {
    let x = rect
        .left()
        .clamp(desktop.left() - rect.width() + 40.0, desktop.right() - 40.0);
    let y = rect.top().clamp(desktop.top(), desktop.bottom() - 28.0);
    Rect::from_min_size(pos2(x, y), rect.size())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn desktop() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 800.0))
    }

    #[test]
    fn new_project_layout_matches_reference() {
        let ws = Workspace::new(desktop(), 200.0);
        let playlist = ws.window(WindowKind::Playlist).unwrap();
        assert!(playlist.visible && playlist.maximized);
        assert_eq!(playlist.rect, desktop());
        assert!(!ws.window(WindowKind::ChannelRack).unwrap().visible);
        assert_eq!(ws.focused, Some(WindowKind::Playlist));
    }

    #[test]
    fn maximize_restore_retains_rect() {
        let mut ws = Workspace::new(desktop(), 200.0);
        let prior = ws.window(WindowKind::ChannelRack).unwrap().rect;
        ws.apply(WsAction::Maximize(WindowKind::ChannelRack), desktop(), 12.0)
            .unwrap();
        ws.apply(WsAction::Restore(WindowKind::ChannelRack), desktop(), 12.0)
            .unwrap();
        assert_eq!(ws.window(WindowKind::ChannelRack).unwrap().rect, prior);
    }

    #[test]
    fn hidden_window_can_be_reopened_and_focused() {
        let mut ws = Workspace::new(desktop(), 200.0);
        ws.apply(WsAction::Close(WindowKind::ChannelRack), desktop(), 12.0)
            .unwrap();
        ws.apply(WsAction::Toggle(WindowKind::ChannelRack), desktop(), 12.0)
            .unwrap();
        assert_eq!(ws.focused, Some(WindowKind::ChannelRack));
        assert!(ws.window(WindowKind::ChannelRack).unwrap().visible);
    }

    #[test]
    fn resize_changes_window_geometry_without_invalid_state() {
        let mut ws = Workspace::new(desktop(), 200.0);
        let before = ws.window(WindowKind::ChannelRack).unwrap().rect;
        ws.apply(
            WsAction::Resize {
                kind: WindowKind::ChannelRack,
                edge: ResizeEdge::BottomRight,
                delta: vec2(35.0, 24.0),
            },
            desktop(),
            12.0,
        )
        .unwrap();
        let after = ws.window(WindowKind::ChannelRack).unwrap().rect;
        assert_eq!(after.min, before.min);
        assert_eq!(after.width(), before.width() + 35.0);
        assert_eq!(after.height(), before.height() + 24.0);
    }
}
