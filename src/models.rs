use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Joint actions keyed by their `name` slug.
pub type Archive = BTreeMap<String, Exercise>;

/// Load every `*.json` joint action under `dir` (saved as `<muscle>/<name>.json`).
/// Two files with the same `name` slug is an error. A missing `dir` is an empty archive.
pub fn load_archive(dir: &Path) -> Result<Archive, String> {
    let mut archive = Archive::new();
    let mut seen: BTreeMap<String, std::path::PathBuf> = BTreeMap::new();
    if !dir.exists() {
        return Ok(archive);
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = fs::read_dir(&d).map_err(|e| format!("read {}: {e}", d.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| format!("read {}: {e}", d.display()))?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let text =
                fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            let exercise: Exercise = serde_json::from_str(&text)
                .map_err(|e| format!("parse {}: {e}", path.display()))?;
            if let Some(first) = seen.get(&exercise.name) {
                return Err(format!(
                    "joint action slug {:?} is used by both {} and {}",
                    exercise.name,
                    first.display(),
                    path.display()
                ));
            }
            seen.insert(exercise.name.clone(), path);
            archive.insert(exercise.name.clone(), exercise);
        }
    }
    Ok(archive)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Exercise {
    pub name: String,
    pub id: u32,
    pub r#type: ExerciseType,
    pub description: String,
    #[serde(rename = "target_muscles")]
    pub target_muscle_groups: Vec<TargetMuscles>,
    pub joints_involved: JointsInvolved,
    pub resistance_profile: ResistanceProfile,
    pub plane_of_motion: PlaneOfMotion,
    pub tips: String,
    pub technique_video: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ExerciseType {
    Compound,
    Isolation,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TargetMuscles {
    pub muscle_name: String,
    pub primary: bool,
    #[serde(rename = "muscle_division")]
    pub muscle_divisions: Vec<MuscleDivision>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Copy, Default)]
pub enum BiasLevel {
    #[serde(rename = "none")]
    #[default]
    None,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "moderate")]
    Moderate,
    #[serde(rename = "strong")]
    Strong,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MuscleDivision {
    pub name: String,
    pub active: bool,
    #[serde(default)]
    pub bias: BiasLevel,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JointsInvolved {
    #[serde(rename = "joint")]
    pub joints: Vec<Joint>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Joint {
    pub name: String,
    pub dynamic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle_initial: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle_final: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ResistanceProfile {
    Ascending,
    Descending,
    Bell,
    Constant,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum PlaneOfMotion {
    Sagittal,
    Frontal,
    Transverse,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_exercise() -> Exercise {
        Exercise {
            name: "chest_press".to_string(),
            id: 0,
            r#type: ExerciseType::Compound,
            description: "A press in the transverse plane.".to_string(),
            target_muscle_groups: vec![TargetMuscles {
                muscle_name: "chest".to_string(),
                primary: true,
                muscle_divisions: vec![
                    MuscleDivision {
                        name: "sternocostal".to_string(),
                        active: true,
                        bias: BiasLevel::None,
                    },
                    MuscleDivision {
                        name: "clavicular".to_string(),
                        active: true,
                        bias: BiasLevel::Light,
                    },
                ],
            }],
            joints_involved: JointsInvolved {
                joints: vec![
                    Joint {
                        name: "shoulder".to_string(),
                        dynamic: false,
                        angle: Some(45),
                        direction: None,
                        angle_initial: None,
                        angle_final: None,
                    },
                    Joint {
                        name: "elbow".to_string(),
                        dynamic: true,
                        angle: None,
                        direction: Some("extension".to_string()),
                        angle_initial: Some(30),
                        angle_final: Some(180),
                    },
                ],
            },
            resistance_profile: ResistanceProfile::Descending,
            plane_of_motion: PlaneOfMotion::Transverse,
            tips: "Keep those shoulders tucked!".to_string(),
            technique_video: "https://example.com/video".to_string(),
        }
    }

    #[test]
    fn exercise_serde_round_trip() {
        let original = sample_exercise();
        let json = serde_json::to_string_pretty(&original).expect("serialize");
        let parsed: Exercise = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.r#type, original.r#type);
        assert_eq!(parsed.description, original.description);
        assert_eq!(parsed.resistance_profile, original.resistance_profile);
        assert_eq!(parsed.plane_of_motion, original.plane_of_motion);
        assert_eq!(parsed.tips, original.tips);
        assert_eq!(parsed.technique_video, original.technique_video);
        assert_eq!(parsed.target_muscle_groups.len(), 1);
        assert_eq!(parsed.target_muscle_groups[0].muscle_name, "chest");
        assert_eq!(parsed.target_muscle_groups[0].muscle_divisions.len(), 2);
        assert_eq!(
            parsed.target_muscle_groups[0].muscle_divisions[1].bias,
            BiasLevel::Light
        );
        assert_eq!(parsed.joints_involved.joints.len(), 2);
        assert_eq!(parsed.joints_involved.joints[0].angle, Some(45));
        assert_eq!(
            parsed.joints_involved.joints[1].direction.as_deref(),
            Some("extension")
        );
    }

    #[test]
    fn exercise_parses_example_shape() {
        let json = r#"{
            "name": "chest_press",
            "id": 0,
            "type": "compound",
            "description": "A press in the transverse plane.",
            "target_muscles": [
                {
                    "muscle_name": "chest",
                    "primary": true,
                    "muscle_division": [
                        { "name": "sternocostal", "active": true },
                        { "name": "clavicular", "active": true, "bias": "light" }
                    ]
                }
            ],
            "joints_involved": {
                "joint": [
                    { "name": "shoulder", "dynamic": false, "angle": 45 },
                    {
                        "name": "elbow",
                        "dynamic": true,
                        "direction": "extension",
                        "angle_initial": 30,
                        "angle_final": 180
                    }
                ]
            },
            "resistance_profile": "descending",
            "plane_of_motion": "transverse",
            "tips": "Keep those shoulders tucked!",
            "technique_video": "https://example.com/video"
        }"#;

        let exercise: Exercise = serde_json::from_str(json).expect("parse example shape");
        assert_eq!(exercise.name, "chest_press");
        assert_eq!(exercise.r#type, ExerciseType::Compound);
        assert_eq!(exercise.resistance_profile, ResistanceProfile::Descending);
        assert_eq!(exercise.plane_of_motion, PlaneOfMotion::Transverse);
        assert_eq!(exercise.target_muscle_groups[0].muscle_divisions[0].bias, BiasLevel::None);
        assert_eq!(exercise.target_muscle_groups[0].muscle_divisions[1].bias, BiasLevel::Light);
        assert_eq!(exercise.joints_involved.joints[1].angle_final, Some(180));
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("ha_archive_{tag}_{nanos}"))
    }

    fn write_exercise(dir: &Path, folder: &str, file: &str, name: &str) {
        let mut ex = sample_exercise();
        ex.name = name.to_string();
        let d = dir.join(folder);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join(file), serde_json::to_string(&ex).unwrap()).unwrap();
    }

    #[test]
    fn load_archive_reads_nested_files_by_slug() {
        let dir = temp_dir("ok");
        write_exercise(&dir, "chest", "press.json", "chest_press");
        write_exercise(&dir, "triceps", "pushdown.json", "pushdown");
        fs::write(dir.join("README.txt"), "not json").unwrap();
        let archive = load_archive(&dir).unwrap();
        assert_eq!(archive.keys().collect::<Vec<_>>(), ["chest_press", "pushdown"]);
        assert!(load_archive(&dir.join("missing")).unwrap().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_archive_rejects_duplicate_slugs() {
        let dir = temp_dir("dup");
        write_exercise(&dir, "chest", "a.json", "chest_press");
        write_exercise(&dir, "shoulders", "b.json", "chest_press");
        let err = load_archive(&dir).unwrap_err();
        assert!(err.contains("chest_press"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }
}
