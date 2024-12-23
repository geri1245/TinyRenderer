struct InstanceInput {
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,
    @location(9) normal_matrix_0: vec3<f32>,
    @location(10) normal_matrix_1: vec3<f32>,
    @location(11) normal_matrix_2: vec3<f32>,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
};

struct LightViewProj {
    view_proj: mat4x4<f32>,
    light_position_and_far_plane: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> light: LightViewProj;

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> @builtin(position) vec4<f32> {
    let world_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    let vertex_position = vec4<f32>(model.position, 1.0);
    var final_position = light.view_proj * world_matrix * vertex_position;
    // WGPU expects the cubemap textures to be correct when we look at the cube from the outside
    // When we are inside the cube (eg. baking the shadows for point lights), the textures are flipped on the
    // horizontal axis. This is the reason why we are flipping the baked shadow texture on the x axis here.
    // This way sampling the shadow cubemap in the lighting shader can be done without any extra effort
    // IMPORTANT: The face culling is set to Back faces on the pipeline, but because of this negative multiplier
    // this will actually be front face culling (face direction changes because of the negative scaling)
    final_position.x *= -1.0;

    return final_position;
}
