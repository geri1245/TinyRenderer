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

struct Light {
    view_proj: mat4x4<f32>,
    position_or_direction: vec3<f32>,
    light_type: i32,
    color: vec3<f32>,
    far_plane_distance: f32,
}

@group(0) @binding(0)
var<uniform> lights: array<Light, 2>;

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
@group(2) @binding(6)
var t_metal_rough_ao: texture_2d<f32>;
@group(2) @binding(7)
var s_metal_rough_ao: sampler;

@group(3) @binding(0)
var t_shadow: texture_depth_2d_array;
@group(3) @binding(1)
var sampler_shadow: sampler_comparison;
@group(3) @binding(2)
var t_shadow_cube: texture_depth_cube;
@group(3) @binding(3)
var sampler_cube: sampler_comparison;

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
        return textureSampleCompareLevel(t_shadow, sampler_shadow, tex_coord, light_id, fragment_pos_ndc.z);
    } else {
        return 1.0;
    }
}

fn vector_to_depth_value(light_to_fragment: vec3<f32>) -> f32 {
    let abs_light_to_fragment = abs(light_to_fragment);
    let local_z = max(abs_light_to_fragment.x, max(abs_light_to_fragment.y, abs_light_to_fragment.z));

    let f = 100.0;
    let n = 0.1;
    let norm_z = (f + n) / (f - n) - (2 * f * n) / (f - n) / local_z;
    return (norm_z + 1.0) * 0.5;
}

fn get_shadow_value(light_id: u32, fragment_pos: vec3<f32>) -> f32 {
    let light = lights[light_id];
    let light_pos = light.position_or_direction;
    let tex_coord = fragment_pos.xyz - light_pos;
    let far_distance = light.far_plane_distance;

    // Compare the shadow map sample against "the depth of the current fragment from the light's perspective"
    return textureSampleCompareLevel(t_shadow_cube, sampler_cube, tex_coord, vector_to_depth_value(tex_coord));
}

const c_ambient_strength: f32 = 0.0;
const c_light_attenuation_constant: f32 = 1.0;
const c_light_attenuation_linear: f32 = 0.01;
const c_light_attenuation_quadratic: f32 = 0.0005;

fn get_light_diffuse_and_specular_contribution(pixel_to_light: vec3<f32>, pixel_to_camera: vec3<f32>, normal: vec3<f32>, shadow: f32) -> f32 {
    let diffuse_strength = max(dot(normal, pixel_to_light), 0.0);

    let half_dir = normalize(pixel_to_camera + pixel_to_light);
    let specular_strength = pow(max(dot(half_dir, normal), 0.0), 32.0);

    return (diffuse_strength + specular_strength) * shadow;
}

fn calculate_point_light_contribution(
    light: Light, pixel_to_camera: vec3<f32>, pixel_position: vec3<f32>, normal: vec3<f32>, shadow: f32
) -> vec3<f32> {
    let pixel_to_light = normalize(light.position_or_direction - pixel_position);

    let diffuse_and_specular_strength = get_light_diffuse_and_specular_contribution(pixel_to_light, pixel_to_camera, normal, shadow);

    let pixel_to_light_distance = length(light.position_or_direction - pixel_position);
    let attenuation = 1.0 / (c_light_attenuation_constant + c_light_attenuation_linear * pixel_to_light_distance + c_light_attenuation_quadratic * (pixel_to_light_distance * pixel_to_light_distance));

    return (c_ambient_strength + diffuse_and_specular_strength * shadow) * attenuation * light.color;
}

@fragment
fn fs_main(fragment_pos_and_coords: VertexOutput) -> @location(0) vec4<f32> {
    var uv = fragment_pos_and_coords.tex_coords;

    let normal = normalize(textureSample(t_normal, s_normal, uv).xyz);
    let albedo_and_shininess = textureSample(t_albedo, s_albedo, uv);
    let albedo = albedo_and_shininess.xyz;
    let position = textureSample(t_position, s_position, uv);

    var final_color = vec3<f32>(0, 0, 0);

    for (var i = 0u; i < 2; i += 1u) {
        let light = lights[i];
        let pixel_to_camera = normalize(camera.position.xyz - position.xyz);

        if light.light_type == 1 {
            let shadow = get_shadow_value(i, position.xyz);

            final_color += calculate_point_light_contribution(light, pixel_to_camera, position.xyz, normal, shadow);
        } else if light.light_type == 2 {
            let shadow = fetch_shadow(i, light.view_proj * position);
            let diffuse_and_specular = get_light_diffuse_and_specular_contribution(-light.position_or_direction, pixel_to_camera, normal, shadow);
            final_color += (c_ambient_strength + diffuse_and_specular) * light.color;
        }
    }

    return vec4(final_color * albedo, 1);
}
