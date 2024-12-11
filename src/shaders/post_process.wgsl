@group(0)
@binding(0)
var destination_texture: texture_storage_2d<rgba16float, write>;

@group(0) @binding(1)
var source_texture: texture_2d<f32>;
@group(0) @binding(2)
var source_texture_samp: sampler;

@compute
@workgroup_size(8,8,1)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let source_texture_size = textureDimensions(source_texture);
    
    // Check if we are not indexing out of our textures
    if any(id.xy > source_texture_size) { return; }

    let texture_coords = (vec2<f32>(id.xy) + vec2(0.5)) / vec2<f32>(source_texture_size);

    let color = textureSampleLevel(source_texture, source_texture_samp, texture_coords, 0.0);
    textureStore(destination_texture, id.xy, color);
}