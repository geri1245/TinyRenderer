struct GlobalGpuParams {
    random_parameter: f32,
    tone_mapping_type: u32,
    ssr_thickness: f32,
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
const max_iterations: i32 = 256;
const max_thickness: f32 = 0.05;

fn world_space_to_texture_space(world_space: vec3<f32>, view_proj_mat: mat4x4<f32>) -> vec3<f32> {
    let clip_space = view_proj_mat * vec4(world_space, 1.0);
    let ndc_space = clip_space.xyz / clip_space.w;
    return vec3(ndc_space.xy * vec2(0.5, -0.5) + 0.5, ndc_space.z);
}

// _ts means texture space. The range is [0, 1] and the domain is the full screen source_texture
// _ws means world space

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
    let texture_coords = vec2<f32>(id.xy) / vec2<f32>(source_texture_size);
    // let color = textureSampleLevel(depth_texture, depth_sampler, texture_coords, 0.0);

    let normal = normalize(textureSampleLevel(gbuffer_normal_texture, gbuffer_normal_sampler, texture_coords, 0.0).xyz);
    var source_color = textureSampleLevel(source_texture, source_texture_samp, texture_coords, 0.0).xyz;

    let reflection_start_pos_ws = textureSampleLevel(gbuffer_position_texture, gbuffer_position_sampler, texture_coords, 0.0).xyz;
    let reflection_start_pos_ts = world_space_to_texture_space(reflection_start_pos_ws, camera.view_proj);

    let reflection_direction = normalize(reflect(normalize(reflection_start_pos_ws - camera.position), normal));
    let reflection_end_pos_ws = reflection_start_pos_ws + max_search_distance * reflection_direction;
    let reflection_end_pos_ts = world_space_to_texture_space(reflection_end_pos_ws, camera.view_proj);

    let reflection_ray_ts = reflection_end_pos_ts - reflection_start_pos_ts;

    // If the distance is really far away, then go at most step_count steps, but don't step less than a single pixel
    let single_pixel_diff = vec2(1.0) / vec2<f32>(source_texture_size);
    var increment = max(single_pixel_diff.x, abs(reflection_ray_ts.x) / f32(max_iterations));
    var quotient = 1 / abs(reflection_ray_ts.x / increment);
    if abs(reflection_ray_ts.x) < abs(reflection_ray_ts.y) {
        increment = max(single_pixel_diff.y, abs(reflection_ray_ts.y) / f32(max_iterations));
        quotient = 1 / abs(reflection_ray_ts.y / increment);
    }

    var hit_pos = vec3(-1.0);
    for (var progress = 0.01; progress < 1.0; progress += quotient) {
        let marched_ray_pos = mix(reflection_start_pos_ts, reflection_end_pos_ts, progress);
        if any(marched_ray_pos.xy > vec2(1.0)) || any(marched_ray_pos.xy < vec2(0.0)) {
            break;
        }
        let actual_depth = textureSampleLevel(depth_texture, depth_sampler, marched_ray_pos.xy, 0.0);

        let distance = marched_ray_pos.z - actual_depth;
        if distance >= 0 && distance < global_gpu_params.ssr_thickness / 10000.0 {
            hit_pos = marched_ray_pos;
            break;
        }
    }

    var color = vec3(0.0);
    if all(hit_pos > vec3(-1.0)) && all(hit_pos < vec3(1.0)) {
        let reflection_color = textureSampleLevel(source_texture, source_texture_samp, hit_pos.xy, 0.0).xyz;
        color = mix(source_color, reflection_color, 0.5);
    } else {
        // color.x = 1.0;
        color = source_color;
    }

    // let color_overrid = textureSampleLevel(gbuffer_albedo_texture, gbuffer_albedo_sampler, texture_coords, 0.0).xyz;
    let color_overrid = vec3(increment * 100.0);

    textureStore(destination_texture, id.xy, vec4<f32>(source_color, 1.0));
}