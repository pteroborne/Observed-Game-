// Original decorative conveyor field. UV x is metres along the belt's direction.
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

struct FieldSettings {
    tint: vec4<f32>,
    edge: vec4<f32>,
    params: vec4<f32>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> field: FieldSettings;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = globals.time;
    let u = uv.x - t * field.params.y;
    let v = uv.y;
    // Crossing filaments drift smoothly: no flashing or stochastic temporal noise.
    let bend = sin(u * 2.4 + v * 7.0 + t * 0.6) * 0.12;
    let bands = pow(0.5 + 0.5 * sin(u * 12.0 + bend * 5.0), 14.0);
    let filaments = pow(0.5 + 0.5 * sin(v * 35.0 + u * 2.0 + bend), 20.0);
    let interference = 0.5 + 0.5 * sin(u * 5.3 - v * 9.0 + t * 0.4);
    let rim = pow(abs(v * 2.0 - 1.0), 14.0);
    let strength = 0.18 + bands * 0.36 + filaments * interference * 0.22;
    let power = field.params.x;
    let color = (field.tint.rgb * strength * 2.4 + field.edge.rgb * rim * 0.7) * power;
    let alpha = (0.30 + bands * 0.24 + filaments * 0.10 + rim * 0.16) * mix(0.45, 1.0, power);
    return vec4(color, alpha);
}
