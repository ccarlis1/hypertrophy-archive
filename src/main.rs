mod models;

use models::*;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

fn main() -> io::Result<()> {
    // Create a sample exercise
    let flat_chest_press = Exercise {
        name: "flat_chest_press".to_string(),
        id: 0,
        r#type: ExerciseType::Compound,
        description: "A press in the frontal plane.".to_string(),
        target_muscles: TargetMuscles {
            muscle_name: "chest".to_string(),
            muscle_divisions: vec![
                MuscleDivision {
                    name: "sternocostal".to_string(),
                    active: true,
                },
                MuscleDivision {
                    name: "clavicular".to_string(),
                    active: true,
                },
                MuscleDivision {
                    name: "abdominal".to_string(),
                    active: true,
                },
            ],
        },
        joints_involved: JointsInvolved {
            joints: vec![
                Joint {
                    name: "shoulder".to_string(),
                    dynamic: false,
                    angle: Some(45),
                    intitial_angle: None,
                    final_angle: None,
                },
                Joint {
                    name: "elbow".to_string(),
                    dynamic: true,
                    angle: None,
                    intitial_angle: Some(30),
                    final_angle: Some(180),
                },
            ],
        },
        resistance_profile: ResistanceProfile::Descending,
        plane_of_motion: PlaneOfMotion::Transverse,
        tips: "Keep those shoulders tucked!".to_string(),
        technique_video: "https://youtube.com/shorts/hWbUlkb5Ms4?si=P89i2PXyGlX_q7XE".to_string(),
    };

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&flat_chest_press)?;
    
    // Ensure the data directory exists
    let data_dir = "data";
    if !Path::new(data_dir).exists() {
        fs::create_dir(data_dir)?;
    }
    
    // Save to file
    let mut file = File::create(format!("{}/flat_chest_press.json", data_dir))?;
    file.write_all(json.as_bytes())?;
    
    println!("Exercise saved to data/flat_chest_press.json");
    
    // Read from file
    let mut file = File::open(format!("{}/flat_chest_press.json", data_dir))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    // Deserialize from JSON
    let loaded_exercise: Exercise = serde_json::from_str(&contents)?;
    println!("Loaded exercise: {}", loaded_exercise.name);
    
    Ok(())
} 