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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_assets_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|labs| labs.parent())
            .expect("lab crate lives under workspace/labs")
            .join("assets")
    }

    #[test]
    fn validate_all_derived_metadata() {
        let derived_dir = workspace_assets_dir().join("oga_25d").join("derived");
        if !derived_dir.exists() {
            println!("Derived assets directory does not exist yet; skipping validation.");
            return;
        }

        let entries = std::fs::read_dir(&derived_dir).expect("Read derived dir");
        let mut count = 0;
        for entry in entries {
            let entry = entry.expect("Valid entry");
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                count += 1;
                let metadata = SpriteMetadata::load_from_path(&path)
                    .unwrap_or_else(|e| panic!("Failed to load metadata file {:?}: {}", path, e));

                // 1. Verify image exists
                // The image_path is relative to the oga_25d/derived directory or relative to metadata path
                let image_path = derived_dir.join(&metadata.image_path);
                assert!(
                    image_path.exists(),
                    "Image file {:?} for metadata {:?} does not exist",
                    image_path,
                    path
                );

                // Load image dimensions using the image crate
                let img = image::ImageReader::open(&image_path)
                    .expect("Open image")
                    .into_dimensions()
                    .expect("Get image dimensions");
                let img_w = img.0;
                let img_h = img.1;

                // 2. Verify frames are in bounds
                assert!(
                    !metadata.frames.is_empty(),
                    "Frames list is empty in metadata {:?}",
                    path
                );
                for (idx, frame) in metadata.frames.iter().enumerate() {
                    assert!(
                        frame.x + frame.w <= img_w,
                        "Frame {} width in metadata {:?} exceeds image width (frame: {:?}, image_w: {})",
                        idx,
                        path,
                        frame,
                        img_w
                    );
                    assert!(
                        frame.y + frame.h <= img_h,
                        "Frame {} height in metadata {:?} exceeds image height (frame: {:?}, image_h: {})",
                        idx,
                        path,
                        frame,
                        img_h
                    );
                }

                // 3. Verify pivot points are normalized (0.0 to 1.0)
                assert!(
                    (0.0..=1.0).contains(&metadata.pivot.0),
                    "Pivot X {} in metadata {:?} is not normalized (0.0 to 1.0)",
                    metadata.pivot.0,
                    path
                );
                assert!(
                    (0.0..=1.0).contains(&metadata.pivot.1),
                    "Pivot Y {} in metadata {:?} is not normalized (0.0 to 1.0)",
                    metadata.pivot.1,
                    path
                );

                // 4. Verify pixels_per_metre is positive
                assert!(
                    metadata.pixels_per_metre > 0.0,
                    "pixels_per_metre {} in metadata {:?} must be positive",
                    metadata.pixels_per_metre,
                    path
                );

                // 5. Verify clips reference valid logical steps
                let dir_count = metadata.directional_count.count();
                let total_frames = metadata.frames.len();
                assert!(
                    total_frames.is_multiple_of(dir_count),
                    "Total frames {} in metadata {:?} is not a multiple of directional count {}",
                    total_frames,
                    path,
                    dir_count
                );
                let logical_steps = total_frames / dir_count;

                for (clip_name, steps) in &metadata.clips {
                    assert!(
                        !steps.is_empty(),
                        "Clip '{}' in metadata {:?} is empty",
                        clip_name,
                        path
                    );
                    for &step in steps {
                        assert!(
                            step < logical_steps,
                            "Clip '{}' index {} in metadata {:?} is out of bounds (logical steps: {})",
                            clip_name,
                            step,
                            path,
                            logical_steps
                        );
                    }
                }
            }
        }
        println!("Successfully validated {} metadata files.", count);
    }
}
