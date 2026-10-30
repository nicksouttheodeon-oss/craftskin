//! Guarded FilmCraft source adapter, opt-in and all-or-nothing at planning time.
//! Does not access the ArtCraft launcher, install files, or compiled executables.
use crate::install::{Result, install_batch, restore_batch};
use crate::{skinpack, presets::Style};
use std::path::{Path, PathBuf};

const BRIDGE: &str = include_str!("craftskin_bridge_template.rs");
const FILES: [&str; 9] = [
    "crates/ui-egui/src/lib.rs",
    "crates/ui-egui/src/theme.rs",
    "crates/ui-egui/src/header.rs",
    "crates/ui-egui/src/panels/monitor.rs",
    "crates/ui-egui/src/panels/timeline.rs",
    "crates/ui-egui/src/panels/timeline/ruler.rs",
    "crates/ui-egui/src/panels/timeline/clips.rs",
    "crates/ui-egui/src/panels/tools.rs",
    "crates/ui-egui/src/panels/settings.rs",
];
const BRIDGE_PATH: &str = "crates/ui-egui/src/craftskin_bridge.rs";

/// Find the unique expected code fragment, tolerating rustfmt-only whitespace
/// changes but never changing identifiers, punctuation or operators. We do not
/// patch if zero or multiple equivalent fragments are found.
fn span_without_whitespace(src: &str, needle: &str) -> Option<(usize, usize, usize)> {
    let mut compact = Vec::with_capacity(src.len());
    let mut positions = Vec::with_capacity(src.len());
    for (i, b) in src.bytes().enumerate() {
        if !b.is_ascii_whitespace() { compact.push(b); positions.push(i); }
    }
    let expected: Vec<u8> = needle.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if expected.is_empty() || expected.len() > compact.len() { return None; }
    let mut first = None;
    let mut count = 0;
    for (i, window) in compact.windows(expected.len()).enumerate() {
        if window == expected.as_slice() {
            count += 1;
            if count == 1 { first = Some((positions[i], positions[i + expected.len() - 1] + 1)); }
            if count > 1 { return Some((0, 0, count)); }
        }
    }
    first.map(|(start, end)| (start, end, count))
}
fn once(src: &str, from: &str, to: &str, name: &str) -> Result<String> {
    let exact = src.matches(from).count();
    let span = if exact == 1 {
        let start = src.find(from).expect("one exact match");
        Some((start, start + from.len(), 1))
    } else if exact == 0 {
        span_without_whitespace(src, from)
    } else { None };
    let Some((start, end, 1)) = span else {
        let likely = from.lines().find(|s| !s.trim().is_empty()).unwrap_or(from).trim();
        return Err(format!("FilmCraft adapter mismatch ({name}): expected one compatible anchor, found {exact} exact match(es); source left unchanged. Expected fragment starts: {likely:?}"));
    };
    let mut out = String::with_capacity(src.len() + to.len());
    out.push_str(&src[..start]);
    out.push_str(to);
    out.push_str(&src[end..]);
    Ok(out)
}
/// Patch only the terminal `self` expression of the real Tokens::with_appearance
/// method. The old adapter expected a following documentation comment verbatim,
/// which is brittle across FilmCraft revisions. This still fails closed if the
/// method's signature/return shape is no longer recognizable.
fn patch_appearance(src: &str) -> Result<String> {
    const NAME: &str = "pub fn with_appearance(";
    if src.matches(NAME).count() != 1 {
        return Err("FilmCraft adapter mismatch (appearance overlay): expected one Tokens::with_appearance method; source left unchanged".into());
    }
    let start = src.find(NAME).expect("uniqueness verified");
    let after = &src[start..];
    let open = start + after.find('{').ok_or("FilmCraft adapter mismatch (appearance overlay): no method body; source left unchanged")?;
    let signature = &src[start..open];
    if !signature.contains("-> Self") || !signature.contains("highlight") || !signature.contains("accessible_contrast") {
        return Err("FilmCraft adapter mismatch (appearance overlay): appearance method signature changed; source left unchanged".into());
    }
    // Find the method's closing brace rather than assuming what comment or
    // function follows it. Nested braces in `if` statements and closures are OK.
    let mut depth = 0usize;
    let mut close = None;
    for (offset, byte) in src.as_bytes()[open..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1).ok_or("FilmCraft adapter mismatch (appearance overlay): malformed braces")?;
                if depth == 0 { close = Some(open + offset); break; }
            }
            _ => {}
        }
    }
    let close = close.ok_or("FilmCraft adapter mismatch (appearance overlay): unterminated method body")?;
    let body = &src[open + 1..close];
    let trimmed = body.trim_end();
    let Some(prefix) = trimmed.strip_suffix("self") else {
        return Err("FilmCraft adapter mismatch (appearance overlay): method no longer ends in `self`; source left unchanged".into());
    };
    // Ensure `self` is the complete last expression on its own line, not a
    // suffix of a field name or an argument inside a different expression.
    if !prefix.rsplit('\n').next().unwrap_or("").chars().all(|c| c == ' ' || c == '\t') {
        return Err("FilmCraft adapter mismatch (appearance overlay): terminal `self` is not a standalone expression; source left unchanged".into());
    }
    let at = open + 1 + prefix.len();
    let mut result = src.to_owned();
    result.replace_range(at..at + "self".len(), "crate::craftskin_bridge::apply_tokens(self)");
    Ok(result)
}

