//! CraftSkin runtime bridge for an opt-in, independently built FilmCraft.
//! Installed by CraftSkin. FilmCraft still owns all actual editing and UI commands.
//! No external packages beyond FilmCraft's existing egui and serde_json dependencies.
use std::sync::{Mutex, OnceLock};
use std::path::PathBuf;
use serde_json::Value;
use egui::Color32;

// The injector replaces these literals before copying this module into FilmCraft.
const VEGAS_JSON: &str = __CRAFTSKIN_VEGAS_JSON__;
const RESOLVE_JSON: &str = __CRAFTSKIN_RESOLVE_JSON__;
const GRAPHITE_JSON: &str = __CRAFTSKIN_GRAPHITE_JSON__;
use crate::theme::{Tokens, ThemeKind};

fn config_path() -> PathBuf {
    if let Some(path) = std::env::var_os("CRAFTSKIN_CONFIG") { return PathBuf::from(path); }
    #[cfg(target_os = "windows")]
    { return std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default().join("CraftSkin/filmcraft.json"); }
    #[cfg(target_os = "macos")]
    { return std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join("Library/Application Support/CraftSkin/filmcraft.json"); }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
            .unwrap_or_else(|| std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join(".config"))
            .join("craftskin/filmcraft.json")
    }
}
// The profile is mutable at runtime. Changes made in the editor are written to disk.
// A poisoned lock does not crash the editor: recover the last available value.
fn state() -> &'static Mutex<Value> {
    static STATE: OnceLock<Mutex<Value>> = OnceLock::new();
    STATE.get_or_init(|| {
        let saved = std::fs::read(config_path()).ok()
            .filter(|bytes| bytes.len() <= 65536)
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .filter(|value| value.get("format_version").and_then(Value::as_u64) == Some(1)
                && value.get("target").and_then(Value::as_str) == Some("filmcraft"));
        Mutex::new(saved.unwrap_or_else(|| builtin("VEGAS Flow")))
    })
}
fn skin() -> Value {
    state().lock().unwrap_or_else(|error| error.into_inner()).clone()
}
fn builtin(name: &str) -> Value {
    let raw = match name {
        "Resolve Flow" => RESOLVE_JSON,
        "Graphite Studio" => GRAPHITE_JSON,
        _ => VEGAS_JSON,
    };
    serde_json::from_str(raw).unwrap_or(Value::Null)
}
fn write_profile(value: Value) -> Result<(), String> {
    let path = config_path();
    let json = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
    let parent = path.parent().ok_or("CraftSkin configuration path has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    // Atomic on the same filesystem. Failed writes leave the previous profile intact.
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&tmp, &json).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    *state().lock().unwrap_or_else(|error| error.into_inner()) = value;
    Ok(())
}
fn get_path<'a>(json: &'a Value, section: &str, key: &str) -> Option<&'a Value> {
    json.get(section)?.get(key)
}
fn set_field(json: &mut Value, section: &str, key: &str, value: Value) {
    if let Some(object) = json.get_mut(section).and_then(Value::as_object_mut) {
        object.insert(key.to_owned(), value);
    }
}
fn color(hex: &str) -> Option<Color32> {
    let b = hex.as_bytes();
    if b.len() != 7 || b[0] != b'#' { return None; }
    let parse = |a: usize| u8::from_str_radix(hex.get(a..a+2)?, 16).ok();
    Some(Color32::from_rgb(parse(1)?, parse(3)?, parse(5)?))
}
fn number(section: &str, key: &str, fallback: f32, lo: f32, hi: f32) -> f32 {
    skin().get(section).and_then(|o| o.get(key)).and_then(Value::as_f64)
        .filter(|n| n.is_finite() && *n >= lo as f64 && *n <= hi as f64)
        .map(|n| n as f32).unwrap_or(fallback)
}
fn choice(section: &str, key: &str) -> String {
    skin().get(section).and_then(|o| o.get(key)).and_then(Value::as_str).unwrap_or("").to_owned()
}

