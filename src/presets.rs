//! FilmCraft preset layouts are serialized using the legacy `dock.rs` layout
//! representation. Current FilmCraft also accepts these when `workspace` is null.
use serde::Serialize;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style { Vegas, Resolve, Graphite }

impl Style {
    pub const ALL: [Style; 3] = [Style::Vegas, Style::Resolve, Style::Graphite];
    pub fn title(self) -> &'static str {
        match self { Self::Vegas => "VEGAS Flow", Self::Resolve => "Resolve Flow", Self::Graphite => "Graphite Studio" }
    }
    pub fn workspace_name(self) -> String { format!("CraftSkin · {}", self.title()) }
    pub fn description(self) -> &'static str {
        match self {
            Self::Vegas => "Large bottom timeline, project bins on the upper left, program monitor on the right.",
            Self::Resolve => "Media browser, central viewing area, inspector sidecar and an organized lower timeline.",
            Self::Graphite => "Source/program editing split and a compact, balanced timeline workspace.",
        }
    }
    pub fn palette(self) -> Palette {
        // Color families are reconstructed from publicly documented editors.
        // No artwork, branding, fonts or icons are reused.
        match self {
            Self::Vegas => Palette { app: "#202020", header: "#303030", panel: "#333333", tab: "#292929", text: "#E3E3E3", muted: "#B0B0B0", accent: "#4A8FC9", timeline: "#292929", track: "#303030", hover: "#454545" },
            Self::Resolve => Palette { app: "#181818", header: "#282828", panel: "#292929", tab: "#242424", text: "#DCDCDC", muted: "#A4A4A4", accent: "#CB5558", timeline: "#1D1D1D", track: "#262626", hover: "#3C3C3C" },
            Self::Graphite => Palette { app: "#13151A", header: "#232630", panel: "#272B35", tab: "#20242D", text: "#E8EBF2", muted: "#A0A8B8", accent: "#B4A0EE", timeline: "#191C24", track: "#252A34", hover: "#414858" },
        }
    }
    pub fn tree(self) -> Tree {
        // Panels use the exact `PanelKind` variant names in FilmCraft's dock.rs.
        // Every panel appears once: the docking validator rejects duplicate panels.
        let tools_and_timeline = || h(0.055, tab(&["Tools"]),
            h(0.91, tab(&["Timeline"]), tab(&["AudioMeters"])));
        match self {
            Style::Vegas => v(0.38,
                h(0.45,
                    tab(&["Project", "MediaBrowser", "Effects", "Libraries", "History"]),
                    h(0.78, tab(&["Program", "Source", "ReferenceMonitor"]),
                        tab(&["Properties", "EffectControls", "LumetriColor"]))),
                tools_and_timeline()),
            Style::Resolve => v(0.53,
                h(0.22,
                    tab(&["Project", "MediaBrowser", "Effects", "Libraries"]),
                    h(0.73,
                        tab(&["Program", "Source", "LumetriScopes"]),
                        tab(&["Properties", "EffectControls", "LumetriColor", "EssentialGraphics"]))),
                tools_and_timeline()),
            Style::Graphite => v(0.50,
                h(0.77,
                    h(0.47, tab(&["Source", "EffectControls", "AudioClipMixer"]), tab(&["Program"])),
                    tab(&["Properties", "LumetriColor"])),
                h(0.26, tab(&["Project", "MediaBrowser", "Effects", "History"]), tools_and_timeline())),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub app: &'static str, pub header: &'static str, pub panel: &'static str,
    pub tab: &'static str, pub text: &'static str, pub muted: &'static str,
    pub accent: &'static str, pub timeline: &'static str, pub track: &'static str,
    pub hover: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub enum Tree {
    Split { vertical: bool, size: SplitSize, a: Box<Tree>, b: Box<Tree> },
    Tabs { panels: Vec<String>, active: usize },
}

#[derive(Clone, Debug, Serialize)]
pub enum SplitSize { Ratio(f32) }

pub fn tab(names: &[&str]) -> Tree {
    Tree::Tabs { panels: names.iter().map(|s| (*s).to_owned()).collect(), active: 0 }
}
pub fn h(ratio: f32, a: Tree, b: Tree) -> Tree {
    Tree::Split { vertical: false, size: SplitSize::Ratio(ratio), a: Box::new(a), b: Box::new(b) }
}
pub fn v(ratio: f32, a: Tree, b: Tree) -> Tree {
    Tree::Split { vertical: true, size: SplitSize::Ratio(ratio), a: Box::new(a), b: Box::new(b) }
}

pub fn validate_tree(tree: &Tree) -> Result<(), String> {
    fn walk(tree: &Tree, depth: usize, seen: &mut HashSet<String>) -> Result<(), String> {
        if depth > 24 { return Err("Workspace nesting is too deep".into()); }
        match tree {
            Tree::Tabs { panels, active } => {
                if panels.is_empty() || *active >= panels.len() { return Err("Invalid panel tabs".into()); }
                for p in panels {
                    if !seen.insert(p.clone()) { return Err(format!("Duplicate panel: {p}")); }
                }
            }
            Tree::Split { size: SplitSize::Ratio(r), a, b, .. } => {
                if !r.is_finite() || *r <= 0.02 || *r >= 0.98 { return Err("Bad panel split ratio".into()); }
                walk(a, depth + 1, seen)?;
                walk(b, depth + 1, seen)?;
            }
        }
        Ok(())
    }
    let mut seen = HashSet::new();
    walk(tree, 0, &mut seen)?;
    if !seen.contains("Timeline") || !seen.contains("Program") { return Err("A workspace must retain Timeline and Program".into()); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layouts_have_unique_panels_and_expected_main_panes() {
        for style in Style::ALL { validate_tree(&style.tree()).unwrap(); }
    }
    #[test]
    fn serialized_legacy_tree_matches_filmcraft_variants() {
        let v = serde_json::to_value(h(0.5, tab(&["Project"]), tab(&["Program"]))).unwrap();
        assert_eq!(v["Split"]["vertical"], false);
        assert_eq!(v["Split"]["size"]["Ratio"], 0.5);
        assert_eq!(v["Split"]["a"]["Tabs"]["panels"][0], "Project");
    }
}
