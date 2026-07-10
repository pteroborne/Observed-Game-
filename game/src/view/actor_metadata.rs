use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectionalCount {
    #[serde(rename = "billboard")]
    Billboard,
    #[serde(rename = "4-way")]
    Way4,
    #[serde(rename = "5-way")]
    Way5,
    #[serde(rename = "8-way")]
    Way8,
    #[serde(rename = "16-way")]
    Way16,
}

impl DirectionalCount {
    pub fn count(self) -> usize {
        match self {
            Self::Billboard => 1,
            Self::Way4 => 4,
            Self::Way5 => 5,
            Self::Way8 => 8,
            Self::Way16 => 16,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SpriteMetadata {
    pub name: String,
    pub image_path: String,
    pub frames: Vec<FrameRect>,
    pub pivot: (f32, f32),
    pub pixels_per_metre: f32,
    pub directional_count: DirectionalCount,
    pub clips: HashMap<String, Vec<usize>>,
    pub default_material_role: String,
}

impl SpriteMetadata {
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let metadata: Self = serde_json::from_str(&content)?;
        Ok(metadata)
    }
}
