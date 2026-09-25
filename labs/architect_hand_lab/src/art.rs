//! Authored SVG card motifs, rasterized once so the native and WASM builds
//! display the exact same pixels without an external asset pipeline.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use observed_mechanics::tiles::TileShape;

#[derive(Resource, Default)]
pub struct CardArt {
    images: HashMap<TileShape, Handle<Image>>,
}

impl CardArt {
    #[must_use]
    pub fn get(&self, shape: TileShape) -> Handle<Image> {
        self.images.get(&shape).cloned().unwrap_or_default()
    }
}

#[must_use]
pub const fn source(shape: TileShape) -> &'static str {
    match shape {
        TileShape::Sealed => include_str!("../art/sealed.svg"),
        TileShape::DeadEnd => include_str!("../art/dead_end.svg"),
        TileShape::Corridor => include_str!("../art/corridor.svg"),
        TileShape::Bend => include_str!("../art/bend.svg"),
        TileShape::Junction => include_str!("../art/junction.svg"),
        TileShape::Hall => include_str!("../art/hall.svg"),
        TileShape::Room => include_str!("../art/room.svg"),
    }
}

#[must_use]
pub fn rasterize(svg: &str, size: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let source = tree.size();
    let scale = (size as f32 / source.width()).min(size as f32 / source.height());
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Some(pixmap.take())
}

const CARD_PIXELS: u32 = 256;

pub fn load(mut art: ResMut<CardArt>, mut images: ResMut<Assets<Image>>) {
    for shape in TileShape::ALL {
        let Some(rgba) = rasterize(source(shape), CARD_PIXELS) else {
            warn!("card art for {shape:?} failed to rasterize");
            continue;
        };
        let image = Image::new(
            Extent3d {
                width: CARD_PIXELS,
                height: CARD_PIXELS,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
        art.images.insert(shape, images.add(image));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_authored_card_is_visible_pixels() {
        for shape in TileShape::ALL {
            let pixels = rasterize(source(shape), 128)
                .unwrap_or_else(|| panic!("{shape:?} did not rasterize"));
            let visible = pixels.chunks_exact(4).filter(|pixel| pixel[3] > 32).count();
            assert!(visible > 128 * 128 / 10, "{shape:?} is nearly empty");
        }
    }
}
