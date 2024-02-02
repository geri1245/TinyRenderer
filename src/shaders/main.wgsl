/// Draws a fullscreen quad, samples the GBuffer and calculates the final fragment values

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    let u = f32((i32(vertex_index) * 2) & 2);
    let v = f32(i32(vertex_index) & 2);
    
    // 0 -> 0,0
    // 1 -> 2, 0
    // 2 -> 0, 2

    let uv = vec2(u, v);
    // Flip the y coordinate (not sure why 1-y is correct here instead of 2-y 🤔)
    output.tex_coords = vec2(uv.x, 1.0 - uv.y);
    output.position = vec4(uv * 2.0 - 1.0, 0.0, 1.0);

    return output;
}

struct PointLight {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
    color: vec3<f32>,
}

struct DirectionalLight {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
    color: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> point_light: PointLight;
@group(0) @binding(1)
var<uniform> directional_light: DirectionalLight;

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    view_inv: mat4x4<f32>,
    proj: mat4x4<f32>,
    proj_inv: mat4x4<f32>,
    position: vec3<f32>,
};

@group(1) @binding(0)
var<uniform> camera: CameraUniform;

@group(2) @binding(0)
var t_position: texture_2d<f32>;
@group(2) @binding(1)
var s_position: sampler;
@group(2) @binding(2)
var t_normal: texture_2d<f32>;
@group(2) @binding(3)
var s_normal: sampler;
@group(2) @binding(4)
var t_albedo: texture_2d<f32>;
@group(2) @binding(5)
var s_albedo: sampler;

@group(3) @binding(0)
var t_shadow: texture_depth_2d;
@group(3) @binding(1)
var sampler_shadow: sampler_comparison;

fn is_valid_tex_coord(tex_coord: vec2<f32>) -> bool {
    return tex_coord.x >= 0.0 && tex_coord.x <= 1.0 && tex_coord.y >= 0.0 && tex_coord.y <= 1.0;
}

fn fetch_shadow(light_id: u32, fragment_pos: vec4<f32>) -> f32 {
    if fragment_pos.w <= 0.0 {
        return 1.0;
    }
    // Convert to NDC
    let fragment_pos_ndc = fragment_pos.xyz / fragment_pos.w;

    // Compute texture coordinates for shadow lookup
    // NDC goes from -1 to 1, tex coords go from 0 to 1. In addition y must be flipped
    let tex_coord = fragment_pos_ndc.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);

    if is_valid_tex_coord(tex_coord) {
        // Compare the shadow map sample against "the depth of the current fragment from the light's perspective"
        return textureSampleCompareLevel(t_shadow, sampler_shadow, tex_coord, fragment_pos_ndc.z);
    } else {
        return 1.0;
    }
}

const c_ambient_strength: f32 = 0.1;

@fragment
fn fs_main(fragment_pos_and_coords: VertexOutput) -> @location(0) vec4<f32> {
    var uv = fragment_pos_and_coords.tex_coords;
    // uv.y = 1.0 - uv.y;
    // return vec4<f32>(uv, 0.0, 1.0);

    let normal = textureSample(t_normal, s_normal, uv).xyz;
    let albedo_and_shininess = textureSample(t_albedo, s_albedo, uv);
    let albedo = albedo_and_shininess.xyz;
    let shininess = albedo_and_shininess.a;
    let position = textureSample(t_position, s_position, uv);

    let ambient_color = point_light.color * c_ambient_strength;

    let shadow = fetch_shadow(0u, point_light.view_proj * position);

    let pixel_to_point_light = normalize(point_light.position - position.xyz);
    let pixel_to_camera = normalize(camera.position.xyz - position.xyz);

    let diffuse_strength = max(dot(normal, pixel_to_point_light), 0.0);
    let diffuse_color = point_light.color * diffuse_strength;

    let half_dir = normalize(pixel_to_camera + pixel_to_point_light);
    let specular_strength = pow(max(dot(half_dir, normal), 0.0), 32.0);
    let specular_color = specular_strength * point_light.color;

    let result = (ambient_color + diffuse_color * shadow + specular_color * shadow) * albedo;
    return vec4(result, 1.0);
}
