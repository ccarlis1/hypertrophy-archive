mod models;
mod gui;


fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        ..Default::default()
    };
    
    eframe::run_native(
        "Hypertrophy Archive",
        options,
        Box::new(|_cc| Box::new(gui::HypertrophyApp::default())),
    )
}
