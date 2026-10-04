// Bounded decorative indoor rain, impact rings and fixed luminous roof panels.
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::{globals,view}
struct Settings { tint:vec4<f32>, haze:vec4<f32>, unpowered:vec4<f32>, params:vec4<f32> }
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> weather:Settings;
@fragment
fn fragment(in:VertexOutput)->@location(0) vec4<f32> {
 let uv=in.uv; let t=globals.time; let power=weather.params.x;let kind=weather.params.y;
 var color=weather.tint.rgb;var alpha=0.0;
 if kind<0.5 {
  let phase=fract(uv.y*0.61+t*(2.2+fract(uv.x*0.317)*0.8)+uv.x*0.271);
  let streak=smoothstep(0.89,0.95,phase)*(1.0-smoothstep(0.98,1.0,phase));
  alpha=streak*0.22; color=color*(0.12+power*0.70);
 } else if kind<1.5 {
  let cell=floor(uv*1.5);let h=fract(sin(dot(cell,vec2(127.1,311.7)))*43758.5453);
  let p=fract(uv*1.5)-vec2(0.5); let age=fract(t*0.48+h);
  let ring=1.0-smoothstep(0.008,0.025,abs(length(p)-age*0.43));
  alpha=ring*(1.0-age)*0.19; color=color*(0.15+power*0.55);
 } else {
  alpha=1.0; color=mix(weather.unpowered.rgb,color*2.8,power);
 }
 let fog=clamp((distance(view.world_position,in.world_position.xyz)-weather.params.z)/(weather.params.w-weather.params.z),0.0,1.0);
 return vec4(mix(color,weather.haze.rgb,fog),alpha);
}
