//! Upload repeating surface textures with filtered mipmaps.

use bevy::prelude::*;
use observed_style as style;

/// Upload a surface image ([`style::surfaces`]) as a repeating texture with its whole
/// mip chain, box-filtered here.
///
/// Without mips a fine pattern - a weave, a joint, tread plate - shimmers into moiré
/// at a few metres. `srgb` for an albedo; a normal map is linear, and its texels are
/// renormalised at every level so a distant surface keeps unit normals.
pub(super) fn surface_texture(
    images: &mut Assets<Image>,
    data: Vec<u8>,
    srgb: bool,
) -> Handle<Image> {
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let size = style::surfaces::SURFACE_TEXTURE_SIZE as usize;
    let mut chain = data.clone();
    let mut level = data;
    let mut side = size;
    let mut levels = 1u32;
    while side > 1 {
        let half = side / 2;
        let mut next = Vec::with_capacity(half * half * 4);
        for y in 0..half {
            for x in 0..half {
                let texel = |dx: usize, dy: usize, c: usize| {
                    f32::from(level[((y * 2 + dy) * side + x * 2 + dx) * 4 + c])
                };
                let mut rgba = [0.0f32; 4];
                for (c, channel) in rgba.iter_mut().enumerate() {
                    *channel =
                        (texel(0, 0, c) + texel(1, 0, c) + texel(0, 1, c) + texel(1, 1, c)) * 0.25;
                }
                if !srgb {
                    let v = |c: f32| c / 127.5 - 1.0;
                    let (nx, ny, nz) = (v(rgba[0]), v(rgba[1]), v(rgba[2]));
                    let length = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-4);
                    for (channel, n) in rgba.iter_mut().zip([nx, ny, nz]) {
                        *channel = (n / length + 1.0) * 127.5;
                    }
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                next.extend(rgba.map(|c| c.round().clamp(0.0, 255.0) as u8));
            }
        }
        chain.extend_from_slice(&next);
        level = next;
        side = half;
        levels += 1;
    }
    #[allow(clippy::cast_possible_truncation)]
    let extent = Extent3d {
        width: size as u32,
        height: size as u32,
        depth_or_array_layers: 1,
    };
    let base = chain[..size * size * 4].to_vec();
    let mut image = Image::new(
        extent,
        TextureDimension::D2,
        base,
        if srgb {
            TextureFormat::Rgba8UnormSrgb
        } else {
            TextureFormat::Rgba8Unorm
        },
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    // `Image::new` checks the data against the base level alone; the chain goes in
    // after, every level in order, which is how a mip-mapped image is laid out.
    image.data = Some(chain);
    image.texture_descriptor.mip_level_count = levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    images.add(image)
}
