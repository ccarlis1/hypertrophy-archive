mod models;
mod gui;

use models::*;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

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

fn load_exercise(file_path: &str) -> io::Result<Exercise> {
    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(serde_json::from_str(&contents)?)
}
