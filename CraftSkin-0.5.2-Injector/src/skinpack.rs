//! Portable, user-editable JSON skins. Complete editing functionality stays in FilmCraft.
use crate::presets::Style;
use serde_json::{json, Value};

pub fn preset(style: Style) -> Value {
    let p = style.palette();
    let (toolgap, transport_align, transport_position, order, toolbar, tw, playhead_style,
         playhead_color, playhead_width, video_clip, audio_clip, clip_mix, radius) = match style {
        Style::Vegas => (2., "left", "below_scrub", vec!["playback.toggle", "playhead.stepBack", "playhead.stepForward", "markers.add", "markers.markIn", "markers.markOut"], "normal", 184., "flag", "#E7E7E7", 1.2, "#4F8396", "#8B7595", 0.76, 6.),
        Style::Resolve => (1., "center", "above_scrub", vec!["playhead.stepBack", "playback.toggle", "playhead.stepForward", "markers.goToIn", "markers.goToOut"], "normal", 212., "triangle", "#E1494D", 1.5, "#417C9F", "#4B8867", 0.90, 6.),
        Style::Graphite => (3., "center", "below_scrub", vec!["playback.toggle", "playhead.stepBack", "playhead.stepForward"], "reverse", 194., "diamond", "#B4A0EE", 1.8, "#576F97", "#507D77", 0.72, 9.),
    };
    json!({
        "format_version": 1,
        "name": style.title(),
        "target": "filmcraft",
        "colors": {
            "app_bg": p.app, "header_bg": p.header, "panel_bg": p.panel,
            "tab_bg": p.tab, "tab_text": p.muted, "tab_text_active": p.text,
            "focus": p.accent, "accent": p.accent, "accent_hover": p.accent,
            "text": p.text, "text_dim": p.muted, "text_faint": p.muted,
            "icon": p.muted, "icon_active": p.text, "hover": p.hover,
            "pressed": p.hover, "field_bg": p.timeline, "field_border": p.tab,
            "separator": p.tab, "row_alt": p.tab, "row_selected": p.hover,
            "tl_bg": p.timeline, "tl_track_bg": p.track, "tl_track_bg_alt": p.timeline,
            "tl_header_bg": p.panel, "tl_ruler_bg": p.timeline,
            "tl_ruler_tick": p.muted, "tl_ruler_text": p.muted,
            "playhead": playhead_color, "track_target": p.accent,
            "clip_video": video_clip, "clip_audio": audio_clip,
            "clip_selected_border": p.accent, "in_out_shade": p.hover,
            "hot_text": p.accent, "pill_active_bg": p.hover,
            "control_surface": p.panel, "control_well": p.timeline,
            "plot_bg": p.timeline, "plot_grid": p.tab,
            "keyframe_plot_bg": p.timeline, "keyframe_plot_grid": p.tab,
            "meter_readout_bg": p.timeline, "caption_track_bg": p.track,
            "monitor_bg": p.timeline, "timecode": p.accent,
            "button_hover": p.hover, "button_border": p.tab,
            "scroll_thumb": p.muted, "scroll_thumb_hover": p.text
        },
        "geometry": { "panel_radius": radius, "button_radius": 5.0, "panel_gap": 3.0,
            "header_mode_gap": 19.0, "track_header_width": tw,
            "timeline_tool_gap": toolgap, "transport_gap": 5.0,
            "playhead_width": playhead_width, "clip_mix": clip_mix },
        "controls": { "playhead_style": playhead_style, "timeline_toolbar": toolbar, "timeline_toolbar_position": if style == Style::Resolve { "above_timecode" } else { "below_timecode" }, "transport_align": transport_align, "transport_position": transport_position, "transport_order": order }
    })
}
pub fn validate(profile: &Value) -> Result<(), String> {
    if profile.get("format_version").and_then(Value::as_u64) != Some(1) { return Err("Unsupported theme pack format".into()); }
    if profile.get("target").and_then(Value::as_str) != Some("filmcraft") { return Err("Theme pack targets a different app".into()); }
    let colors = profile.get("colors").and_then(Value::as_object).ok_or("Missing colors")?;
    if colors.len() > 80 { return Err("Too many color fields".into()); }
    for (k, v) in colors {
        let Some(s) = v.as_str() else { return Err(format!("Invalid color: {k}")); };
        if s.len() != 7 || !s.starts_with('#') || !s[1..].chars().all(|ch| ch.is_ascii_hexdigit()) { return Err(format!("Invalid hex color for {k}")); }
    }
    for (k, min, max) in [
        ("panel_radius", 2., 14.), ("button_radius", 2., 12.), ("panel_gap", 1., 12.),
        ("header_mode_gap", 12., 35.), ("track_header_width", 150., 320.),
        ("timeline_tool_gap", 0., 10.), ("transport_gap", 0., 12.),
        ("playhead_width", 0.7, 3.5), ("clip_mix", 0., 1.),
    ] {
        if let Some(v) = profile.pointer(&format!("/geometry/{k}")) {
            let n = v.as_f64().ok_or(format!("Invalid geometry value: {k}"))?;
            if !n.is_finite() || n < min || n > max { return Err(format!("Out of range: {k}")); }
        }
    }
    let head = profile.pointer("/controls/playhead_style").and_then(Value::as_str).unwrap_or("native");
    if !["native", "flag", "triangle", "diamond"].contains(&head) { return Err("Unknown playhead style".into()); }
    let alignment = profile.pointer("/controls/transport_align").and_then(Value::as_str).unwrap_or("center");
    if !["center", "left", "right"].contains(&alignment) { return Err("Unknown transport alignment".into()); }
    let position = profile.pointer("/controls/transport_position").and_then(Value::as_str).unwrap_or("below_scrub");
    if !["above_scrub", "below_scrub"].contains(&position) { return Err("Unknown transport position".into()); }
    let timeline_position = profile.pointer("/controls/timeline_toolbar_position").and_then(Value::as_str).unwrap_or("below_timecode");
    if !["above_timecode", "below_timecode"].contains(&timeline_position) { return Err("Unknown timeline toolbar position".into()); }
    let toolbar = profile.pointer("/controls/timeline_toolbar").and_then(Value::as_str).unwrap_or("normal");
    if !["normal", "reverse"].contains(&toolbar) { return Err("Unknown toolbar order".into()); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn builtin_packs_validate() { for style in Style::ALL { validate(&preset(style)).unwrap(); } }
    #[test] fn theme_profiles_have_distinct_timeline_colors() {
        let v = preset(Style::Vegas);
        let r = preset(Style::Resolve);
        let g = preset(Style::Graphite);
        assert_ne!(v["colors"]["playhead"], r["colors"]["playhead"]);
        assert_ne!(r["colors"]["clip_audio"], g["colors"]["clip_audio"]);
        assert_eq!(r["controls"]["playhead_style"], "triangle");
    }
    #[test] fn unknown_playhead_is_rejected() {
        let mut v = preset(Style::Resolve);
        v["controls"]["playhead_style"] = json!("unknown");
        assert!(validate(&v).is_err());
    }
    #[test] fn malformed_skin_fails() { let mut v = preset(Style::Vegas); v["colors"]["accent"] = json!("purple"); assert!(validate(&v).is_err()); }
}
