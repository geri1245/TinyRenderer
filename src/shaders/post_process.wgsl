const MAX_ITERATIONS: u32 = 50u;

@group(0)
@binding(0)
var destination_texture: texture_storage_2d<rgba8unorm, write>;

@group(0) @binding(1)
var source_texture: texture_2d<f32>;
@group(0) @binding(2)
var source_texture_samp: sampler;

@compute
@workgroup_size(8,8,1)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let source_texture_size = vec2(textureDimensions(source_texture).x, textureDimensions(source_texture).y);
    
    // Check if we are not indexing out of our textures
    if any(id.xy > source_texture_size) { return; }

    let pixel_coords = vec2(i32(id.x), i32(id.y));
    let texture_coords = vec2(f32(id.x), f32(id.y)) / vec2<f32>(source_texture_size);

    let color = textureSampleLevel(source_texture, source_texture_samp, texture_coords, 0.0);
    textureStore(destination_texture, pixel_coords, color);
}