/// Runs after FilmCraft's Appearance preferences, keeping foreground contrast, etc.
pub fn apply_tokens(mut t: Tokens) -> Tokens {
    // Don't turn a light-accessibility theme into an unreadable dark skin.
    if t.kind == ThemeKind::Light { return t; }
    let profile = skin();
    let Some(colors) = profile.get("colors") else { return t; };
    macro_rules! set {
        ($($field:ident),* $(,)?) => {$({
            if let Some(c) = colors.get(stringify!($field)).and_then(Value::as_str).and_then(color) { t.$field = c; }
        })*};
    }
    set!(app_bg, header_bg, panel_bg, tab_bg, tab_text, tab_text_active, focus,
        accent, accent_hover, text, text_dim, text_faint, icon, icon_active, hover,
        pressed, field_bg, field_border, separator, row_alt, row_selected,
        tl_bg, tl_track_bg, tl_track_bg_alt, tl_header_bg, tl_ruler_bg,
        tl_ruler_tick, tl_ruler_text, playhead, track_target, monitor_bg,
        timecode, button_hover, button_border, scroll_thumb, scroll_thumb_hover,
        clip_selected_border, in_out_shade, hot_text, pill_active_bg,
        control_surface, control_well, plot_bg, plot_grid,
        keyframe_plot_bg, keyframe_plot_grid, meter_readout_bg, caption_track_bg);
    t.radius = number("geometry", "panel_radius", t.radius, 2.0, 14.0);
    t.radius_sm = number("geometry", "button_radius", t.radius_sm, 2.0, 12.0);
    t.gap = number("geometry", "panel_gap", t.gap, 1.0, 12.0);
    t
}
pub fn header_mode_gap() -> f32 { number("geometry", "header_mode_gap", 24.0, 12.0, 35.0) }
pub fn timeline_header_width(current: f32, canvas: f32) -> f32 {
    // Retain screen room for editing; presets aren't absolute pixel geometry.
    number("geometry", "track_header_width", current, 150.0, 320.0)
        .min((canvas - 250.0).max(150.0))
}
pub fn timeline_tool_gap() -> f32 { number("geometry", "timeline_tool_gap", 2.0, 0.0, 10.0) }
pub fn timeline_tool_order() -> String { choice("controls", "timeline_toolbar") }
pub fn timeline_toolbar_above() -> bool { choice("controls", "timeline_toolbar_position") == "above_timecode" }
pub fn transport_alignment() -> String { choice("controls", "transport_align") }
pub fn transport_above_scrub() -> bool { choice("controls", "transport_position") == "above_scrub" }
pub fn transport_gap() -> f32 { number("geometry", "transport_gap", 6.0, 0.0, 12.0) }
pub fn transport_order(command: &str) -> usize {
    skin().get("controls").and_then(|v| v.get("transport_order")).and_then(Value::as_array)
        .and_then(|a| a.iter().position(|item| item.as_str() == Some(command)))
        .unwrap_or(usize::MAX)
}
pub fn transport_start(row: egui::Rect, total: f32) -> f32 {
    // `choice()` returns an owned String; match its string slice, not the String.
    match transport_alignment().as_str() {
        "left" => row.left() + 10.0,
        "right" => (row.right() - 36.0 - total).max(row.left() + 6.0),
        _ => row.center().x - 15.0 - total.max(28.0) / 2.0,
    }
}

/// Blend the original per-clip label hue with the profile's video/audio color.
/// `clip_mix = 0` retains original labels; 1 applies the profile's uniform clip hue.
/// Disabled and selected clip shading is still controlled by FilmCraft.
pub fn clip_base_color(native: Color32, is_video: bool) -> Color32 {
    let field = if is_video { "clip_video" } else { "clip_audio" };
    let Some(target) = skin().pointer(&format!("/colors/{field}")).and_then(Value::as_str).and_then(color) else { return native; };
    let amount = number("geometry", "clip_mix", 0.0, 0.0, 1.0);
    let lerp = |old: u8, new: u8| -> u8 { (old as f32 * (1.0 - amount) + new as f32 * amount).round().clamp(0.0, 255.0) as u8 };
    Color32::from_rgb(lerp(native.r(), target.r()), lerp(native.g(), target.g()), lerp(native.b(), target.b()))
}

/// Draw only the playhead decoration. The FilmCraft session still controls its real
/// position, timing, commands, snapping and hit testing. The native shape is preserved
/// when `playhead_style` is absent or unsupported.
pub fn paint_playhead(p: &egui::Painter, x: f32, ruler_y: f32, bottom: f32, tint: Color32) {
    use egui::{pos2, Stroke};
    let style = choice("controls", "playhead_style");
    let width = number("geometry", "playhead_width", 1.0, 0.7, 3.5);
    // `choice()` returns an owned String; match its string slice, not the String.
    let polygon = match style.as_str() {
        "triangle" => vec![pos2(x - 6.5, ruler_y - 12.0), pos2(x + 6.5, ruler_y - 12.0), pos2(x, ruler_y)],
        "flag" => vec![pos2(x - 4.0, ruler_y - 14.0), pos2(x + 5.0, ruler_y - 14.0), pos2(x + 5.0, ruler_y - 4.0), pos2(x, ruler_y)],
        "diamond" => vec![pos2(x, ruler_y - 15.0), pos2(x + 6.0, ruler_y - 9.0), pos2(x, ruler_y - 3.0), pos2(x - 6.0, ruler_y - 9.0)],
        _ => vec![pos2(x - 5.0, ruler_y - 13.0), pos2(x + 5.0, ruler_y - 13.0), pos2(x + 5.0, ruler_y - 5.0), pos2(x, ruler_y), pos2(x - 5.0, ruler_y - 5.0)],
    };
    p.add(egui::Shape::convex_polygon(polygon, tint, Stroke::NONE));
    p.line_segment([pos2(x, ruler_y), pos2(x, bottom)], Stroke::new(width, tint));
}


/// FilmCraft owns the Tools panel; this only selects its layout geometry.
pub fn tools_horizontal() -> bool {
    choice("controls", "tools_orientation") == "horizontal"
}

