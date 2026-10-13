// Scene shader for rendering 3D meshes with textures

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world_pos = input.position;
    output.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    output.world_pos = world_pos;
    output.normal = input.normal;
    output.uv = input.uv;
    return output;
}

@group(1) @binding(0)
var base_texture: texture_2d<f32>;

@group(1) @binding(1)
var base_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(base_texture, base_sampler, input.uv);
    
    // Simple lighting
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let n = normalize(input.normal);
    let diffuse = max(dot(n, light_dir), 0.0);
    let ambient = 0.3;
    let lighting = ambient + diffuse * 0.7;
    
    return vec4<f32>(color.rgb * lighting, color.a);
}