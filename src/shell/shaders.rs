// Obsidian Glass depth-of-field shaders
pub const DEPTH_OF_FIELD_SHADER: &str = r#"
@group(0) @binding(0) var t_color: texture_2d<f32>;
@group(0) @binding(1) var s_sampler: sampler;

@fragment
fn fs_main(@location(0) tex_coords: vec2<f32>) -> @location(0) vec4<f32> {
    // Sample the color and apply glass-like tint and blur
    let color = textureSample(t_color, s_sampler, tex_coords);
    return vec4<f32>(color.rgb * 0.9, 0.85); 
}
"#;
