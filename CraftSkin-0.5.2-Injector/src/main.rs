#![recursion_limit = "512"]

mod install;
mod adapter;
mod skinpack;
mod presets;
mod ui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 790.0])
            .with_min_inner_size([860.0, 610.0]),
        ..Default::default()
    };
    eframe::run_native("CraftSkin", options, Box::new(|_| Ok(Box::new(ui::CraftSkin::default()))))
}
