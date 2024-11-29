@group(0) @binding(0)
var source_texture: texture_2d<f32>;
@group(0) @binding(1)
var source_texture_samp: sampler;

@group(1) @binding(0)
var destination_texture: texture_storage_2d<rgba8unorm, write>;

@compute
@workgroup_size(8,8,1)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let destination_texture_size = textureDimensions(destination_texture).xy;
    let source_texture_size = textureDimensions(source_texture).xy;
    let max_source_index = id.xy * 2 + 1;

    // Check if we are not indexing out of our textures
    if any(id.xy > destination_texture_size) { return; }
    if any(max_source_index > source_texture_size) { return; }

    let source_texture_size_f32 = vec2<f32>(source_texture_size);
    // Use a simple box filter
    let offset = vec2(0.0, 1.0);
    // Index of the top left texel of the box filter, converted to vec2<f32>
    let source_texture_base_coords = 2.0 * vec2(f32(id.x), f32(id.y));
    var color = textureSampleLevel(source_texture, source_texture_samp, (source_texture_base_coords + offset.xx) / source_texture_size_f32, 0.0);
    color += textureSampleLevel(source_texture, source_texture_samp, (source_texture_base_coords + offset.xy) / source_texture_size_f32, 0.0);
    color += textureSampleLevel(source_texture, source_texture_samp, (source_texture_base_coords + offset.yx) / source_texture_size_f32, 0.0);
    color += textureSampleLevel(source_texture, source_texture_samp, (source_texture_base_coords + offset.yy) / source_texture_size_f32, 0.0);

    textureStore(destination_texture, id.xy, color * 0.25);
}