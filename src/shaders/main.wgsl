struct Light {
    view_proj: mat4x4<f32>,
    position_or_direction: vec3<f32>,
    light_type: i32,
    color: vec3<f32>,
    far_plane_distance: f32,
}

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    view_inv: mat4x4<f32>,
    proj: mat4x4<f32>,
    proj_inv: mat4x4<f32>,
    position: vec3<f32>,
};

struct LightParams {
    point_light_count: u32,
    directional_light_count: u32,
};

@group(0) @binding(0)
var<uniform> lights: array<Light, 2>;

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
var t_rough_metal_ao: texture_2d<f32>;
@group(2) @binding(7)
var s_rough_metal_ao: sampler;

@group(3) @binding(0)
var t_shadow: texture_depth_2d_array;
@group(3) @binding(1)
var sampler_shadow: sampler_comparison;
@group(3) @binding(2)
var t_shadow_cube: texture_depth_cube_array;
@group(3) @binding(3)
var sampler_cube: sampler_comparison;

@group(4)
@binding(0)
var destination_texture: texture_storage_2d<rgba16float, write>;

@group(4) @binding(1)
var screen_texture: texture_2d<f32>;
@group(4) @binding(2)
var screen_texture_samp: sampler;

@group(5) @binding(0)
var diffuse_irradiance_map: texture_cube<f32>;
@group(5) @binding(1)
var diffuse_irradiance_sampler: sampler;

@group(6) @binding(0)
var<uniform> light_params: LightParams;

fn is_valid_tex_coord(tex_coord: vec2<f32>) -> bool {
    return tex_coord.x >= 0.0 && tex_coord.x <= 1.0 && tex_coord.y >= 0.0 && tex_coord.y <= 1.0;
}

fn fetch_shadow(light_id: u32, fragment_pos2: vec4<f32>) -> f32 {
    var fragment_pos = fragment_pos2;
    fragment_pos.x *= -1.0;
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
        return textureSampleCompareLevel(t_shadow, sampler_shadow, tex_coord, 0, fragment_pos_ndc.z);
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
    return textureSampleCompareLevel(t_shadow_cube, sampler_cube, tex_coord, 0, vector_to_depth_value(tex_coord));
}

const c_ambient_strength: f32 = 0.0;
const c_light_attenuation_constant: f32 = 1.0;
const c_light_attenuation_linear: f32 = 0.01;
const c_light_attenuation_quadratic: f32 = 0.0005;

const F0_NON_METALLIC: vec3<f32> = vec3(0.04);
const PI: f32 = 3.14159265359;

