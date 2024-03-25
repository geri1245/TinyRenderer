struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> viewproj: mat4x4<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_position: vec3<f32>,
};

@vertex
fn vs_main(
    vertex_in: VertexInput,
) -> VertexOutput {
    let vertex_position = vec4<f32>(vertex_in.position, 1.0);
    var final_position = viewproj * vertex_position;
    // WGPU expects the cubemap textures to be correct when we look at the cube from the outside
    // When we are inside the cube (eg. creating the environment map), the textures are flipped on the
    // horizontal axis. This is the reason why we are flipping the texture on the x axis here.
    // This way sampling the result cubemap in the lighting shader can be done without any extra effort
    final_position.x *= -1.0;
    // final_position.y *= -1.0;

    var output: VertexOutput;
    output.clip_position = final_position;
    output.local_position = vertex_in.position;

    return output;
}

@group(1)
@binding(0)
var t_equirectangular: texture_2d<f32>;
@group(1)
@binding(1)
var s_equirectangular: sampler;

const invAtan: vec2<f32> = vec2(0.1591, 0.3183);

fn sample_spherical_map(v: vec3<f32>) -> vec2<f32> {
    var uv = vec2(atan2(v.z, v.x), asin(v.y));
    uv *= invAtan;
    uv += 0.5;
    uv.y = 1.0 - uv.y;
    return uv;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = sample_spherical_map(normalize(input.local_position));
    let color = textureSample(t_equirectangular, s_equirectangular, uv).rgb;

    return vec4(color, 1.0);
}
