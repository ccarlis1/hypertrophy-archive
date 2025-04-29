use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Exercise {
    pub name: String,
    pub id: u32,
    pub r#type: ExerciseType,
    pub description: String,
    pub target_muscles: TargetMuscles,
    pub joints_involved: JointsInvolved,
    pub resistance_profile: ResistanceProfile,
    pub plane_of_motion: PlaneOfMotion,
    pub tips: String,
    pub technique_video: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExerciseType {
    Compound,
    Isolation,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TargetMuscles {
    pub muscle_name: String,
    #[serde(rename = "muscle_division")]
    pub muscle_divisions: Vec<MuscleDivision>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MuscleDivision {
    pub name: String,
    pub active: bool,
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
    pub intitial_angle: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_angle: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResistanceProfile {
    Ascending,
    Descending,
    Bell,
    Constant,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaneOfMotion {
    Sagittal,
    Frontal,
    Transverse,
} 