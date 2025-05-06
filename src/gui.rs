use crate::models::*;
use eframe::egui;
use egui::{Color32, RichText, Ui, InnerResponse};
use std::collections::HashMap;
use std::path::PathBuf;

// Add this trait definition before the implementation
trait UiExt {
    fn horizontal_centered<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
}

impl UiExt for Ui {
    fn horizontal_centered<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        self.with_layout(
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            add_contents
        )
    }
}

pub struct HypertrophyApp {
    exercise: Exercise,
    status_message: String,
    data_dir: PathBuf,
    muscle_divisions: HashMap<String, Vec<String>>,
    joint_names: Vec<String>,
    current_tab: Tab,
    show_save_dialog: bool,
    save_filename: String,
}

enum Tab {
    BasicInfo,
    TargetMuscles,
    JointsInvolved,
    AdditionalInfo,
    Preview,
}

impl Default for HypertrophyApp {
    fn default() -> Self {
        let exercise = Exercise {
            name: String::new(),
            id: 0,
            r#type: ExerciseType::Compound,
            description: String::new(),
            target_muscles: TargetMuscles {
                muscle_name: String::new(),
                muscle_divisions: Vec::new(),
            },
            joints_involved: JointsInvolved { joints: Vec::new() },
            resistance_profile: ResistanceProfile::Constant,
            plane_of_motion: PlaneOfMotion::Transverse,
            tips: String::new(),
            technique_video: String::new(),
        };

        // Predefined muscle divisions
        let mut muscle_divisions = HashMap::new();
        muscle_divisions.insert(
            "chest".to_string(),
            vec![
                "sternocostal".to_string(),
                "clavicular".to_string(),
                "abdominal".to_string(),
            ],
        );
        muscle_divisions.insert(
            "back".to_string(),
            vec![
                "upper trapezius".to_string(),
                "middle trapezius".to_string(),
                "lower trapezius".to_string(),
                "rhomboids".to_string(),
                "latissimus dorsi".to_string(),
            ],
        );
        muscle_divisions.insert(
            "shoulders".to_string(),
            vec![
                "anterior deltoid".to_string(),
                "lateral deltoid".to_string(),
                "posterior deltoid".to_string(),
            ],
        );
        muscle_divisions.insert(
            "biceps".to_string(),
            vec![
                "long head".to_string(),
                "short head".to_string(),
            ],
        );
        muscle_divisions.insert(
            "triceps".to_string(),
            vec![
                "long head".to_string(),
                "lateral head".to_string(),
                "medial head".to_string(),
            ],
        );
        muscle_divisions.insert(
            "quadriceps".to_string(),
            vec![
                "rectus femoris".to_string(),
                "vastus lateralis".to_string(),
                "vastus medialis".to_string(),
                "vastus intermedius".to_string(),
            ],
        );
        muscle_divisions.insert(
            "hamstrings".to_string(),
            vec![
                "biceps femoris".to_string(),
                "semitendinosus".to_string(),
                "semimembranosus".to_string(),
            ],
        );

        // Joint names
        let joint_names = vec![
            "shoulder".to_string(),
            "elbow".to_string(),
            "wrist".to_string(),
            "hip".to_string(),
            "knee".to_string(),
            "ankle".to_string(),
            "spine".to_string(),
        ];

        Self {
            exercise,
            status_message: "Ready to create a new exercise".to_string(),
            data_dir: PathBuf::from("data"),
            muscle_divisions,
            joint_names,
            current_tab: Tab::BasicInfo,
            show_save_dialog: false,
            save_filename: String::new(),
        }
    }
}

