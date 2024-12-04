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

@compute
@workgroup_size(8,8,1)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let source_texture_size = vec2(textureDimensions(source_texture).x, textureDimensions(source_texture).y);
    
    // Check if we are not indexing out of our textures
    if any(id.xy > source_texture_size) { return; }

    let pixel_coords = vec2(i32(id.x), i32(id.y));
    let texture_coords = vec2(f32(id.x), f32(id.y)) / vec2<f32>(source_texture_size);

    let normal = normalize(textureSampleLevel(gbuffer_normal_texture, gbuffer_normal_sampler, texture_coords, 0.0).xyz);
    let position = textureSampleLevel(gbuffer_position_texture, gbuffer_position_sampler, texture_coords, 0.0).xyz;
    let reflection_direction = normalize(reflect(position - camera.position, normal));
    let color = textureSampleLevel(source_texture, source_texture_samp, texture_coords, 0.0);
    let skybox_reflection_color = textureSampleLevel(skybox_texture, skybox_sampler, reflection_direction, 0.0);

    textureStore(destination_texture, pixel_coords, color);
}