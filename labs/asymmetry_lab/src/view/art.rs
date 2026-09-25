//! Icons, authored as SVG and rasterized at startup.
//!
//! The art direction is Chip's Challenge: chunky, pictographic, hard-outlined,
//! and readable at a glance without relying on hue. That last part is the
//! reason it was chosen — a silhouette with a heavy dark outline survives any
//! colour-vision deficiency, and so does the *motion* in `animate`.
//!
//! The SVG text is `include_str!`d rather than loaded through the asset server,
//! so the browser build still ships as a single file with no runtime fetches
//! and the lab keeps its "no asset pipeline" property. The art is still real
//! `.svg` under `art/`, editable in any vector tool and diffable in review,
//! which is what makes iterating on it cheap.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use observed_style::{ColorVisionMode, simulate_color_vision};

/// Every icon the board can draw. Adding one here forces it into the legend.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Icon {
    Pawn,
    Rival,
    Held,
    Guardian,
    Flag,
    FlagPlanted,
    Prison,
    Base,
    Wall,
    Ghost,
    Facing,
}

impl Icon {
    pub const ALL: [Icon; 11] = [
        Icon::Pawn,
        Icon::Rival,
        Icon::Held,
        Icon::Guardian,
        Icon::Flag,
        Icon::FlagPlanted,
        Icon::Prison,
        Icon::Base,
        Icon::Wall,
        Icon::Ghost,
        Icon::Facing,
    ];

    #[must_use]
    pub const fn source(self) -> &'static str {
        match self {
            Icon::Pawn => include_str!("../../art/pawn.svg"),
            Icon::Rival => include_str!("../../art/rival.svg"),
            Icon::Held => include_str!("../../art/held.svg"),
            Icon::Guardian => include_str!("../../art/guardian.svg"),
            Icon::Flag => include_str!("../../art/flag.svg"),
            Icon::FlagPlanted => include_str!("../../art/flag_planted.svg"),
            Icon::Prison => include_str!("../../art/prison.svg"),
            Icon::Base => include_str!("../../art/base.svg"),
            Icon::Wall => include_str!("../../art/wall.svg"),
            Icon::Ghost => include_str!("../../art/ghost.svg"),
            Icon::Facing => include_str!("../../art/facing.svg"),
        }
    }
}

/// Rasterize an SVG into straight RGBA8 at `size` square.
///
/// Returns `None` rather than panicking: a malformed icon should cost that one
/// mark, not the whole lab.
#[must_use]
pub fn rasterize(svg: &str, size: u32) -> Option<Vec<u8>> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg, &options).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;

    let source = tree.size();
    let scale = (size as f32 / source.width()).min(size as f32 / source.height());
    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Some(pixmap.take())
}

/// Recolour a rasterized icon through a colour-vision simulation.
///
/// The simulation has to reach the *pixels*. Icons carry their colour in the
/// texture, so tinting the sprite would leave the `Vision` control lying about
/// the artwork — showing a simulated board with unsimulated art on it, which is
/// worse than not offering the control at all.
fn simulate(rgba: &mut [u8], mode: ColorVisionMode) {
    if mode == ColorVisionMode::Normal {
        return;
    }
    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[3] == 0 {
            continue;
        }
        let source = Color::srgba_u8(pixel[0], pixel[1], pixel[2], 255);
        let seen: Color = simulate_color_vision(source, mode).into();
        let srgb = seen.to_srgba();
        pixel[0] = (srgb.red.clamp(0.0, 1.0) * 255.0) as u8;
        pixel[1] = (srgb.green.clamp(0.0, 1.0) * 255.0) as u8;
        pixel[2] = (srgb.blue.clamp(0.0, 1.0) * 255.0) as u8;
    }
}

