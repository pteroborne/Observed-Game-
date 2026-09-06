// The water surface of `daydream_lab`.
//
// The reflection arrives as a texture rendered by a second camera placed at
// the main camera's position reflected through the plane y = 0. Because that
// camera sees the world from where the reflection appears to come from, the
// image it produces already lines up with the screen - so this shader samples
// it at the fragment's screen position rather than at the mesh's UVs.
//
// On top of that: a Schlick term, so the water is nearly a mirror at grazing
// angles and mostly its own colour when looked straight down into; a slow
// ripple that grows with distance, so the near water is glass and the far
// water breathes; and the same linear haze the rest of the scene fades into.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::{view, globals}

struct WaterSettings {
    /// The colour of the water itself, seen through the reflection.
    tint: vec4<f32>,
    /// The colour everything fades to. Matches the scene's fog and clear colour.
    haze: vec4<f32>,
    /// x: fog start, y: fog end, z: reflectance head-on, w: ripple amplitude.
    params: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> water: WaterSettings;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var reflection_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var reflection_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world = in.world_position.xyz;
    let distance_to_eye = distance(view.world_position, world);

    // Two slow waves crossing at an angle, so the disturbance never reads as a
    // single travelling line. Amplitude rises with distance because a still
    // surface only ever looks disturbed where the reflection is stretched.
    let t = globals.time;
    let swell = sin(world.z * 0.42 + t * 0.55) + sin(world.x * 0.61 - t * 0.37) * 0.6;
    let reach = clamp(distance_to_eye / 40.0, 0.0, 1.0);
    let ripple = swell * water.params.w * reach;

    var uv = in.position.xy / view.viewport.zw;
    uv.y = uv.y + ripple;
    uv = clamp(uv, vec2(0.0), vec2(1.0));
    // Water is a lossy mirror. Taking a bite out of the reflection is what
    // separates a pool from a sheet of glass.
    let sampled = textureSample(reflection_texture, reflection_sampler, uv).rgb;
    let reflected = sampled * vec3(0.74, 0.83, 0.86);

    // Schlick. Head-on the water is its own colour; at a grazing angle down
    // the length of the hall it is a mirror.
    let to_eye = normalize(view.world_position - world);
    let facing = clamp(dot(to_eye, normalize(in.world_normal)), 0.0, 1.0);
    let head_on = water.params.z;
    // Schlick would take this to 1.0 at the horizon. Capped short of that,
    // because a mirror that perfect stops reading as a surface at all.
    let fresnel = min(head_on + (1.0 - head_on) * pow(1.0 - facing, 5.0), 0.80);

    var color = mix(water.tint.rgb, reflected, fresnel);

    let fog = clamp(
        (distance_to_eye - water.params.x) / (water.params.y - water.params.x),
        0.0,
        1.0,
    );
    color = mix(color, water.haze.rgb, fog);

    return vec4(color, 1.0);
}
