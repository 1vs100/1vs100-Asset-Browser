// UI shader for rendering 2D UI elements

struct UiVertexInput {
    @builtin(vertex_index) vertex_index: u32,
};

struct UiVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

// Full-screen quad vertices (2 triangles = 6 vertices)
const QUAD_POSITIONS = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>( 1.0,  1.0),
    vec2<f32>(-1.0,  1.0),
);

const QUAD_UVS = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 0.0),
);

@vertex
fn vs_main(input: UiVertexInput) -> UiVertexOutput {
    var output: UiVertexOutput;
    output.clip_position = vec4<f32>(QUAD_POSITIONS[input.vertex_index], 0.0, 1.0);
    output.uv = QUAD_UVS[input.vertex_index];
    output.color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    return output;
}

@group(0) @binding(0)
var ui_texture: texture_2d<f32>;

@group(0) @binding(1)
var ui_sampler: sampler;

@fragment
fn fs_main(input: UiVertexOutput) -> @location(0) vec4<f32> {
    // For now, just render the texture
    let color = textureSample(ui_texture, ui_sampler, input.uv);
    return color * input.color;
}