/// The rasterized icon set: every icon under every colour-vision simulation,
/// built once at startup so cycling `Vision` is instant.
#[derive(Resource, Default)]
pub struct ArtAtlas {
    /// Keyed by the vision mode's index rather than the mode itself:
    /// `ColorVisionMode` is a production type and does not derive `Hash`, and a
    /// lab's convenience is not a reason to widen a shipped crate's API.
    icons: HashMap<(Icon, usize), Handle<Image>>,
}

/// A mode's position in [`ColorVisionMode::ALL`], which is its stable key.
fn vision_key(mode: ColorVisionMode) -> usize {
    ColorVisionMode::ALL
        .iter()
        .position(|candidate| *candidate == mode)
        .unwrap_or(0)
}

impl ArtAtlas {
    #[must_use]
    pub fn get(&self, icon: Icon, vision: ColorVisionMode) -> Option<Handle<Image>> {
        self.icons
            .get(&(icon, vision_key(vision)))
            .or_else(|| self.icons.get(&(icon, 0)))
            .cloned()
    }
}

/// Icons are drawn at this many pixels square. The board scales to fit the
/// viewport, so this is what a phone at 3x actually gets.
const ICON_PIXELS: u32 = 128;

pub fn load(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut atlas = ArtAtlas::default();
    for icon in Icon::ALL {
        let Some(base) = rasterize(icon.source(), ICON_PIXELS) else {
            warn!("icon {icon:?} failed to rasterize; it will not be drawn");
            continue;
        };
        for mode in ColorVisionMode::ALL {
            let mut rgba = base.clone();
            simulate(&mut rgba, mode);
            let image = Image::new(
                Extent3d {
                    width: ICON_PIXELS,
                    height: ICON_PIXELS,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                rgba,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
            );
            atlas
                .icons
                .insert((icon, vision_key(mode)), images.add(image));
        }
    }
    commands.insert_resource(atlas);
}

#[cfg(test)]
mod tests {
    use super::{ICON_PIXELS, Icon, rasterize};

    #[test]
    fn an_svg_becomes_pixels() {
        let red = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8">
            <rect width="8" height="8" fill="#ff0000"/></svg>"##;
        let pixels = rasterize(red, 16).expect("a valid svg rasterizes");
        assert_eq!(pixels.len(), 16 * 16 * 4);
        let centre = (8 * 16 + 8) * 4;
        assert!(pixels[centre] > 200, "red channel");
        assert!(pixels[centre + 1] < 60, "green channel");
        assert!(pixels[centre + 3] > 200, "opaque");
    }

    #[test]
    fn every_icon_rasterizes_to_something_visible() {
        // An icon that parses but draws nothing is worse than one that fails
        // loudly: it leaves a mark on the board that is simply invisible.
        for icon in Icon::ALL {
            let pixels =
                rasterize(icon.source(), ICON_PIXELS).unwrap_or_else(|| panic!("{icon:?} failed"));
            let opaque = pixels.chunks_exact(4).filter(|p| p[3] > 32).count();
            assert!(
                opaque > (ICON_PIXELS * ICON_PIXELS / 100) as usize,
                "{icon:?} rasterized nearly empty ({opaque} visible pixels)"
            );
        }
    }

    #[test]
    fn a_malformed_icon_costs_only_itself() {
        assert!(rasterize("not an svg at all", 16).is_none());
    }

    #[test]
    fn the_vision_simulation_reaches_the_pixels() {
        use observed_style::ColorVisionMode;
        let mut normal = rasterize(Icon::Guardian.source(), 32).expect("rasterizes");
        let mut simulated = normal.clone();
        super::simulate(&mut simulated, ColorVisionMode::Deuteranopia);
        assert_ne!(
            normal, simulated,
            "the simulation must change the artwork, not only the tint"
        );
        // `Normal` must be a no-op, or the default board is already lying.
        let untouched = rasterize(Icon::Guardian.source(), 32).expect("rasterizes");
        super::simulate(&mut normal, ColorVisionMode::Normal);
        assert_eq!(normal, untouched);
    }
}
