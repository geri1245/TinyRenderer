/// Renders into offscreen buffers:
/// Fills up the GBuffer, doesn't do any lighting calculations

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
    view: mat4x4<f32>,
    view_inv: mat4x4<f32>,
    proj: mat4x4<f32>,
    proj_inv: mat4x4<f32>,
    position: vec3<f32>,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec4<f32>,
    @location(2) tex_coord: vec2<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
};

@group(1) @binding(0)
var<uniform> camera: CameraUniform;

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
    out.world_position = model_matrix * vertex_position;

    out.clip_position = camera.view_proj * model_matrix * vertex_position;

    let tangent = normalize((normal_matrix * model.tangent).xyz);
    let bitangent = normalize((normal_matrix * model.bitangent).xyz);
    let normal = normalize((normal_matrix * model.normal).xyz);

    out.tangent = tangent;
    out.bitangent = bitangent;
    out.world_normal = normal;

    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;
@group(0) @binding(2)
var t_normal: texture_2d<f32>;
@group(0) @binding(3)
var s_normal: sampler;
@group(0) @binding(4)
var t_roughness: texture_2d<f32>;
@group(0) @binding(5)
var s_roughness: sampler;
@group(0) @binding(6)
var t_metalness: texture_2d<f32>;
@group(0) @binding(7)
var s_metalness: sampler;

struct GBufferOutput {
  @location(0) position: vec4<f32>,
  @location(1) normal: vec4<f32>,
  @location(2) albedo: vec4<f32>,
  @location(3) rough_metal_ao: vec4<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> GBufferOutput {
    var output: GBufferOutput;
    output.position = in.world_position;
    let tbn_mat = mat3x3(
        normalize(in.tangent),
        normalize(in.bitangent),
        normalize(in.world_normal),
    );

    let tangent_space_normal = 2.0 * textureSample(t_normal, s_normal, in.tex_coord).xyz - 1.0;
    output.normal = vec4(normalize(tbn_mat * tangent_space_normal), 1.0);
    output.albedo = textureSample(t_diffuse, s_diffuse, in.tex_coord);
    output.rough_metal_ao = vec4(
        textureSample(t_roughness, s_roughness, in.tex_coord).x,
        textureSample(t_metalness, s_metalness, in.tex_coord).x,
        1.0,
        0.0
    );

    return output;
}