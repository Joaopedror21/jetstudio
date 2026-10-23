use std::collections::BTreeMap;

use egui::Rect;

#[derive(Clone, Debug)]
pub struct ProbeEntry {
    pub rect: Rect,
    pub role: &'static str,
    pub enabled: bool,
    pub value: Option<String>,
}

#[derive(Default)]
pub struct LayoutProbe {
    entries: BTreeMap<String, ProbeEntry>,
}

impl LayoutProbe {
    pub fn begin_frame(&mut self) {
        self.entries.clear();
    }

    pub fn insert(
        &mut self,
        id: impl Into<String>,
        rect: Rect,
        role: &'static str,
        enabled: bool,
        value: Option<String>,
    ) {
        self.entries.insert(
            id.into(),
            ProbeEntry {
                rect,
                role,
                enabled,
                value,
            },
        );
    }

    pub fn entries(&self) -> &BTreeMap<String, ProbeEntry> {
        &self.entries
    }
}