impl eframe::App for HypertrophyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Set global text scaling
        let mut style = (*ctx.style()).clone();
        style.text_styles.get_mut(&egui::TextStyle::Body).unwrap().size = 16.0;
        style.text_styles.get_mut(&egui::TextStyle::Button).unwrap().size = 18.0;
        style.text_styles.get_mut(&egui::TextStyle::Heading).unwrap().size = 24.0;
        ctx.set_style(style);
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(5.0);
                ui.heading(RichText::new("Hypertrophy Archive").size(32.0).color(Color32::from_rgb(120, 200, 255)));
                ui.add_space(5.0);
            });

            ui.add_space(-475.0);

            ui.horizontal_centered(|ui| {
                ui.add_space(20.0);
                let button_size = egui::vec2(140.0, 40.0);
                
                if ui.add_sized(button_size, self.tab_button_styled("Basic Info", Tab::BasicInfo)).clicked() {
                    self.current_tab = Tab::BasicInfo;
                }
                if ui.add_sized(button_size, self.tab_button_styled("Target Muscles", Tab::TargetMuscles)).clicked() {
                    self.current_tab = Tab::TargetMuscles;
                }
                if ui.add_sized(button_size, self.tab_button_styled("Joints Involved", Tab::JointsInvolved)).clicked() {
                    self.current_tab = Tab::JointsInvolved;
                }
                if ui.add_sized(button_size, self.tab_button_styled("Additional Info", Tab::AdditionalInfo)).clicked() {
                    self.current_tab = Tab::AdditionalInfo;
                }
                if ui.add_sized(button_size, self.tab_button_styled("Preview", Tab::Preview)).clicked() {
                    self.current_tab = Tab::Preview;
                }
                ui.add_space(20.0);
            });

            ui.add_space(15.0);
            ui.separator();
            ui.add_space(15.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    egui::Frame::none()
                        .inner_margin(egui::style::Margin::symmetric(40.0, 10.0))
                        .show(ui, |ui| {
                            // Tab content
                            match self.current_tab {
                                Tab::BasicInfo => self.show_basic_info_tab(ui),
                                Tab::TargetMuscles => self.show_target_muscles_tab(ui),
                                Tab::JointsInvolved => self.show_joints_involved_tab(ui),
                                Tab::AdditionalInfo => self.show_additional_info_tab(ui),
                                Tab::Preview => self.show_preview_tab(ui),
                            }
                        });
                });

            // Status bar
            ui.add_space(15.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status_message).size(16.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
                    if ui.button(RichText::new("Save Exercise").size(18.0)).clicked() {
                        self.show_save_dialog = true;
                    }
                });
            });

            // Save dialog
            if self.show_save_dialog {
                self.show_save_dialog(ctx);
            }
        });
    }
}

impl HypertrophyApp {
    fn tab_button_styled(&self, name: &str, tab: Tab) -> egui::Button {
        let is_selected = std::mem::discriminant(&self.current_tab) == std::mem::discriminant(&tab);
        
        let text = if is_selected {
            RichText::new(name).strong().color(Color32::from_rgb(120, 200, 255)).size(18.0)
        } else {
            RichText::new(name).size(18.0)
        };
        
        let mut button = egui::Button::new(text);
        
        if is_selected {
            button = button.fill(Color32::from_rgb(40, 40, 60));
        }
        
        button
    }