fn fresnel_schlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (vec3(1.0) - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

fn fresnel_schlick_roughness(cosTheta: f32, F0: vec3<f32>, roughness: f32) -> vec3<f32> {
    return F0 + (max(vec3(1.0 - roughness), F0) - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
} 

fn distribution_ggx(normal: vec3<f32>, half_dir: vec3<f32>, roughness: f32) -> f32 {
    let rough_squared = roughness * roughness;
    let rough_4 = rough_squared * rough_squared;
    let n_dot_h = max(dot(normal, half_dir), 0.0);
    let n_dot_h_2 = n_dot_h * n_dot_h;

    var denom = (n_dot_h_2 * (rough_4 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return rough_4 / denom;
}

fn geometry_schlick_ggx(normal_dot_view: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = r * r / 8.0;

    let denom = normal_dot_view * (1.0 - k) + k;

    return normal_dot_view / denom;
}

fn geometery_smith(normal: vec3<f32>, view: vec3<f32>, light: vec3<f32>, roughness: f32) -> f32 {
    let normal_dot_view = max(dot(normal, view), 0.0);
    let normal_dot_light = max(dot(normal, light), 0.0);
    let ggx2 = geometry_schlick_ggx(normal_dot_view, roughness);
    let ggx1 = geometry_schlick_ggx(normal_dot_light, roughness);

    return ggx1 * ggx2;
}

fn calculate_light_contribution(
    pixel_to_light: vec3<f32>, light_color: vec3<f32>, attenuation: f32, pixel_to_camera: vec3<f32>, pixel_position: vec3<f32>, normal: vec3<f32>, albedo: vec3<f32>, metalness: f32, roughness: f32
) -> vec3<f32> {
    let half_dir = normalize(pixel_to_camera + pixel_to_light);
    let radiance = light_color * attenuation;

    let F0 = mix(F0_NON_METALLIC, albedo, metalness);
    let F = fresnel_schlick(max(dot(half_dir, pixel_to_camera), 0.0), F0);

    let NDF = distribution_ggx(normal, half_dir, roughness);
    let G = geometery_smith(normal, pixel_to_camera, pixel_to_light, roughness);

    let normal_dot_light = max(dot(normal, pixel_to_light), 0.0);

    let numerator = NDF * G * F;
    let denominator = 4.0 * max(dot(normal, pixel_to_camera), 0.0) * normal_dot_light + 0.0001;
    let specular = numerator / denominator;

    let kS = F;
    let kD = (vec3(1.0) - kS) * (1.0 - metalness);

    let light_contribution = (kD * albedo / PI + specular) * radiance * normal_dot_light;

    return light_contribution;
}

fn get_diffuse_irradiance(normal: vec3<f32>, view: vec3<f32>, roughness: f32, albedo: vec3<f32>, metalness: f32) -> vec3<f32> {
    let F0 = mix(F0_NON_METALLIC, albedo, metalness);
    let specular_reflection_portion = fresnel_schlick_roughness(max(dot(normal, view), 0.0), F0, roughness);
    let diffuse_reflection_portion = 1.0 - specular_reflection_portion;

    let all_irradiance = textureSampleLevel(diffuse_irradiance_map, diffuse_irradiance_sampler, normal, 0.0).rgb;
    let diffuse_irradiance = diffuse_reflection_portion * all_irradiance;

    return diffuse_irradiance * albedo;
}

@compute
@workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let destination_texture_size = vec2(textureDimensions(destination_texture).x, textureDimensions(destination_texture).y);
    
    // Check if we are not indexing out of our textures
    if any(id.xy > destination_texture_size) { return; }

    let uv = vec2(f32(id.x), f32(id.y)) / vec2<f32>(destination_texture_size);

    let normal = normalize(textureSampleLevel(t_normal, s_normal, uv, 0.0).xyz);
    let albedo_and_shininess = textureSampleLevel(t_albedo, s_albedo, uv, 0.0);
    let albedo = albedo_and_shininess.xyz;
    let position = textureSampleLevel(t_position, s_position, uv, 0.0);
    let rough_metal_ao = textureSampleLevel(t_rough_metal_ao, s_rough_metal_ao, uv, 0.0);
    let roughness = rough_metal_ao.x;
    let metalness = rough_metal_ao.y;
    let ambient_occlusion = rough_metal_ao.z;
    let pixel_to_camera = normalize(camera.position.xyz - position.xyz);

    var irradiance = vec3<f32>(0, 0, 0);

    // Point lights
    for (var i = 0u; i < light_params.point_light_count; i += 1u) {
        let light = lights[i];

        let shadow = get_shadow_value(i, position.xyz);
        if shadow > 0.0 {
            let pixel_to_light = light.position_or_direction - position.xyz;
            let pixel_to_light_distance = length(pixel_to_light);
            let attenuation = 1.0 / (pixel_to_light_distance * pixel_to_light_distance);

            irradiance += calculate_light_contribution(
                normalize(pixel_to_light), light.color, attenuation, pixel_to_camera, position.xyz, normal, albedo, metalness, roughness
            );
        }
    }

    // Directional lights
    for (var i = 0u; i < light_params.directional_light_count; i += 1u) {
        let light = lights[i + light_params.point_light_count];

        let shadow = fetch_shadow(i, light.view_proj * position);
        if shadow > 0.0 {
            irradiance += calculate_light_contribution(
                -light.position_or_direction, light.color, 1.0, pixel_to_camera, position.xyz, normal, albedo, metalness, roughness
            );
        }
    }

    let diffuse_ambient_light = get_diffuse_irradiance(normal, pixel_to_camera, roughness, albedo, metalness);
    let ambient = diffuse_ambient_light * ambient_occlusion;

    let finalHdrColor = ambient + irradiance;

    let pixel_coords = vec2(i32(id.x), i32(id.y));
    textureStore(destination_texture, pixel_coords, vec4(finalHdrColor, 1));
}
