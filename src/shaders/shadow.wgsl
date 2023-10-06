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
};

struct Light {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
    color: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> light: Light;

@vertex
fn vs_bake(
    model: VertexInput,
    instance: InstanceInput,) -> @builtin(position) vec4<f32> {
        let world_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    let vertex_position = vec4<f32>(model.position, 1.0);
    return light.view_proj * world_matrix * vertex_position;
}
