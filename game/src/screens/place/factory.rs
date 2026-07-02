use bevy::prelude::*;
use observed_core::RoomId;

pub(crate) fn room_color(room_id: RoomId) -> Color {
    let r = (((room_id.0 * 17 + 5) % 255) as f32) / 255.0;
    let g = (((room_id.0 * 31 + 13) % 255) as f32) / 255.0;
    let b = (((room_id.0 * 59 + 29) % 255) as f32) / 255.0;
    Color::srgb(0.3 + r * 0.7, 0.3 + g * 0.7, 0.3 + b * 0.7)
}

pub(crate) struct RoomMaterialFactory;

impl RoomMaterialFactory {
    pub fn create_floor_material(
        room_id: RoomId,
        base_handle: &Handle<StandardMaterial>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        let mut mat = (*materials.get(base_handle).unwrap()).clone();
        let col = room_color(room_id);
        if mat.base_color_texture.is_some() {
            mat.base_color = Color::WHITE;
            mat.emissive = LinearRgba::from(col) * 0.45;
        } else {
            mat.base_color = col;
            mat.emissive = LinearRgba::from(col) * 3.0;
        }
        materials.add(mat)
    }

    pub fn create_light_color(room_id: RoomId) -> Color {
        let col = room_color(room_id);
        let r = 0.3 + (col.to_linear().red) * 2.0;
        let g = 0.3 + (col.to_linear().green) * 2.0;
        let b = 0.3 + (col.to_linear().blue) * 2.0;
        Color::srgb(r.min(1.0), g.min(1.0), b.min(1.0))
    }
}
