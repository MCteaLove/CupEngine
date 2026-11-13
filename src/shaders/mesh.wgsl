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
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) m0: vec4<f32>,
    @location(3) m1: vec4<f32>,
    @location(4) m2: vec4<f32>,
    @location(5) m3: vec4<f32>,
    @location(6) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let world = model * vec4<f32>(in.position, 1.0);
    var out: VsOut;
    out.clip = g.view_proj * world;
    out.world_pos = world.xyz;
    // верно для равномерного масштаба
    out.normal = normalize((model * vec4<f32>(in.normal, 0.0)).xyz);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let l = normalize(-g.light_dir.xyz);
    let v = normalize(g.cam_pos.xyz - in.world_pos);
    let h = normalize(l + v);
    let diff = max(dot(n, l), 0.0);
    let spec = pow(max(dot(n, h), 0.0), 48.0) * 0.35 * select(0.0, 1.0, diff > 0.0);
    let rgb = in.color.rgb * (g.ambient.rgb + g.light_color.rgb * diff) + g.light_color.rgb * spec;
    return vec4<f32>(rgb, in.color.a);
}