fn edit_choice(ui: &mut egui::Ui, profile: &mut Value, label: &str,
               section: &str, key: &str, values: &[&str]) -> bool {
    let mut current = get_path(profile, section, key).and_then(Value::as_str)
        .unwrap_or(values[0]).to_owned();
    let before = current.clone();
    egui::ComboBox::from_label(label).selected_text(&current).show_ui(ui, |ui| {
        for value in values { ui.selectable_value(&mut current, (*value).to_owned(), *value); }
    });
    if current != before {
        set_field(profile, section, key, Value::String(current));
        true
    } else { false }
}
fn edit_number(ui: &mut egui::Ui, profile: &mut Value, label: &str,
               key: &str, lo: f32, hi: f32) -> bool {
    let mut v = get_path(profile, "geometry", key).and_then(Value::as_f64)
        .unwrap_or(lo as f64) as f32;
    let change = ui.add(egui::Slider::new(&mut v, lo..=hi).text(label)).changed();
    if change { set_field(profile, "geometry", key, serde_json::json!(v)); }
    change
}
fn edit_color(ui: &mut egui::Ui, profile: &mut Value, key: &str) -> bool {
    let before = get_path(profile, "colors", key).and_then(Value::as_str).unwrap_or("#888888");
    let Some(c) = color(before) else { return false; };
    let mut rgb = [c.r(), c.g(), c.b()];
    let mut change = false;
    ui.horizontal(|ui| {
        change = ui.color_edit_button_srgb(&mut rgb).changed();
        ui.label(key.replace('_', " "));
    });
    if change {
        set_field(profile, "colors", key,
            Value::String(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])));
    }
    change
}

/// Render the theme editor in FilmCraft itself. This is NOT part of the CraftSkin injector UI.
/// Returns true after a successful profile update, which triggers `app.set_theme`.
pub fn settings_ui(ui: &mut egui::Ui) -> bool {
    let mut value = skin();
    let mut dirty = false;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("VEGAS Flow").to_owned();
    ui.label("CraftSkin changes apply and save immediately, including when Preferences is canceled.");
    ui.horizontal_wrapped(|ui| {
        for preset in ["VEGAS Flow", "Resolve Flow", "Graphite Studio"] {
            if ui.selectable_label(name == preset, preset).clicked() {
                let keep_orientation = choice("controls", "tools_orientation");
                value = builtin(preset);
                if keep_orientation == "horizontal" {
                    set_field(&mut value, "controls", "tools_orientation", Value::String(keep_orientation));
                }
                dirty = true;
            }
        }
    });
    ui.separator();
    ui.heading("Panel and toolbar arrangement");
    dirty |= edit_choice(ui, &mut value, "Tools panel", "controls", "tools_orientation", &["vertical", "horizontal"]);
    dirty |= edit_choice(ui, &mut value, "Playhead", "controls", "playhead_style", &["native", "triangle", "flag", "diamond"]);
    dirty |= edit_choice(ui, &mut value, "Monitor controls", "controls", "transport_align", &["left", "center", "right"]);
    dirty |= edit_choice(ui, &mut value, "Monitor transport", "controls", "transport_position", &["below_scrub", "above_scrub"]);
    dirty |= edit_choice(ui, &mut value, "Timeline toolbar", "controls", "timeline_toolbar_position", &["below_timecode", "above_timecode"]);
    dirty |= edit_choice(ui, &mut value, "Toolbar order", "controls", "timeline_toolbar", &["normal", "reverse"]);
    ui.separator();
    ui.heading("Geometry");
    dirty |= edit_number(ui, &mut value, "Track header width", "track_header_width", 150.0, 320.0);
    dirty |= edit_number(ui, &mut value, "Panel corner radius", "panel_radius", 2.0, 14.0);
    dirty |= edit_number(ui, &mut value, "Button corner radius", "button_radius", 2.0, 12.0);
    dirty |= edit_number(ui, &mut value, "Panel spacing", "panel_gap", 1.0, 12.0);
    dirty |= edit_number(ui, &mut value, "Clip palette strength", "clip_mix", 0.0, 1.0);
    dirty |= edit_number(ui, &mut value, "Playhead width", "playhead_width", 0.7, 3.5);
    dirty |= edit_number(ui, &mut value, "Transport gap", "transport_gap", 0.0, 12.0);
    dirty |= edit_number(ui, &mut value, "Timeline toolbar gap", "timeline_tool_gap", 0.0, 10.0);
    dirty |= edit_number(ui, &mut value, "Header mode gap", "header_mode_gap", 12.0, 35.0);
    ui.separator();
    ui.heading("Palette");
    let keys: Vec<String> = value.get("colors").and_then(Value::as_object)
        .map(|o| o.keys().cloned().collect()).unwrap_or_default();
    for key in keys { dirty |= edit_color(ui, &mut value, &key); }
    if dirty {
        match write_profile(value) {
            Ok(()) => return true,
            Err(error) => { ui.colored_label(egui::Color32::RED, format!("Could not save CraftSkin: {error}")); }
        }
    }
    false
}

