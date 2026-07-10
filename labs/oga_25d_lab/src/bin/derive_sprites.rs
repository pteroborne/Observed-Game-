use image::GenericImageView;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

// We redefine minimal structs here or we can import them.
// Importing is cleaner. Let's write them directly to JSON using serde_json.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = manifest_dir.parent().unwrap().parent().unwrap();
    let raw_dir = workspace_dir.join("assets").join("oga_25d").join("raw");
    let derived_dir = workspace_dir.join("assets").join("oga_25d").join("derived");
    let temp_dir = manifest_dir.join("temp_extract");

    println!("Starting sprite derivation...");
    println!("Raw dir: {:?}", raw_dir);
    println!("Derived dir: {:?}", derived_dir);

    // Create target and temp directories
    fs::create_dir_all(&derived_dir)?;
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    // Helper: Convert cyan pixels to transparent
    let make_cyan_transparent = |img: &mut image::RgbaImage| {
        for pixel in img.pixels_mut() {
            // Cyan is (0, 255, 255)
            if pixel[0] == 0 && pixel[1] == 255 && pixel[2] == 255 {
                *pixel = image::Rgba([0, 0, 0, 0]);
            }
        }
    };

    // Helper: Extract a zip file using system tar
    let extract_zip =
        |zip_name: &str, sub_path: &Path| -> Result<PathBuf, Box<dyn std::error::Error>> {
            let zip_path = raw_dir.join(sub_path).join(zip_name);
            if !zip_path.exists() {
                return Err(format!("Zip not found: {:?}", zip_path).into());
            }
            let out_dir = temp_dir.join(zip_name.replace(".zip", ""));
            fs::create_dir_all(&out_dir)?;

            println!("Extracting {:?} to {:?}", zip_path, out_dir);
            let status = Command::new("tar")
                .arg("-xf")
                .arg(&zip_path)
                .arg("-C")
                .arg(&out_dir)
                .status()?;
            if !status.success() {
                return Err(format!("tar extraction failed for {:?}", zip_name).into());
            }
            Ok(out_dir)
        };

    // Helper: Save derived PNG and write its JSON metadata
    let save_derived = |name: &str,
                        img: &image::RgbaImage,
                        pivot: (f32, f32),
                        ppm: f32,
                        dir_count: &str,
                        clips: HashMap<String, Vec<usize>>,
                        material_role: &str|
     -> Result<(), Box<dyn std::error::Error>> {
        let png_name = format!("{}.png", name);
        let json_name = format!("{}.json", name);
        let png_path = derived_dir.join(&png_name);
        let json_path = derived_dir.join(&json_name);

        // Save PNG
        img.save(&png_path)?;

        // Build metadata JSON manually or using serde_json
        let mut frames = Vec::new();
        // Since we cropped it, standard derived files are single frames, EXCEPT actor sheets
        if name == "rival_actor" {
            // Rival is an 8x7 grid of 64x64 cells
            for row in 0..7 {
                for col in 0..8 {
                    frames.push(serde_json::json!({
                        "x": col * 64,
                        "y": row * 64,
                        "w": 64,
                        "h": 64
                    }));
                }
            }
        } else if name == "guardian_actor" {
            // Guardian is a 6x1 horizontal strip of 64x64 cells
            for col in 0..6 {
                frames.push(serde_json::json!({
                    "x": col * 64,
                    "y": 0,
                    "w": 64,
                    "h": 64
                }));
            }
        } else {
            frames.push(serde_json::json!({
                "x": 0,
                "y": 0,
                "w": img.width(),
                "h": img.height()
            }));
        }

        let metadata = serde_json::json!({
            "name": name,
            "image_path": png_name,
            "frames": frames,
            "pivot": [pivot.0, pivot.1],
            "pixels_per_metre": ppm,
            "directional_count": dir_count,
            "clips": clips,
            "default_material_role": material_role
        });

        let mut file = File::create(&json_path)?;
        file.write_all(serde_json::to_string_pretty(&metadata)?.as_bytes())?;
        println!("Savedderived: {} (+ JSON)", name);
        Ok(())
    };

    // 1. Rival Actor Sheet (derived from knekko_guard/guard_spritesheet.png)
    // knekko_guard/guard_spritesheet.png is transparent RGBA 512x448
    let guard_src_path = raw_dir.join("knekko_guard").join("guard_spritesheet.png");
    if guard_src_path.exists() {
        let img = image::ImageReader::open(&guard_src_path)?
            .decode()?
            .to_rgba8();
        let mut clips = HashMap::new();
        clips.insert("idle".to_string(), vec![0]);
        clips.insert("walk".to_string(), vec![0, 1, 2, 3]);
        clips.insert("operate".to_string(), vec![4]);
        clips.insert("alert".to_string(), vec![5]);
        clips.insert("disrupted".to_string(), vec![6]);
        save_derived("rival_actor", &img, (0.5, 0.0), 64.0, "8-way", clips, "Rival")?;
    } else {
        println!("Warning: guard spritesheet not found!");
    }

    // 2. Keycards
    // xcvg_keycards/xcvg_cardkeys_premade.zip
    let xcvg_dir = extract_zip("xcvg_cardkeys_premade.zip", Path::new("xcvg_keycards"))?;

    // Keystone Card (using full_id.png)
    let keystone_src = xcvg_dir.join("full_id.png");
    if keystone_src.exists() {
        let img = image::ImageReader::open(&keystone_src)?
            .decode()?
            .to_rgba8();
        let mut clips = HashMap::new();
        clips.insert("idle".to_string(), vec![0]);
        save_derived(
            "keystone_card",
            &img,
            (0.5, 0.5),
            80.0,
            "billboard",
            clips,
            "You",
        )?;
    }

    // Keystone Core (using full_stripe.png)
    let core_src = xcvg_dir.join("full_stripe.png");
    if core_src.exists() {
        let img = image::ImageReader::open(&core_src)?.decode()?.to_rgba8();
        let mut clips = HashMap::new();
        clips.insert("idle".to_string(), vec![0]);
        save_derived(
            "keystone_core",
            &img,
            (0.5, 0.5),
            80.0,
            "billboard",
            clips,
            "You",
        )?;
    }

    // Exit Access Card (using slim_id.png)
    let exit_src = xcvg_dir.join("slim_id.png");
    if exit_src.exists() {
        let img = image::ImageReader::open(&exit_src)?.decode()?.to_rgba8();
        let mut clips = HashMap::new();
        clips.insert("idle".to_string(), vec![0]);
        save_derived(
            "exit_access_card",
            &img,
            (0.5, 0.5),
            80.0,
            "billboard",
            clips,
            "Control",
        )?;
    }

    // 3. Items (from raw/nmn_items/items_paletted.png)
    let items_src_path = raw_dir.join("nmn_items").join("items_paletted.png");
    if items_src_path.exists() {
        let full_items_img = image::ImageReader::open(&items_src_path)?
            .decode()?
            .to_rgba8();

        let crop_and_save_item = |name: &str,
                                  x: u32,
                                  y: u32,
                                  w: u32,
                                  h: u32,
                                  pivot: (f32, f32),
                                  role: &str|
         -> Result<(), Box<dyn std::error::Error>> {
            let cropped = full_items_img.view(x, y, w, h).to_image();
            let mut rgba_img = cropped.clone();
            make_cyan_transparent(&mut rgba_img);
            let mut clips = HashMap::new();
            clips.insert("idle".to_string(), vec![0]);
            save_derived(name, &rgba_img, pivot, 80.0, "billboard", clips, role)?;
            Ok(())
        };

        // Crop definitions
        crop_and_save_item("battery_charge", 224, 193, 33, 40, (0.5, 0.5), "Control")?;
        crop_and_save_item("route_cell", 216, 143, 28, 21, (0.5, 0.5), "Control")?;
        crop_and_save_item("repair_token", 109, 257, 26, 27, (0.5, 0.5), "Control")?;
        crop_and_save_item("relay_device", 263, 60, 33, 19, (0.5, 0.5), "Control")?;
        crop_and_save_item("anchor_torch", 128, 36, 15, 17, (0.5, 0.0), "You")?;
    }

    // 4. Decorations (from raw/nmn_decorations/decorations_a_paletted.png)
    let dec_src_path = raw_dir
        .join("nmn_decorations")
        .join("decorations_a_paletted.png");
    if dec_src_path.exists() {
        let full_dec_img = image::ImageReader::open(&dec_src_path)?
            .decode()?
            .to_rgba8();

        let crop_and_save_dec = |name: &str,
                                 x: u32,
                                 y: u32,
                                 w: u32,
                                 h: u32,
                                 pivot: (f32, f32),
                                 role: &str|
         -> Result<(), Box<dyn std::error::Error>> {
            let cropped = full_dec_img.view(x, y, w, h).to_image();
            let mut rgba_img = cropped.clone();
            make_cyan_transparent(&mut rgba_img);
            let mut clips = HashMap::new();
            clips.insert("idle".to_string(), vec![0]);
            save_derived(name, &rgba_img, pivot, 64.0, "billboard", clips, role)?;
            Ok(())
        };

        crop_and_save_dec("column", 328, 25, 27, 88, (0.5, 0.0), "Plain")?;
        crop_and_save_dec("torch_wall", 219, 30, 27, 83, (0.5, 0.0), "You")?;
    }

    // 5. LAB Sampler
    // mutantleg_lab_sprites/lab_sprite.zip
    let ml_sprites_dir = extract_zip("lab_sprite.zip", Path::new("mutantleg_lab_sprites"))?;
    // mutantleg_lab_textures/lab_texture.zip
    let ml_textures_dir = extract_zip("lab_texture.zip", Path::new("mutantleg_lab_textures"))?;
 
    // Guardian Actor Sheet (Stitched horizontal sheet of robot sprites: 6 frames of 64x64)
    let robot_frames = [
        "robot0.png",
        "robot1.png",
        "robot2.png",
        "robotatt0.png",
        "robotatt1.png",
        "robothit0.png",
    ];
    let mut guardian_img = image::RgbaImage::new(384, 64);
    let mut all_exist = true;
    for (i, frame_name) in robot_frames.iter().enumerate() {
        let frame_path = ml_sprites_dir.join("LAB").join("sprites").join(frame_name);
        if frame_path.exists() {
            let frame_img = image::ImageReader::open(&frame_path)?.decode()?.to_rgba8();
            for y in 0..64 {
                for x in 0..64 {
                    guardian_img.put_pixel(i as u32 * 64 + x, y, *frame_img.get_pixel(x, y));
                }
            }
        } else {
            all_exist = false;
        }
    }
    if all_exist {
        make_cyan_transparent(&mut guardian_img);
        let mut clips = HashMap::new();
        clips.insert("idle".to_string(), vec![0]);
        clips.insert("walk".to_string(), vec![0, 1, 2]);
        clips.insert("operate".to_string(), vec![3]);
        clips.insert("alert".to_string(), vec![4]);
        clips.insert("disrupted".to_string(), vec![5]);
        save_derived(
            "guardian_actor",
            &guardian_img,
            (0.5, 0.0),
            64.0,
            "billboard",
            clips,
            "Director",
        )?;
    }

    // Crate
    let crate_src = ml_sprites_dir.join("LAB").join("sprites").join("crate.png");
    if crate_src.exists() {
        let img = image::ImageReader::open(&crate_src)?.decode()?.to_rgba8();
        let mut clips = HashMap::new();
        clips.insert("idle".to_string(), vec![0]);
        save_derived(
            "lab_crate",
            &img,
            (0.5, 0.0),
            64.0,
            "billboard",
            clips,
            "Plain",
        )?;
    }

    // Table
    let table_src = ml_sprites_dir
        .join("LAB")
        .join("sprites")
        .join("d_table.png");
    if table_src.exists() {
        let img = image::ImageReader::open(&table_src)?.decode()?.to_rgba8();
        let mut clips = HashMap::new();
        clips.insert("idle".to_string(), vec![0]);
        save_derived(
            "lab_table",
            &img,
            (0.5, 0.0),
            64.0,
            "billboard",
            clips,
            "Plain",
        )?;
    }

    // Wall Tile (Texture)
    let tile_src = ml_textures_dir.join("LAB").join("wall").join("tile000.png");
    if tile_src.exists() {
        let img = image::ImageReader::open(&tile_src)?.decode()?.to_rgba8();
        let mut clips = HashMap::new();
        clips.insert("idle".to_string(), vec![0]);
        save_derived(
            "lab_wall_tile",
            &img,
            (0.5, 0.5),
            64.0,
            "billboard",
            clips,
            "Plain",
        )?;
    }

    // Cleanup temp extraction dir
    fs::remove_dir_all(&temp_dir)?;

    // Copy derived actors directly to assets/sprites/
    let sprites_dir = workspace_dir.join("assets").join("sprites");
    fs::create_dir_all(&sprites_dir)?;
    for name in &["rival_actor", "guardian_actor"] {
        for ext in &["png", "json"] {
            let src = derived_dir.join(format!("{}.{}", name, ext));
            let dest = sprites_dir.join(format!("{}.{}", name, ext));
            if src.exists() {
                fs::copy(&src, &dest)?;
                println!("Copied derived actor file to assets/sprites/: {}.{}", name, ext);
            }
        }
    }

    // Copy decoration sprites
    let decors = [
        ("column.png", "decor_column.png"),
        ("torch_wall.png", "decor_torch.png"),
        ("lab_crate.png", "decor_lab_crate.png"),
        ("lab_table.png", "decor_lab_table.png"),
    ];
    for (src_name, dest_name) in decors {
        let src = derived_dir.join(src_name);
        let dest = sprites_dir.join(dest_name);
        if src.exists() {
            fs::copy(&src, &dest)?;
            println!("Copied derived decoration file to assets/sprites/: {}", dest_name);
        }
    }

    // Copy lab wall texture tile to assets/textures/wall_albedo_lab.png
    let textures_dir = workspace_dir.join("assets").join("textures");
    fs::create_dir_all(&textures_dir)?;
    let tile_src = derived_dir.join("lab_wall_tile.png");
    let tile_dest = textures_dir.join("wall_albedo_lab.png");
    if tile_src.exists() {
        fs::copy(&tile_src, &tile_dest)?;
        println!("Copied derived wall tile to assets/textures/wall_albedo_lab.png");
    }

    println!("Sprite derivation completed successfully!");
    Ok(())
}
