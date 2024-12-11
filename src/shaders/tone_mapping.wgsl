struct GlobalGpuParams {
    random_parameter: f32,
    tone_mapping_type: u32,
}

@group(0)
@binding(0)
var destination_texture: texture_storage_2d<rgba8unorm, write>;

@group(0) @binding(1)
var source_texture: texture_2d<f32>;
@group(0) @binding(2)
var source_texture_samp: sampler;

@group(1) @binding(0)
var<uniform> global_gpu_params: GlobalGpuParams;

@compute
@workgroup_size(8,8,1)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let source_texture_size = textureDimensions(source_texture);
    
    // Check if we are not indexing out of our textures
    if any(id.xy > source_texture_size) { return; }

    let texture_coords = (vec2<f32>(id.xy) + vec2(0.5)) / vec2<f32>(source_texture_size);

    let hdr_color = textureSampleLevel(source_texture, source_texture_samp, texture_coords, 0.0).xyz;

    // Tone mapping
    var ldr_color = hdr_color;
    if global_gpu_params.tone_mapping_type == 1 {
        ldr_color = vec3(1.0) - exp(-hdr_color * global_gpu_params.random_parameter); // Exposure-based
    } else if global_gpu_params.tone_mapping_type == 2 {
        ldr_color = hdr_color / (hdr_color + vec3(1.0)); // Reinhard
    }

    // // Gamma correction
    let gamma_corrected_color = vec4<f32>(pow(ldr_color, vec3(1.0 / 2.2)), 1.0);

    textureStore(destination_texture, id.xy, gamma_corrected_color);
}