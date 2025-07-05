// Vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertexIndex: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0)
    );
    return vec4<f32>(pos[vertexIndex], 0.0, 1.0);
}

// Uniforms (16-byte aligned for WebGL compatibility)
struct Uniforms {
    time: f32,
    bin_size: f32,
    resolution: vec2<f32>,
    frequency_bars: array<vec4<f32>, 16>, // 64 floats as 16 vec4s for proper alignment
}
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

// Fragment shader
@fragment
fn fs_main(@builtin(position) fragCoord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = fragCoord.xy / uniforms.resolution;
    
    // Calculate which frequency bar this pixel belongs to
    let bar_index = i32(uv.x * uniforms.bin_size);
    
    // Bounds check to prevent out-of-bounds access
    if (bar_index >= i32(uniforms.bin_size) || bar_index < 0) {
        return vec4<f32>(0.1, 0.1, 0.1, 1.0); // Dark background for invalid indices
    }
    
    // Get the height of this bar (normalized 0-1)
    let vec4_index = bar_index / 4;
    let component_index = bar_index % 4;
    let raw_amplitude = uniforms.frequency_bars[vec4_index][component_index];
    // Apply power scaling for dramatic height differences
    let clamped_amplitude = clamp(raw_amplitude, 0.0, 1.0);
    
    // Different power scaling based on frequency range for more variance
    let freq_ratio = f32(bar_index) / uniforms.bin_size;
    let power_factor = if freq_ratio < 0.25 {
        3.0 // Bass: Strong power scaling
    } else if freq_ratio < 0.75 {
        4.0 // Mid: Maximum power scaling for drama
    } else {
        2.5 // Treble: Moderate power scaling
    };
    
    let powered_height = pow(clamped_amplitude, 1.0 / power_factor);
    let bar_height = clamp(powered_height * 0.9, 0.0, 1.0);
    
    // Create bar visualization
    let bar_x = f32(bar_index) / uniforms.bin_size;
    let bar_width = 1.0 / uniforms.bin_size;
    let bar_center = bar_x + bar_width * 0.5;
    
    // Check if we're inside a bar
    let is_in_bar = uv.x >= bar_x && uv.x < bar_x + bar_width;
    
    // Bar visualization from bottom up
    let bar_fill = (1.0 - uv.y) < bar_height;
    
    // Color based on frequency (low=red, mid=green, high=blue)
    let freq_ratio = f32(bar_index) / uniforms.bin_size;
    let r = mix(1.0, 0.0, freq_ratio);
    let g = sin(freq_ratio * 3.14159) * 2.0;
    let b = freq_ratio;
    
    // Show bars with color, background as dark
    if is_in_bar && bar_fill {
        return vec4<f32>(r, g, b, 1.0);
    } else {
        return vec4<f32>(0.1, 0.1, 0.1, 1.0); // Dark background
    }
}
