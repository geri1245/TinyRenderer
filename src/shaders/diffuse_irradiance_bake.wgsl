// Precomputes the diffuse irradiance map for a given cubemap and stores it in another cubemap
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
var t_cubemap: texture_cube<f32>;
@group(1)
@binding(1)
var s_cubemap: sampler;

const PI: f32 = 3.14159265359;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var irradiance = vec3(0.0);

    let guessed_up = vec3(0.0, 1.0, 0.0);

    // the sample direction equals the hemisphere's orientation
    let normal = normalize(input.local_position);
    let right = normalize(cross(guessed_up, normal));
    let up = normalize(cross(normal, right));

    let sampleDeltaPhi = 0.125;
    let sampleDeltaTheta = 0.025;
    var samplesTaken = 0.0;
    for (var phi: f32 = 0.0; phi < 2.0 * PI; phi += sampleDeltaPhi) {
        for (var theta: f32 = 0.0; theta < 0.5 * PI; theta += sampleDeltaTheta) {
            //spherical to cartesian (in tangent space)
            let tangentSample = vec3(sin(theta) * cos(phi), sin(theta) * sin(phi), cos(theta));
            
            // tangent space to world
            let sampleVec = tangentSample.x * right + tangentSample.y * up + tangentSample.z * normal;

            irradiance += textureSample(t_cubemap, s_cubemap, sampleVec).rgb * cos(theta) * sin(theta);
            samplesTaken += 1.0;
        }
    }

    irradiance = PI * irradiance * (1.0 / f32(samplesTaken));

    return vec4(irradiance, 1.0);
}