fn patch(relative: &str, src: &str) -> Result<String> {
    if src.contains("crate::craftskin_bridge") || src.contains("pub mod craftskin_bridge;") {
        return Err(format!("{relative} is already patched; restore the previous source integration before reapplying"));
    }
    match relative {
        "crates/ui-egui/src/lib.rs" => once(src, "pub mod theme;", "pub mod theme;\n\npub mod craftskin_bridge;", "module registration"),
        "crates/ui-egui/src/theme.rs" => patch_appearance(src),
        "crates/ui-egui/src/header.rs" => once(src,
            "        x += gw + 24.0;",
            "        x += gw + crate::craftskin_bridge::header_mode_gap();", "header mode spacing"),
        "crates/ui-egui/src/panels/timeline.rs" => {
            let a = once(src,
                "    let header_w = app.ui.timeline.header_w;\n    let content = Rect::from_min_max(",
                "    app.ui.timeline.header_w = crate::craftskin_bridge::timeline_header_width(app.ui.timeline.header_w, rect.width());\n    let header_w = app.ui.timeline.header_w;\n    let content = Rect::from_min_max(", "track header geometry")?;
            once(&a,
                "        // a short head on the tick row, as in Effect Controls, so it leaves the markers and timecodes clear\n        let (top, tip) = (ruler.max.y - 13.0, ruler.max.y);\n        let head = [pos2(px - 5.0, top), pos2(px + 5.0, top), pos2(px + 5.0, tip - 5.0), pos2(px, tip), pos2(px - 5.0, tip - 5.0)];\n        painter.add(egui::Shape::convex_polygon(head.to_vec(), t.playhead, Stroke::NONE));\n        painter.line_segment([pos2(px, tip), pos2(px, content.max.y)], Stroke::new(1.0, t.playhead));",
                "        crate::craftskin_bridge::paint_playhead(&painter, px, ruler.max.y, content.max.y, t.playhead);",
                "timeline playhead shape")
        },
        "crates/ui-egui/src/panels/timeline/clips.rs" => once(src,
            "    let base = label_color(app, it.label);",
            "    let base = crate::craftskin_bridge::clip_base_color(label_color(app, it.label), kind == TrackKind::Video);",
            "timeline clip colors"),
        "crates/ui-egui/src/panels/timeline/ruler.rs" => {
            let before = once(src,
                "    let tc_rect = Rect::from_min_size(pos2(rect.min.x + 14.0, rect.min.y + 4.0), vec2(hw - 20.0, 20.0));",
                "    let tc_y = if crate::craftskin_bridge::timeline_toolbar_above() { 33.0 } else { 4.0 };\n    let tc_rect = Rect::from_min_size(pos2(rect.min.x + 14.0, rect.min.y + tc_y), vec2(hw - 20.0, 20.0));",
                "timeline timecode relocation")?;
            let a = once(&before, "    let mut x = rect.min.x + 12.0;\n    let y = rect.min.y + 26.0;",
                "    let mut x = rect.min.x + 12.0;\n    let y = rect.min.y + if crate::craftskin_bridge::timeline_toolbar_above() { 2.0 } else { 26.0 };\n    let tool_gap = crate::craftskin_bridge::timeline_tool_gap();", "timeline toolbar geometry")?;
            let b = once(&a,
                "    for (icon, key, on, toggle, tip) in toggles {",
                "    let mut toggles = toggles;\n    // Keep timeline command behavior and icons unchanged, just rearrange their placement.\n    if crate::craftskin_bridge::timeline_tool_order() == \"reverse\" { toggles.reverse(); }\n    for (icon, key, on, toggle, tip) in toggles {", "timeline button order")?;
            once(&b, "        x += 30.0;", "        x += 28.0 + tool_gap;", "timeline button density")
        }
        "crates/ui-egui/src/panels/monitor.rs" => {
            let a = once(src,
                "    buttons.retain(|(_, command, _)| transport_visible(&hidden, command));",
                "    buttons.retain(|(_, command, _)| transport_visible(&hidden, command));\n    buttons.sort_by_key(|(_, command, _)| crate::craftskin_bridge::transport_order(command));", "transport command reordering")?;
            let b = once(&a,
                "    let gap = if count * TRANSPORT_BUTTON + (count - 1.0).max(0.0) * 6.0 <= available { 6.0 } else { 2.0 };",
                "    let preferred_gap = crate::craftskin_bridge::transport_gap();\n    let gap = if count * TRANSPORT_BUTTON + (count - 1.0).max(0.0) * preferred_gap <= available { preferred_gap } else { 2.0 };",
                "transport spacing")?;
            let c = once(&b,
                "    let bar = Rect::from_min_size(pos2(rect.min.x + 14.0, settings_y + 28.0 + source_drag_h), vec2(rect.width() - 28.0, 22.0));",
                "    let scrub_offset = if crate::craftskin_bridge::transport_above_scrub() { 32.0 } else { 0.0 };\n    let bar = Rect::from_min_size(pos2(rect.min.x + 14.0, settings_y + 28.0 + source_drag_h + scrub_offset), vec2(rect.width() - 28.0, 22.0));", "monitor transport position / scrub")?;
            let d = once(&c,
                "    let row3 = Rect::from_min_size(pos2(rect.min.x, zoom_bar.max.y + 4.0), vec2(rect.width(), 32.0));",
                "    let transport_y = if crate::craftskin_bridge::transport_above_scrub() { bar.min.y - 32.0 } else { zoom_bar.max.y + 4.0 };\n    let row3 = Rect::from_min_size(pos2(rect.min.x, transport_y), vec2(rect.width(), 32.0));", "monitor transport row relocation")?;
            once(&d,
                "    let mut x = row.center().x - 15.0 - total.max(TRANSPORT_BUTTON) / 2.0;",
                "    let mut x = crate::craftskin_bridge::transport_start(row, total);", "transport alignment")
        }
        "crates/ui-egui/src/panels/tools.rs" => {
            // Keep the native Tools panel and all its command handlers, icons and flyouts.
            // Only change geometry; no floating Skin button or reserved header row.
            let a = once(src,
                "    let mut y = rect.min.y + 4.0;",
                "    let mut y = rect.min.y + 4.0;\n    let mut x = rect.min.x + 4.0;\n    let horizontal = crate::craftskin_bridge::tools_horizontal();",
                "Tools panel orientation setup")?;
            let b = once(&a,
                "    let r = Rect::from_min_size(pos2(rect.center().x - size / 2.0, y), vec2(size, size));",
                "    let r = Rect::from_min_size(pos2(if horizontal { x } else { rect.center().x - size / 2.0 }, y), vec2(size, size));",
                "Tools panel horizontal positions")?;
            once(&b,
                "    y += size + 4.0;\n    if y > rect.max.y - size {",
                "    if horizontal {\n        x += size + 4.0;\n        if x + size > rect.max.x { x = rect.min.x + 4.0; y += size + 4.0; }\n    } else { y += size + 4.0; }\n    if y > rect.max.y - size {",
                "Tools panel wrapping")
        }
        "crates/ui-egui/src/panels/settings.rs" => once(src,
            "                    rows(app, ui, &mut d, cat.rows);",
            "                    rows(app, ui, &mut d, cat.rows);\n                    if cat.id == \"appearance\" {\n                        ui.add_space(12.0);\n                        ui.separator();\n                        ui.heading(\"CraftSkin\");\n                        if crate::craftskin_bridge::settings_ui(ui) {\n                            let kind = app.tokens.kind;\n                            app.set_theme(ui.ctx(), kind);\n                        }\n                    }",
            "Preferences Appearance integration"),
        _ => Err(format!("Unknown adapter file: {relative}")),
    }
}
pub fn plan(root: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    if !root.join("Cargo.toml").is_file() || !root.join("crates/ui-egui/Cargo.toml").is_file() {
        return Err("Select a separate FilmCraft source checkout, not the installed application".into());
    }
    let mut out = Vec::with_capacity(FILES.len() + 1);
    for relative in FILES {
        let path = root.join(relative);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if bytes.len() > 1024 * 1024 * 2 { return Err(format!("Unusually large source: {}", path.display())); }
        let src = String::from_utf8(bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let modified = patch(relative, &src).map_err(|err| {
            format!("{}\nFailed source file: {}\nFilmCraft checkout: {}\nYou may need to restore an older CraftSkin integration first.",
                err, relative, root.display())
        })?;
        out.push((path, modified.into_bytes()));
    }
    let bridge = root.join(BRIDGE_PATH);
    if bridge.exists() { return Err("craftskin_bridge.rs already exists; will not overwrite".into()); }
    let mut runtime = BRIDGE.to_owned();
    for (marker, preset) in [
        ("__CRAFTSKIN_VEGAS_JSON__", Style::Vegas),
        ("__CRAFTSKIN_RESOLVE_JSON__", Style::Resolve),
        ("__CRAFTSKIN_GRAPHITE_JSON__", Style::Graphite),
    ] {
        let json = serde_json::to_string(&skinpack::preset(preset)).map_err(|e| e.to_string())?;
        runtime = runtime.replace(marker, &format!("{json:?}"));
    }
    if runtime.contains("__CRAFTSKIN_") { return Err("Unresolved embedded preset marker".into()); }
    out.push((bridge, runtime.into_bytes()));
    Ok(out)
}
pub fn install(root: &Path) -> Result<PathBuf> {
    install_batch(root, &plan(root)?)
}
pub fn restore(root: &Path) -> Result<PathBuf> { restore_batch(root) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_bridge_matches_owned_strings_as_str() {
        // Regression test: both helpers return String, while match literals are &str.
        assert!(BRIDGE.contains("match transport_alignment().as_str() {"));
        assert!(BRIDGE.contains("let polygon = match style.as_str() {"));
        assert!(!BRIDGE.contains("match transport_alignment() {"));
        assert!(!BRIDGE.contains("let polygon = match style {"));
    }
    #[test]
    fn appearance_patch_survives_doc_comment_changes() {
        let original = "impl Tokens {\n    pub fn with_appearance(mut self, highlight: Option<[u8; 3]>, accessible_contrast: bool) -> Self {\n        if accessible_contrast { if highlight.is_some() { self.gap = 3.0; } }\n        self\n    }\n    // Completely changed comment and spacing\n    pub fn mono(size: f32) -> FontId { todo!() }\n}";
        let patched = patch("crates/ui-egui/src/theme.rs", original).unwrap();
        assert!(patched.contains("        crate::craftskin_bridge::apply_tokens(self)\n    }"));
        assert!(patched.contains("Completely changed comment"));
        assert!(patched.contains("if accessible_contrast { if highlight.is_some()"));
    }
    #[test]
    fn appearance_patch_rejects_unsupported_return() {
        let original = "pub fn with_appearance(mut self, highlight: Option<[u8; 3]>, accessible_contrast: bool) -> Self {\n        Self::default()\n    }";
        assert!(patch("crates/ui-egui/src/theme.rs", original).is_err());
    }
    #[test]
    fn appearance_patch_rejects_ambiguous_or_repeated_method() {
        let original = "pub fn with_appearance(mut self, highlight: Option<[u8; 3]>, accessible_contrast: bool) -> Self { self }";
        assert!(patch("crates/ui-egui/src/theme.rs", &format!("{original}\n{original}")).is_err());
        assert!(patch("crates/ui-egui/src/theme.rs", original).is_err()); // not standalone
    }
    #[test]
    fn clip_base_patch_is_guarded() {
        let a = patch("crates/ui-egui/src/panels/timeline/clips.rs", "    let base = label_color(app, it.label);").unwrap();
        assert!(a.contains("clip_base_color"));
        assert!(patch("crates/ui-egui/src/panels/timeline/clips.rs", "missing").is_err());
    }
    #[test]
    fn playhead_patch_retains_timing_and_position() {
        let source = "    let header_w = app.ui.timeline.header_w;\n    let content = Rect::from_min_max(\n        // a short head on the tick row, as in Effect Controls, so it leaves the markers and timecodes clear\n        let (top, tip) = (ruler.max.y - 13.0, ruler.max.y);\n        let head = [pos2(px - 5.0, top), pos2(px + 5.0, top), pos2(px + 5.0, tip - 5.0), pos2(px, tip), pos2(px - 5.0, tip - 5.0)];\n        painter.add(egui::Shape::convex_polygon(head.to_vec(), t.playhead, Stroke::NONE));\n        painter.line_segment([pos2(px, tip), pos2(px, content.max.y)], Stroke::new(1.0, t.playhead));";
        let out = patch("crates/ui-egui/src/panels/timeline.rs", source).unwrap();
        assert!(out.contains("paint_playhead(&painter, px"));
    }
    #[test]
    fn tools_panel_patch_preserves_icons_and_commands() {
        let original = "    let mut y = rect.min.y + 4.0;\n    let r = Rect::from_min_size(pos2(rect.center().x - size / 2.0, y), vec2(size, size));\n    icons::paint(ui.painter(), r, current.icon(), t.icon);\n    y += size + 4.0;\n    if y > rect.max.y - size {";
        let out = patch("crates/ui-egui/src/panels/tools.rs", original).unwrap();
        assert!(out.contains("tools_horizontal()"));
        assert!(!out.contains("tools_settings(app, ui, rect)"));
        assert!(out.contains("icons::paint(ui.painter()"));
        assert!(out.contains("x += size + 4.0"));
        assert!(out.contains("let mut y = rect.min.y + 4.0;"));
    }
    #[test]
    fn preferences_injection_is_inside_appearance_page() {
        let fixture = "        fn draw() {\n                    rows(app, ui, &mut d, cat.rows);\n        }";
        let result = patch("crates/ui-egui/src/panels/settings.rs", fixture).unwrap();
        assert!(result.contains("if cat.id == \"appearance\""));
        assert!(result.contains("crate::craftskin_bridge::settings_ui(ui)"));
        assert!(result.contains("app.set_theme(ui.ctx(), kind)"));
    }
    #[test]
    fn rustfmt_whitespace_difference_is_accepted_but_ambiguity_is_rejected() {
        let source = "fn f(){let   mut   y = rect.min.y + 4.0;\n}";
        let result = once(source, "let mut y = rect.min.y + 4.0;", "let y = 2.0;", "test").unwrap();
        assert!(result.contains("let y = 2.0;"));
        assert!(once("let a=1;let a=1;", "let a = 1;", "let b = 2;", "ambiguous").is_err());
    }
    #[test]
    fn exact_anchor_refuses_mismatches() {
        assert!(patch("crates/ui-egui/src/header.rs", "not FilmCraft").is_err());
        assert!(once("AA AA", "AA", "BB", "unique").is_err());
    }
    #[test]
    fn swaps_timeline_header_rows_without_changing_commands() {
        let sample = "    let tc_rect = Rect::from_min_size(pos2(rect.min.x + 14.0, rect.min.y + 4.0), vec2(hw - 20.0, 20.0));\n    let mut x = rect.min.x + 12.0;\n    let y = rect.min.y + 26.0;\n    for (icon, key, on, toggle, tip) in toggles {\n        x += 30.0;";
        let result = patch("crates/ui-egui/src/panels/timeline/ruler.rs", sample).unwrap();
        assert!(result.contains("timeline_toolbar_above()"));
        assert!(result.contains("toggles.reverse()"));
        assert!(result.contains("for (icon, key, on, toggle, tip) in toggles"));
    }
    #[test]
    fn patches_component_positions_without_removing_actions() {
        let original = "before\n    buttons.retain(|(_, command, _)| transport_visible(&hidden, command));\n    let gap = if count * TRANSPORT_BUTTON + (count - 1.0).max(0.0) * 6.0 <= available { 6.0 } else { 2.0 };\n    let bar = Rect::from_min_size(pos2(rect.min.x + 14.0, settings_y + 28.0 + source_drag_h), vec2(rect.width() - 28.0, 22.0));\n    let row3 = Rect::from_min_size(pos2(rect.min.x, zoom_bar.max.y + 4.0), vec2(rect.width(), 32.0));\n    let mut x = row.center().x - 15.0 - total.max(TRANSPORT_BUTTON) / 2.0;\nafter";
        let patched = patch("crates/ui-egui/src/panels/monitor.rs", original).unwrap();
        assert!(patched.contains("sort_by_key"));
        assert!(patched.contains("transport_start"));
        assert_eq!(patched.matches("buttons.retain").count(), 1);
    }
}
