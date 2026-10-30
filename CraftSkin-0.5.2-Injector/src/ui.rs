//! The standalone CraftSkin application is strictly a source integrator.
//! All theme editing lives inside the FilmCraft build that receives the bridge.
use crate::adapter;
use eframe::egui;
use std::path::PathBuf;

pub struct CraftSkin {
    source_dir: String,
    status: String,
    confirm: bool,
}
impl Default for CraftSkin {
    fn default() -> Self {
        Self {
            source_dir: String::new(),
            status: "Choose a clean, separate FilmCraft source checkout.".to_owned(),
            confirm: false,
        }
    }
}
impl eframe::App for CraftSkin {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("CraftSkin 0.5.2 — Source Injector");
            ui.label("Installs the in-editor theme system and horizontal Tools panel into FilmCraft's Rust source.");
            ui.add_space(12.0);
            ui.strong("FilmCraft source checkout:");
            ui.text_edit_singleline(&mut self.source_dir);
            ui.add_space(7.0);
            ui.checkbox(&mut self.confirm,
                "I have a backup and understand this modifies the selected source checkout.");
            ui.horizontal(|ui| {
                if ui.add_enabled(self.confirm && !self.source_dir.trim().is_empty(),
                        egui::Button::new("Inject CraftSkin into source")).clicked() {
                    let root = PathBuf::from(self.source_dir.trim());
                    self.status = match adapter::install(&root) {
                        Ok(manifest) => format!("Installed source integration. Backup: {}\nBuild FilmCraft with cargo build --release -p filmcraft.", manifest.display()),
                        Err(error) => format!("No integration installed: {error}"),
                    };
                }
                if ui.add_enabled(self.confirm && !self.source_dir.trim().is_empty(),
                        egui::Button::new("Restore previous source")).clicked() {
                    let root = PathBuf::from(self.source_dir.trim());
                    self.status = match adapter::restore(&root) {
                        Ok(dir) => format!("Restored original source from {}", dir.display()),
                        Err(error) => format!("Restore blocked: {error}"),
                    };
                }
            });
            ui.separator();
            ui.label(&self.status);
            ui.add_space(14.0);
            ui.heading("After compiling FilmCraft");
            ui.label("Open Edit → Preferences → Appearance and select VEGAS Flow, Resolve Flow or Graphite Studio.");
            ui.label("Change panel orientation, playhead, colors and geometry in FilmCraft Preferences.");
            ui.label("Profile changes are saved automatically; the official ArtCraft installation is not modified.");
            ui.add_space(6.0);
            ui.small("Installer checks each anchor, reports the exact source file on mismatch, and rejects unfamiliar revisions.");
        });
    }
}
