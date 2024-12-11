struct SkyOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) viewDirection: vec3<f32>,
};

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    view_inv: mat4x4<f32>,
    proj: mat4x4<f32>,
    proj_inv: mat4x4<f32>,
    position: vec3<f32>,
};

@group(1) @binding(0)
var<uniform> camera: CameraUniform;

fn to_mat3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(m[0].xyz, m[1].xyz, m[2].xyz);
}

@vertex
fn vs_sky(@builtin(vertex_index) vertex_index: u32) -> SkyOutput {
    // hacky way to draw a large triangle
    let tmp1 = i32(vertex_index) / 2;
    let tmp2 = i32(vertex_index) & 1;
    let pos = vec4<f32>(
        f32(tmp1) * 4.0 - 1.0,
        f32(tmp2) * 4.0 - 1.0,
        0.0,
        1.0
    );

    let view_inv = to_mat3(camera.view_inv);
    let unprojected = view_inv * (camera.proj_inv * pos).xyz;

    var result: SkyOutput;
    result.viewDirection = unprojected;
    result.position = pos;
    return result;
}

@group(0)
@binding(0)
var r_texture: texture_cube<f32>;
@group(0)
@binding(1)
var r_sampler: sampler;

@fragment
fn fs_sky(vertex: SkyOutput) -> @location(0) vec4<f32> {
    return textureSample(r_texture, r_sampler, vertex.viewDirection);
}