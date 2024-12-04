struct GlobalGpuParams {
    random_parameter: f32,
    tone_mapping_type: u32,
}

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    view_inv: mat4x4<f32>,
    proj: mat4x4<f32>,
    proj_inv: mat4x4<f32>,
    position: vec3<f32>,
};

@group(0)
@binding(0)
var destination_texture: texture_storage_2d<rgba16float, write>;

@group(0) @binding(1)
var source_texture: texture_2d<f32>;
@group(0) @binding(2)
var source_texture_samp: sampler;

@group(1) @binding(0)
var<uniform> global_gpu_params: GlobalGpuParams;

@group(2) @binding(0)
var<uniform> camera: CameraUniform;

@group(3) @binding(0)
var skybox_texture: texture_cube<f32>;
@group(3) @binding(1)
var skybox_sampler: sampler;

@group(4) @binding(0)
var gbuffer_position_texture: texture_2d<f32>;
@group(4) @binding(1)
var gbuffer_position_sampler: sampler;
@group(4) @binding(2)
var gbuffer_normal_texture: texture_2d<f32>;
@group(4) @binding(3)
var gbuffer_normal_sampler: sampler;
@group(4) @binding(4)
var gbuffer_albedo_texture: texture_2d<f32>;
@group(4) @binding(5)
var gbuffer_albedo_sampler: sampler;
@group(4) @binding(6)
var gbuffer_rough_metal_ao_texture: texture_2d<f32>;
@group(4) @binding(7)
var gbuffer_rough_metal_ao_sampler: sampler;

@group(5) @binding(0)
var depth_texture: texture_depth_2d;
@group(5) @binding(1)
var depth_sampler: sampler;

const max_search_distance: f32 = 200.0;

@compute
@workgroup_size(8,8,1)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let source_texture_size = textureDimensions(source_texture);
    
    // Check if we are not indexing out of our textures
    if any(id.xy > source_texture_size) { return; }

    // The current pixel's coordinates in the range [0, 1]
    // let color = vec4(pixel_coords_texture_space.x, 0.0, 0.0, 1.0);
    // + 0.5 to sample the middle of the texels
    // division by source_texture_size to get from [0, texture_size] to [0, 1]
    // *2 -1 to get from [0, 1] to [-1, 1]
    let texture_coords = (vec2<f32>(id.xy) + vec2(0.5)) / vec2<f32>(source_texture_size);
    // let color = textureSampleLevel(depth_texture, depth_sampler, texture_coords, 0.0);

    let position = textureSampleLevel(gbuffer_position_texture, gbuffer_position_sampler, texture_coords, 0.0).xyz;
    let normal = normalize(textureSampleLevel(gbuffer_normal_texture, gbuffer_normal_sampler, texture_coords, 0.0).xyz);
    let reflection_direction = normalize(reflect(normalize(position - camera.position), normal));
    let reflection_end_pos = position + max_search_distance * reflection_direction;
    let reflection_end_pos_in_texture_space = vec4(reflection_end_pos, 1.0) * camera.view_proj;

    var color = vec4<f32>(0.0);
    if max(dot(normal, normalize(camera.position - position)), 0.0) < 0.5 {
        let skybox_reflection_color = textureSampleLevel(skybox_texture, skybox_sampler, reflection_direction, 0.0);
        color = skybox_reflection_color;
    } else {
    }
    color = textureSampleLevel(source_texture, source_texture_samp, texture_coords, 0.0);

    // let color = textureSampleLevel(source_texture, source_texture_samp, texture_coords, 0.0);

    let pixel_coords = vec2(i32(id.x), i32(id.y));
    textureStore(destination_texture, pixel_coords, vec4<f32>(color));
}