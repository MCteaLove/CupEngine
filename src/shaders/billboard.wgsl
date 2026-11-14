struct Globals {
    view_proj: mat4x4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_pos: vec4<f32>,
    light_dir: vec4<f32>,
    light_color: vec4<f32>,
    ambient: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

struct VsIn {
    @builtin(vertex_index) vi: u32,
    @location(0) pos_r: vec4<f32>,   // xyz = центр, w = радиус
    @location(1) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

var<private> CORNERS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
);

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let c = CORNERS[in.vi];
    let world = in.pos_r.xyz + (g.cam_right.xyz * c.x + g.cam_up.xyz * c.y) * in.pos_r.w;
    var out: VsOut;
    out.clip = g.view_proj * world4(world);
    out.uv = c;
    out.color = in.color;
    return out;
}

fn world4(p: vec3<f32>) -> vec4<f32> {
    return vec4<f32>(p, 1.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let d = length(in.uv);
    if (d > 1.0) {
        discard;
    }
    let core = smoothstep(1.0, 0.55, d);
    let rgb = mix(in.color.rgb, vec3<f32>(1.0), smoothstep(0.6, 0.0, d) * 0.6);
    return vec4<f32>(rgb, core * in.color.a);
}
