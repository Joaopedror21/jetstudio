//! Snap values [Manual: toolbar_panels, "Global Snap Panel"; pianoroll_menu, "Snap"]: "Main - Snap
//! is set to the Global snap value"; "Line - Events snap to the nearest grid-line, notice that the
//! grid changes resolution as the Piano roll, Playlist or Event Editor are Zoomed"; "Cell - Events
//! snap to the start of the grid-cell they fall in"; "(none) - No snapping"; "Steps 1/6 to 1
//! (step)"; "Beats 1/6 to 1 (beat)"; "Bar - 1 bar". "The snap options (none), Line & Cell are not
//! input quantized." The Events and Markers options are not offered yet.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SnapValue {
    /// Local snaps only: follow the global snap.
    Main,
    #[default]
    Line,
    Cell,
    None,
    /// A fraction 1/n of a step (n = 1..=6).
    Step(u8),
    /// A fraction 1/n of a beat (n = 1..=6).
    Beat(u8),
    Bar,
}

/// Tick sizes of the grid a snap resolves against. `line` is the spacing of the grid lines the
/// editor draws at its current zoom.
#[derive(Clone, Copy, Debug)]
pub struct GridTicks {
    pub step: i64,
    pub beat: i64,
    pub bar: i64,
    pub line: i64,
}

impl SnapValue {
    /// The Global snap selector's options, fine to coarse after the special ones.
    pub const GLOBAL: [SnapValue; 14] = [
        SnapValue::Line,
        SnapValue::Cell,
        SnapValue::None,
        SnapValue::Step(6),
        SnapValue::Step(4),
        SnapValue::Step(3),
        SnapValue::Step(2),
        SnapValue::Step(1),
        SnapValue::Beat(6),
        SnapValue::Beat(4),
        SnapValue::Beat(3),
        SnapValue::Beat(2),
        SnapValue::Beat(1),
        SnapValue::Bar,
    ];

    /// The options of a window's local snap: Main, then the global ones.
    pub fn local_options() -> Vec<SnapValue> {
        std::iter::once(SnapValue::Main)
            .chain(Self::GLOBAL)
            .collect()
    }

    pub fn global_options() -> Vec<SnapValue> {
        Self::GLOBAL.to_vec()
    }

    pub fn label(self) -> String {
        match self {
            SnapValue::Main => "Main".to_owned(),
            SnapValue::Line => "Line".to_owned(),
            SnapValue::Cell => "Cell".to_owned(),
            SnapValue::None => "(none)".to_owned(),
            SnapValue::Step(1) => "Step".to_owned(),
            SnapValue::Step(n) => format!("1/{n} step"),
            SnapValue::Beat(1) => "Beat".to_owned(),
            SnapValue::Beat(n) => format!("1/{n} beat"),
            SnapValue::Bar => "Bar".to_owned(),
        }
    }

    /// `Main` takes the global value.
    pub fn resolve(self, global: SnapValue) -> SnapValue {
        if self == SnapValue::Main {
            global
        } else {
            self
        }
    }

    /// Grid size in ticks (at least 1). `Main` must be resolved first; it counts as Line.
    pub fn ticks(self, grid: GridTicks) -> i64 {
        let fraction = |whole: i64, n: u8| (whole / i64::from(n.max(1))).max(1);
        match self {
            SnapValue::Main | SnapValue::Line | SnapValue::Cell => grid.line.max(1),
            SnapValue::None => 1,
            SnapValue::Step(n) => fraction(grid.step, n),
            SnapValue::Beat(n) => fraction(grid.beat, n),
            SnapValue::Bar => grid.bar.max(1),
        }
    }

    /// "The snap options (none), Line & Cell are not input quantized."
    pub fn quantizes_input(self) -> bool {
        !matches!(
            self,
            SnapValue::Main | SnapValue::Line | SnapValue::Cell | SnapValue::None
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snaps_resolve_to_grid_sizes() {
        let grid = GridTicks {
            step: 24,
            beat: 96,
            bar: 384,
            line: 24,
        };
        assert_eq!(
            SnapValue::Main.resolve(SnapValue::Beat(2)),
            SnapValue::Beat(2)
        );
        assert_eq!(
            SnapValue::Step(1).resolve(SnapValue::Bar),
            SnapValue::Step(1)
        );
        assert_eq!(SnapValue::Beat(2).ticks(grid), 48);
        assert_eq!(SnapValue::Step(3).ticks(grid), 8);
        assert_eq!(SnapValue::Line.ticks(grid), 24);
        assert_eq!(SnapValue::None.ticks(grid), 1);
        assert_eq!(SnapValue::Bar.ticks(grid), 384);
        assert!(SnapValue::Step(4).quantizes_input());
        assert!(!SnapValue::Line.quantizes_input() && !SnapValue::None.quantizes_input());
        assert_eq!(SnapValue::local_options().len(), 15);
        assert_eq!(SnapValue::global_options()[0], SnapValue::Line);
        assert_eq!(SnapValue::Beat(4).label(), "1/4 beat");
    }
}
