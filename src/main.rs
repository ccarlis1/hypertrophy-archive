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
