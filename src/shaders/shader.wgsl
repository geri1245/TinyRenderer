struct InstanceInput {
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,
    @location(9) normal_matrix_0: vec3<f32>,
    @location(10) normal_matrix_1: vec3<f32>,
    @location(11) normal_matrix_2: vec3<f32>,
};

struct CameraUniform {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
};

@group(1) @binding(0)
var<uniform> camera: CameraUniform;

struct Light {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
    color: vec3<f32>,
}
@group(0) @binding(0)
var<uniform> light: Light;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec4<f32>,
    @location(2) tex_coord: vec2<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    let normal_matrix = mat3x3<f32>(
        instance.normal_matrix_0,
        instance.normal_matrix_1,
        instance.normal_matrix_2,
    );

    let vertex_position = vec4<f32>(model.position, 1.0);

    var out: VertexOutput;
    out.tex_coord = model.tex_coord;
    out.world_normal = normal_matrix * model.normal;
    out.world_position = model_matrix * vertex_position;

    out.clip_position = camera.view_proj * model_matrix * vertex_position;

    return out;
}

@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var s_diffuse: sampler;

@group(3) @binding(0)
var t_shadow: texture_depth_2d_array;
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
        return textureSampleCompareLevel(t_shadow, sampler_shadow, tex_coord, i32(light_id), fragment_pos_ndc.z);
    } else {
        return 1.0;
    }
}

const c_ambient_strength: f32 = 0.1;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let texture_color = textureSample(t_diffuse, s_diffuse, in.tex_coord);

    let ambient_color = light.color * c_ambient_strength;

    let shadow = fetch_shadow(0u, light.view_proj * in.world_position);

    let pixel_to_light = normalize(light.position - in.world_position.xyz);
    let pixel_to_camera = normalize(camera.position.xyz - in.world_position.xyz);

    let diffuse_strength = max(dot(in.world_normal, pixel_to_light), 0.0);
    let diffuse_color = light.color * diffuse_strength;

    let half_dir = normalize(pixel_to_camera + pixel_to_light);
    let specular_strength = pow(max(dot(half_dir, in.world_normal), 0.0), 32.0);
    let specular_color = specular_strength * light.color;

    let result = (ambient_color + diffuse_color * shadow + specular_color * shadow) * texture_color.xyz;
    return vec4(result, texture_color.a);
}
