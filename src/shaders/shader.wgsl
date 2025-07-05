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

// Distance field functions for smooth shapes
fn sdfLine(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

fn sdfCircle(p: vec2<f32>, center: vec2<f32>, radius: f32) -> f32 {
    return length(p - center) - radius;
}

// Bloom effect function
fn bloom(dist: f32, intensity: f32, radius: f32) -> f32 {
    return intensity * exp(-dist * dist / (radius * radius));
}

// HSV to RGB conversion for dynamic colors
fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

// Fragment shader
@fragment
fn fs_main(@builtin(position) fragCoord: vec4<f32>) -> @location(0) vec4<f32> {
    // Convert fragCoord to UV coordinates with explicit bottom-to-top mapping
    let uv = vec2<f32>(
        (fragCoord.x / uniforms.resolution.x - 0.5) * (uniforms.resolution.x / uniforms.resolution.y),
        (uniforms.resolution.y - fragCoord.y) / uniforms.resolution.y - 0.5
    );
    let aspect = uniforms.resolution.x / uniforms.resolution.y;

    var final_color = vec3<f32>(0.05, 0.05, 0.1); // Dark blue background
    let time = uniforms.time;

    // Draw frequency bars as lines with circles and bloom
    for (var i = 0; i < i32(uniforms.bin_size); i++) {
        let bar_index = i;
        if (bar_index >= i32(uniforms.bin_size)) {
            break;
        }

        // Get amplitude for this bar
        let vec4_index = bar_index / 4;
        let component_index = bar_index % 4;
        let raw_amplitude = uniforms.frequency_bars[vec4_index][component_index];
        let amplitude = clamp(raw_amplitude * 2.0, 0.0, 1.0);

        // Skip if amplitude is too low
        // if amplitude < 0.01 {
        //     continue;
        // }

        // Calculate line position (from bottom to top)
        let x_pos = (f32(bar_index) / uniforms.bin_size - 0.5) * aspect;
        let min_height = 0.05; // 5% minimum height
        let max_height = 0.8; // 80% maximum height
        let actual_amplitude = min_height + amplitude * (max_height - min_height);
        let line_start = vec2<f32>(x_pos, -0.5);  // Bottom of screen
        let line_end = vec2<f32>(x_pos, -0.5 + actual_amplitude);  // Grow upward

        // Calculate circle position at top of line
        let circle_center = line_end;
        let circle_radius = 0.02;

        // Dynamic color based on frequency and amplitude
        let freq_ratio = f32(bar_index) / uniforms.bin_size;
        let hue = freq_ratio * 0.8 + time * 0.1; // Slowly rotating hue
        let saturation = 0.8 + amplitude * 0.2;
        let brightness = 0.5 + amplitude * 0.5;
        let base_color = hsv2rgb(vec3<f32>(hue, saturation, brightness));

        // Line distance and rendering
        let line_dist = sdfLine(uv, line_start, line_end);
        let line_thickness = 0.003 + amplitude * 0.001;
        let line_alpha = smoothstep(line_thickness + 0.001, line_thickness, line_dist);

        // Circle distance and rendering
        let circle_dist = sdfCircle(uv, circle_center, circle_radius);
        let circle_alpha = smoothstep(0.001, 0.0, circle_dist);

        // Toned down bloom effects
        let bloom_radius = 0.02 + amplitude * 0.03;
        let bloom_intensity = amplitude * 0.8;

        // Subtle line bloom
        let line_bloom = bloom(line_dist, bloom_intensity * 0.2, bloom_radius * 0.5);

        // Single circle bloom layer
        let circle_bloom = bloom(circle_dist, bloom_intensity * 0.5, bloom_radius);

        // Combine effects with reduced bloom
        let total_alpha = line_alpha + circle_alpha + line_bloom * 0.3 + circle_bloom * 0.5;

        // Add color with additive blending
        final_color += base_color * total_alpha;

        // Subtle sparkle for high frequencies
        if freq_ratio > 0.7 && amplitude > 0.5 {
            let sparkle_dist = length(uv - circle_center);
            let sparkle = amplitude * exp(-sparkle_dist * 30.0) * (sin(time * 8.0 + f32(bar_index)) * 0.5 + 0.5);
            final_color += vec3<f32>(1.0, 1.0, 0.8) * sparkle * 0.2;
        }
    }

    // Add subtle background glow based on overall energy
    var total_energy = 0.0;
    for (var i = 0; i < i32(uniforms.bin_size); i++) {
        let vec4_index = i / 4;
        let component_index = i % 4;
        total_energy += uniforms.frequency_bars[vec4_index][component_index];
    }
    total_energy /= uniforms.bin_size;

    // Subtle background glow
    let center_dist = length(uv);
    let bg_glow = total_energy * exp(-center_dist * 3.0) * 0.03;
    final_color += vec3<f32>(0.1, 0.05, 0.15) * bg_glow;

    // Apply tone mapping and gamma correction
    // final_color = final_color / (final_color + vec3<f32>(1.0));
    // final_color = pow(final_color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(final_color, 1.0);
}
