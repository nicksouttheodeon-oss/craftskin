# CraftSkin JSON v1 — fidelity additions (0.4)

The runtime skin is loaded when a **custom FilmCraft executable** starts. No art assets, fonts or original app icons are replaced. The UI uses FilmCraft's existing Rust icons and rounded controls.

New color keys (all optional):

- `clip_video`, `clip_audio`: overlay hues for video and audio clips.
- `clip_selected_border`, `in_out_shade`, `hot_text`, `pill_active_bg`, `control_surface`, `control_well`, `plot_bg`, `plot_grid`, `keyframe_plot_bg`, `keyframe_plot_grid`, `meter_readout_bg`, `caption_track_bg`: finer application surfaces.

New geometry:

- `playhead_width`: line width, 0.7–3.5 points.
- `clip_mix`: blend of current FilmCraft per-clip label color and theme clip hue, from 0 (retain original label) to 1 (replace hue entirely). The default for an absent field is 0.

New control:

- `playhead_style`: `native`, `flag` (VEGAS Flow), `triangle` (Resolve Flow), or `diamond` (Graphite Studio).

For full examples see `themes/*.json`. Profile format version remains 1; the v0.3.2 bridge ignores unknown keys, so the visual additions only activate in the **v0.4-integrated custom FilmCraft**.

The theme is inspired by broad visual conventions, not sourced art. The playhead is a render-only decoration. Playhead timestamps, hit testing, snapping and editor commands remain FilmCraft's.

## Compatibility limitations

FilmCraft's native interface uses more than a hundred design tokens and multiple independently authored panels. This version adds material improvements to the timeline and overall surfaces but does **not** produce pixel-identical reproductions of VEGAS Pro or DaVinci Resolve. Detailed panel internals, clip thumbnails, track controls and toolbars outside the supported integration points remain FilmCraft's own.

## Upgrade from CraftSkin 0.3.2

1. In CraftSkin 0.3.2, use **Restore integrated FilmCraft source** on your custom source checkout; it checks hashes and will refuse to overwrite unrelated changes. Leave the official ArtCraft installation untouched.
2. Build and launch CraftSkin 0.4, select the **same unpatched checkout**, and click **Integrate component adapter**. If exact anchors no longer match, stop and report the named error.
3. Build your custom FilmCraft with `cargo build --release -p filmcraft`.
4. In CraftSkin 0.4, **Apply complete selected theme**, selecting the custom FilmCraft data directory.
5. Restart only your **custom** FilmCraft executable. If a workspace is not active, choose it in Window > Workspaces.

If you have local changes beyond CraftSkin to FilmCraft's source, first commit or save those changes; restoration may reject them. Switching presets later needs only a new JSON skin file and FilmCraft restart. However adding new adapter capabilities requires rebuilding the custom executable.