    fn show_basic_info_tab(&mut self, ui: &mut Ui) {
        ui.heading("Basic Information");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Exercise Name:");
            ui.text_edit_singleline(&mut self.exercise.name);
        });

        ui.horizontal(|ui| {
            ui.label("Exercise Type:");
            ui.radio_value(&mut self.exercise.r#type, ExerciseType::Compound, "Compound");
            ui.radio_value(&mut self.exercise.r#type, ExerciseType::Isolation, "Isolation");
        });

        ui.horizontal(|ui| {
            ui.label("Description:");
            ui.text_edit_multiline(&mut self.exercise.description);
        });

        ui.horizontal(|ui| {
            ui.label("Plane of Motion:");
            ui.radio_value(
                &mut self.exercise.plane_of_motion,
                PlaneOfMotion::Sagittal,
                "Sagittal",
            );
            ui.radio_value(
                &mut self.exercise.plane_of_motion,
                PlaneOfMotion::Frontal,
                "Frontal",
            );
            ui.radio_value(
                &mut self.exercise.plane_of_motion,
                PlaneOfMotion::Transverse,
                "Transverse",
            );
        });

        ui.horizontal(|ui| {
            ui.label("Resistance Profile:");
            ui.radio_value(
                &mut self.exercise.resistance_profile,
                ResistanceProfile::Ascending,
                "Ascending",
            );
            ui.radio_value(
                &mut self.exercise.resistance_profile,
                ResistanceProfile::Descending,
                "Descending",
            );
            ui.radio_value(
                &mut self.exercise.resistance_profile,
                ResistanceProfile::Bell,
                "Bell",
            );
            ui.radio_value(
                &mut self.exercise.resistance_profile,
                ResistanceProfile::Constant,
                "Constant",
            );
        });

        ui.add_space(10.0);
        if ui.button("Next: Target Muscles").clicked() {
            self.current_tab = Tab::TargetMuscles;
        }
    }

    fn show_target_muscles_tab(&mut self, ui: &mut Ui) {
        ui.heading("Target Muscles");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Primary Muscle:");
            egui::ComboBox::from_label("")
                .selected_text(RichText::new(&self.exercise.target_muscles.muscle_name).size(18.0))
                .width(150.0)
                .show_ui(ui, |ui| {
                    for muscle in self.muscle_divisions.keys() {
                        if ui
                            .selectable_label(
                                self.exercise.target_muscles.muscle_name == *muscle,
                                muscle,
                            )
                            .clicked()
                        {
                            self.exercise.target_muscles.muscle_name = muscle.clone();
                            self.exercise.target_muscles.muscle_divisions.clear();
                        }
                    }
                });
        });

        if !self.exercise.target_muscles.muscle_name.is_empty() {
            ui.add_space(5.0);
            ui.label("Muscle Divisions:");
            
            if let Some(divisions) = self.muscle_divisions.get(&self.exercise.target_muscles.muscle_name) {
                for division in divisions {
                    let mut is_active = self.exercise.target_muscles.muscle_divisions
                        .iter()
                        .any(|md| md.name == *division && md.active);
                    
                    if ui.checkbox(&mut is_active, division).changed() {
                        self.exercise.target_muscles.muscle_divisions.retain(|md| md.name != *division);
                        
                        if is_active {
                            self.exercise.target_muscles.muscle_divisions.push(MuscleDivision {
                                name: division.clone(),
                                active: true,
                            });
                        } else {
                            self.exercise.target_muscles.muscle_divisions.push(MuscleDivision {
                                name: division.clone(),
                                active: false,
                            });
                        }
                    }
                }
            }
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Previous: Basic Info").clicked() {
                self.current_tab = Tab::BasicInfo;
            }
            if ui.button("Next: Joints Involved").clicked() {
                self.current_tab = Tab::JointsInvolved;
            }
        });
    }

    fn show_joints_involved_tab(&mut self, ui: &mut Ui) {
        ui.heading("Joints Involved");
        ui.add_space(10.0);

        let mut joints_to_remove = None;
        for (i, joint) in self.exercise.joints_involved.joints.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Joint {}:", i + 1));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
                        if ui.button("Remove").clicked() {
                            joints_to_remove = Some(i);
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Name:");
                    egui::ComboBox::from_id_source(format!("joint_name_combo_{}", i))
                        .selected_text(RichText::new(&joint.name).size(18.0))
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            for joint_name in &self.joint_names {
                                if ui.selectable_label(joint.name == *joint_name, joint_name).clicked() {
                                    joint.name = joint_name.clone();
                                }
                            }
                        });
                });

                ui.checkbox(&mut joint.dynamic, "Dynamic Movement").changed().then(|| {
                    if joint.dynamic {
                        joint.angle = None;
                        if joint.angle_initial.is_none() {
                            joint.angle_initial = Some(0);
                        }
                        if joint.angle_final.is_none() {
                            joint.angle_final = Some(0);
                        }
                    } else {
                        joint.direction = None;
                        joint.angle_initial = None;
                        joint.angle_final = None;
                        if joint.angle.is_none() {
                            joint.angle = Some(0);
                        }
                    }
                });

                if joint.dynamic {
                    ui.horizontal(|ui| {
                        ui.label("Direction:");
                        
                        let mut direction_text = joint.direction.clone().unwrap_or_default();
                        
                        if ui.text_edit_singleline(&mut direction_text).changed() {
                            joint.direction = Some(direction_text);
                        }
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Initial Angle:");
                        let mut angle = joint.angle_initial.unwrap_or(0);
                        if ui.add(egui::Slider::new(&mut angle, 0..=180).suffix("°")).changed() {
                            joint.angle_initial = Some(angle);
                        }
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Final Angle:");
                        let mut angle = joint.angle_final.unwrap_or(0);
                        if ui.add(egui::Slider::new(&mut angle, 0..=180).suffix("°")).changed() {
                            joint.angle_final = Some(angle);
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label("Fixed Angle:");
                        let mut angle = joint.angle.unwrap_or(0);
                        if ui.add(egui::Slider::new(&mut angle, 0..=180).suffix("°")).changed() {
                            joint.angle = Some(angle);
                        }
                    });
                }
            });
            ui.add_space(5.0);
        }

        if let Some(index) = joints_to_remove {
            self.exercise.joints_involved.joints.remove(index);
        }

        // Add new joint button
        if ui.button("Add Joint").clicked() {
            let joint_id = self.exercise.joints_involved.joints.len();
            
            self.exercise.joints_involved.joints.push(Joint {
                name: self.joint_names.first().cloned().unwrap_or_default(),
                dynamic: false,
                angle: Some(0),
                direction: None,
                angle_initial: None,
                angle_final: None,
            });
            
            self.status_message = format!("Added new joint (#{}).", joint_id + 1);
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Previous: Target Muscles").clicked() {
                self.current_tab = Tab::TargetMuscles;
            }
            if ui.button("Next: Additional Info").clicked() {
                self.current_tab = Tab::AdditionalInfo;
            }
        });
    }

    fn show_additional_info_tab(&mut self, ui: &mut Ui) {
        ui.heading("Additional Information");
        ui.add_space(10.0);

        ui.label("Tips for Proper Form:");
        ui.text_edit_multiline(&mut self.exercise.tips);

        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label("Technique Video URL:");
            ui.text_edit_singleline(&mut self.exercise.technique_video);
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Previous: Joints Involved").clicked() {
                self.current_tab = Tab::JointsInvolved;
            }
            if ui.button("Next: Preview").clicked() {
                self.current_tab = Tab::Preview;
            }
        });
    }

    fn show_preview_tab(&mut self, ui: &mut Ui) {
        ui.heading("Exercise Preview");
        ui.add_space(10.0);

        let json = serde_json::to_string_pretty(&self.exercise).unwrap_or_else(|_| "Error serializing exercise".to_string());
        
        ui.group(|ui| {
            ui.label(format!("Name: {}", self.exercise.name));
            ui.label(format!("Type: {:?}", self.exercise.r#type));
            ui.label(format!("Description: {}", self.exercise.description));
            ui.label(format!("Plane of Motion: {:?}", self.exercise.plane_of_motion));
            ui.label(format!("Resistance Profile: {:?}", self.exercise.resistance_profile));
            
            ui.add_space(5.0);
            ui.label(format!("Target Muscle: {}", self.exercise.target_muscles.muscle_name));
            ui.label("Active Divisions:");
            for division in &self.exercise.target_muscles.muscle_divisions {
                if division.active {
                    ui.label(format!("- {}", division.name));
                }
            }
            
            ui.add_space(5.0);
            ui.label("Joints Involved:");
            for joint in &self.exercise.joints_involved.joints {
                if joint.dynamic {
                    ui.label(format!(
                        "- {} (Dynamic): {} from {}° to {}°",
                        joint.name,
                        joint.direction.clone().unwrap_or_default(),
                        joint.angle_initial.unwrap_or(0),
                        joint.angle_final.unwrap_or(0)
                    ));
                } else {
                    ui.label(format!(
                        "- {} (Static): {}°",
                        joint.name,
                        joint.angle.unwrap_or(0)
                    ));
                }
            }
            
            ui.add_space(5.0);
            ui.label(format!("Tips: {}", self.exercise.tips));
            ui.label(format!("Video: {}", self.exercise.technique_video));
        });

        ui.collapsing("JSON Preview", |ui| {
            ui.monospace(json);
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Previous: Additional Info").clicked() {
                self.current_tab = Tab::AdditionalInfo;
            }
            if ui.button("Save Exercise").clicked() {
                self.show_save_dialog = true;
            }
        });
    }

    fn show_save_dialog(&mut self, ctx: &egui::Context) {
        if self.save_filename.is_empty() {
            self.save_filename = self.exercise.name.clone();
        }
        
        egui::Window::new("Save Exercise")
            .fixed_size([300.0, 100.0])
            .show(ctx, |ui| {
                ui.label("Enter a filename to save the exercise:");
                
                ui.text_edit_singleline(&mut self.save_filename);
                
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.show_save_dialog = false;
                        self.save_filename = String::new();
                    }
                    
                    if ui.button("Save").clicked() {
                        let filename_to_save = self.save_filename.clone();
                        self.save_exercise(&filename_to_save);
                        self.show_save_dialog = false;
                        self.save_filename = String::new();
                    }
                });
            });
    }

    fn save_exercise(&mut self, filename: &str) {
        use std::fs::{self, File};
        use std::io::Write;
        
        let sanitized = filename.trim().replace(" ", "_").to_lowercase();
        let filename = if sanitized.is_empty() { "unnamed_exercise".to_string() } else { sanitized };
        
        // Ensure data directory exists
        if !self.data_dir.exists() {
            if let Err(e) = fs::create_dir_all(&self.data_dir) {
                self.status_message = format!("Error creating data directory: {}", e);
                return;
            }
        }
        
        // Create file path
        let file_path = self.data_dir.join(format!("{}.json", filename));
        
        match serde_json::to_string_pretty(&self.exercise) {
            Ok(json) => {
                // Write to file
                match File::create(&file_path).and_then(|mut file| file.write_all(json.as_bytes())) {
                    Ok(_) => {
                        self.status_message = format!("Exercise saved to {}", file_path.display());
                    }
                    Err(e) => {
                        self.status_message = format!("Error writing file: {}", e);
                    }
                }
            }
            Err(e) => {
                self.status_message = format!("Error serializing exercise: {}", e);
            }
        }
    }
} 