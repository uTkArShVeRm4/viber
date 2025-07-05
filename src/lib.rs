use wasm_bindgen::prelude::*;
use web_sys::console;
use std::io::Cursor;
use phastft::planner::Direction;

mod renderer;
use renderer::Renderer;

// A macro to provide `println!(..)`-style syntax for `console.log` logging.
macro_rules! log {
    ( $( $t:tt )* ) => {
        console::log_1(&format!( $( $t )* ).into());
    }
}

#[wasm_bindgen]
pub struct App {
    renderer: Renderer,
    audio_frames: Vec<Vec<f32>>,
    fft_results: Vec<Vec<f32>>,
    frequency_bars: Vec<Vec<f32>>,
    previous_bars: Vec<f32>,
    audio_processed: bool,
    bin_size: usize,
}

#[wasm_bindgen]
impl App {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        log!("Initializing music visualizer...");

        Self {
            renderer: Renderer::new(),
            audio_frames: Vec::new(),
            fft_results: Vec::new(),
            frequency_bars: Vec::new(),
            previous_bars: vec![0.0; 64],
            audio_processed: false,
            bin_size: 64,
        }
    }

    #[wasm_bindgen]
    pub async fn init(&mut self, canvas_id: &str) -> Result<(), JsValue> {
        self.renderer.init(canvas_id).await?;
        Ok(())
    }

    #[wasm_bindgen]
    pub fn render(&mut self, time: f64, frame_index: usize, smoothing_factor: f32) {
        let bin_size = self.bin_size;
        
        if self.audio_processed {
            let target_bars = if frame_index < self.frequency_bars.len() {
                self.frequency_bars[frame_index].clone()
            } else {
                vec![0.0; bin_size]
            };
            let smoothed_bars = self.smooth_interpolate(&target_bars, smoothing_factor);
            self.renderer.render(time, &smoothed_bars, bin_size);
        } else {
            // Render empty bars or default animation when no audio is loaded
            let empty_bars = vec![0.0; bin_size];
            self.renderer.render(time, &empty_bars, bin_size);
        }
    }

    #[wasm_bindgen]
    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    #[wasm_bindgen]
    pub fn get_frequency_bars(&self, frame_index: usize) -> Vec<f32> {
        if self.audio_processed && frame_index < self.frequency_bars.len() {
            self.frequency_bars[frame_index].clone()
        } else {
            vec![0.0; self.bin_size] // Return empty bars if index out of bounds or no audio processed
        }
    }

    #[wasm_bindgen]
    pub fn get_total_frames(&self) -> usize {
        if self.audio_processed {
            self.frequency_bars.len()
        } else {
            0
        }
    }

    #[wasm_bindgen]
    pub fn set_bin_size(&mut self, bin_size: usize) {
        self.bin_size = bin_size;
        self.previous_bars = vec![0.0; bin_size];
    }

    #[wasm_bindgen]
    pub fn process_audio_file(&mut self, file_data: &[u8]) -> Result<(), JsValue> {
        log!("Processing audio file, size: {} bytes", file_data.len());
        
        // Create a cursor from the byte data
        let cursor = Cursor::new(file_data);
        
        // Try to read the WAV file
        match hound::WavReader::new(cursor) {
            Ok(reader) => {
                let spec = reader.spec();
                log!("WAV file info:");
                log!("  Channels: {}", spec.channels);
                log!("  Sample rate: {} Hz", spec.sample_rate);
                log!("  Bits per sample: {}", spec.bits_per_sample);
                log!("  Sample format: {:?}", spec.sample_format);
                log!("  Duration: {:.2} seconds", reader.duration() as f64 / spec.sample_rate as f64);
                
                // Read all samples
                let samples: Result<Vec<i16>, _> = reader.into_samples().collect();
                match samples {
                    Ok(sample_vec) => {
                        log!("Total samples: {}", sample_vec.len());
                        
                        // Convert to mono if stereo (take left channel only)
                        let mono_samples = if spec.channels == 2 {
                            sample_vec.iter().step_by(2).cloned().collect::<Vec<i16>>()
                        } else {
                            sample_vec
                        };
                        
                        log!("Mono samples: {}", mono_samples.len());
                        
                        // Process audio with framing and windowing
                        self.process_audio_frames(&mono_samples);
                        
                        // Process FFT on windowed frames
                        self.process_fft();
                        
                        // Map FFT results to frequency bars
                        self.map_to_frequency_bars(spec.sample_rate);
                        
                        // Mark audio as processed
                        self.audio_processed = true;
                        log!("Audio processing complete! Ready for visualization.");
                        
                        Ok(())
                    }
                    Err(e) => {
                        log!("Error reading samples: {:?}", e);
                        Err(JsValue::from_str(&format!("Failed to read samples: {:?}", e)))
                    }
                }
            }
            Err(e) => {
                log!("Error reading WAV file: {:?}", e);
                Err(JsValue::from_str(&format!("Failed to read WAV file: {:?}", e)))
            }
        }
    }

    fn process_audio_frames(&mut self, samples: &[i16]) {
        const FRAME_SIZE: usize = 1024;
        const TARGET_FPS: f64 = 120.0;
        const SAMPLE_RATE: f64 = 44100.0;
        
        // Calculate hop size for 120fps synchronization
        let duration_seconds = samples.len() as f64 / SAMPLE_RATE;
        let target_frames = (duration_seconds * TARGET_FPS) as usize;
        let hop_size = if target_frames > 0 {
            samples.len() / target_frames
        } else {
            FRAME_SIZE
        };
        
        // Calculate number of frames with calculated hop size
        let frame_count = if samples.len() >= FRAME_SIZE {
            (samples.len() - FRAME_SIZE) / hop_size + 1
        } else {
            0
        };
        
        log!("Audio duration: {:.2} seconds", duration_seconds);
        log!("Target frames for 60fps: {}", target_frames);
        log!("Calculated hop size: {} samples", hop_size);
        log!("Processing {} frames (hop size: {})", frame_count, hop_size);
        
        // Generate Hann window
        let hann_window = self.generate_hann_window(FRAME_SIZE);
        
        // Clear previous audio frames
        self.audio_frames.clear();
        
        // Process each frame with calculated hop size
        for frame_idx in 0..frame_count {
            let start_idx = frame_idx * hop_size;
            let end_idx = start_idx + FRAME_SIZE;
            
            if end_idx <= samples.len() {
                let frame = &samples[start_idx..end_idx];
                let windowed_frame = self.apply_hann_window(frame, &hann_window);
                
                // Store the windowed frame
                self.audio_frames.push(windowed_frame);
                
                // Log first frame details for debugging
                if frame_idx == 0 {
                    log!("First frame raw samples (first 10): {:?}", &frame[..10]);
                    log!("First frame windowed samples (first 10): {:?}", &self.audio_frames[0][..10]);
                }
            }
        }
        
        log!("Stored {} windowed frames for 120fps visualization", self.audio_frames.len());
    }
    
    fn process_fft(&mut self) {
        log!("Starting FFT processing on {} frames", self.audio_frames.len());
        
        // Clear previous FFT results
        self.fft_results.clear();
        
        for (frame_idx, frame) in self.audio_frames.iter().enumerate() {
            // Prepare data for FFT (real and imaginary parts)
            let mut real_data: Vec<f32> = frame.clone();
            let mut imag_data: Vec<f32> = vec![0.0; frame.len()];
            
            // Perform FFT
            phastft::fft_32(&mut real_data, &mut imag_data, Direction::Forward);
            
            // Calculate magnitudes (sqrt(real^2 + imag^2))
            let magnitudes: Vec<f32> = real_data.iter()
                .zip(imag_data.iter())
                .map(|(r, i)| (r * r + i * i).sqrt())
                .collect();
            
            // Log first frame FFT results for debugging
            if frame_idx == 0 {
                log!("First frame FFT magnitudes (first 10): {:?}", &magnitudes[..10]);
                log!("First frame FFT magnitudes (bins 100-110): {:?}", &magnitudes[100..110]);
                
                // Find peak frequency
                let max_magnitude = magnitudes.iter().fold(0.0f32, |a, &b| a.max(b));
                let max_index = magnitudes.iter().position(|&x| x == max_magnitude).unwrap_or(0);
                log!("Peak frequency bin: {}, magnitude: {:.2}", max_index, max_magnitude);
                
                // Log some frequency range statistics
                let low_freq_sum: f32 = magnitudes[0..50].iter().sum();
                let mid_freq_sum: f32 = magnitudes[50..200].iter().sum();
                let high_freq_sum: f32 = magnitudes[200..512].iter().sum();
                log!("Frequency range energies - Low (0-50): {:.2}, Mid (50-200): {:.2}, High (200-512): {:.2}", 
                     low_freq_sum, mid_freq_sum, high_freq_sum);
            }
            
            // Store magnitudes
            self.fft_results.push(magnitudes);
        }
        
        log!("FFT processing complete. Generated {} FFT results", self.fft_results.len());
    }
    
    fn map_to_frequency_bars(&mut self, sample_rate: u32) {
        let num_bars = self.bin_size;
        const MIN_FREQ: f32 = 20.0;    // 20 Hz
        const MAX_FREQ: f32 = 20000.0; // 20 kHz
        
        log!("Mapping FFT results to {} logarithmic frequency bars", num_bars);
        log!("Frequency range: {:.1} Hz to {:.1} Hz", MIN_FREQ, MAX_FREQ);
        
        // Generate logarithmic frequency boundaries
        let freq_boundaries = self.generate_log_frequencies(MIN_FREQ, MAX_FREQ, num_bars);
        
        // Log some frequency ranges for debugging (perceptual distribution)
        log!("Perceptual frequency distribution:");
        match num_bars {
            64 => {
                log!("  Bins 0-3: Sub-bass (20-100 Hz)");
                log!("  Bins 4-23: Bass (100-500 Hz)");
                log!("  Bins 24-47: Mid-range (500-4000 Hz)");
                log!("  Bins 48-63: High frequencies (4000-20000 Hz)");
            }
            32 => {
                log!("  Bins 0-1: Sub-bass (20-100 Hz)");
                log!("  Bins 2-11: Bass (100-500 Hz)");
                log!("  Bins 12-23: Mid-range (500-4000 Hz)");
                log!("  Bins 24-31: High frequencies (4000-20000 Hz)");
            }
            16 => {
                log!("  Bin 0: Sub-bass (20-100 Hz)");
                log!("  Bins 1-5: Bass (100-500 Hz)");
                log!("  Bins 6-11: Mid-range (500-4000 Hz)");
                log!("  Bins 12-15: High frequencies (4000-20000 Hz)");
            }
            _ => {
                log!("  Using logarithmic distribution");
            }
        }
        for i in 0..5.min(num_bars) {
            log!("  Bar {}: {:.1} Hz - {:.1} Hz", i, freq_boundaries[i], freq_boundaries[i + 1]);
        }
        
        // Clear previous frequency bars
        self.frequency_bars.clear();
        
        // Map each FFT frame to frequency bars
        for (frame_idx, fft_frame) in self.fft_results.iter().enumerate() {
            let bars = self.map_fft_to_bars(fft_frame, sample_rate, &freq_boundaries, num_bars);
            self.frequency_bars.push(bars);
            
            // Log first frame for debugging
            if frame_idx == 0 {
                let log_end = (10).min(self.frequency_bars[0].len());
                log!("First frame frequency bars (first {}): {:?}", log_end, &self.frequency_bars[0][..log_end]);
                
                // Find peak bar
                let max_bar = self.frequency_bars[0].iter().fold(0.0f32, |a, &b| a.max(b));
                let max_bar_idx = self.frequency_bars[0].iter().position(|&x| x == max_bar).unwrap_or(0);
                if max_bar_idx < freq_boundaries.len() - 1 {
                    log!("Peak bar: {} (freq range: {:.1} Hz - {:.1} Hz), magnitude: {:.2}", 
                         max_bar_idx, freq_boundaries[max_bar_idx], freq_boundaries[max_bar_idx + 1], max_bar);
                }
            }
        }
        
        log!("Frequency bar mapping complete. Generated {} bar frames", self.frequency_bars.len());
    }
    
    fn generate_log_frequencies(&self, min_freq: f32, max_freq: f32, num_bars: usize) -> Vec<f32> {
        let mut frequencies = Vec::with_capacity(num_bars + 1);
        
        // Perceptual frequency distribution strategy
        // More resolution in mid-range where music content is dense
        match num_bars {
            64 => {
                // Sub-bass (20-100Hz): 4 bins
                for i in 0..=4 {
                    let freq = 20.0 + (i as f32 / 4.0) * 80.0;
                    frequencies.push(freq);
                }
                // Bass (100-500Hz): 20 bins  
                for i in 1..=20 {
                    let freq = 100.0 * (500.0f32 / 100.0f32).powf(i as f32 / 20.0);
                    frequencies.push(freq);
                }
                // Mid-range (500-4000Hz): 24 bins
                for i in 1..=24 {
                    let freq = 500.0 * (4000.0f32 / 500.0f32).powf(i as f32 / 24.0);
                    frequencies.push(freq);
                }
                // High frequencies (4000-20000Hz): 16 bins
                for i in 1..=16 {
                    let freq = 4000.0 * (20000.0f32 / 4000.0f32).powf(i as f32 / 16.0);
                    frequencies.push(freq);
                }
            }
            32 => {
                // Sub-bass (20-100Hz): 2 bins
                for i in 0..=2 {
                    let freq = 20.0 + (i as f32 / 2.0) * 80.0;
                    frequencies.push(freq);
                }
                // Bass (100-500Hz): 10 bins
                for i in 1..=10 {
                    let freq = 100.0 * (500.0f32 / 100.0f32).powf(i as f32 / 10.0);
                    frequencies.push(freq);
                }
                // Mid-range (500-4000Hz): 12 bins
                for i in 1..=12 {
                    let freq = 500.0 * (4000.0f32 / 500.0f32).powf(i as f32 / 12.0);
                    frequencies.push(freq);
                }
                // High frequencies (4000-20000Hz): 8 bins
                for i in 1..=8 {
                    let freq = 4000.0 * (20000.0f32 / 4000.0f32).powf(i as f32 / 8.0);
                    frequencies.push(freq);
                }
            }
            16 => {
                // Sub-bass (20-100Hz): 1 bin
                frequencies.push(20.0);
                frequencies.push(100.0);
                // Bass (100-500Hz): 5 bins
                for i in 1..=5 {
                    let freq = 100.0 * (500.0f32 / 100.0f32).powf(i as f32 / 5.0);
                    frequencies.push(freq);
                }
                // Mid-range (500-4000Hz): 6 bins
                for i in 1..=6 {
                    let freq = 500.0 * (4000.0f32 / 500.0f32).powf(i as f32 / 6.0);
                    frequencies.push(freq);
                }
                // High frequencies (4000-20000Hz): 4 bins
                for i in 1..=4 {
                    let freq = 4000.0 * (20000.0f32 / 4000.0f32).powf(i as f32 / 4.0);
                    frequencies.push(freq);
                }
            }
            _ => {
                // Fallback to logarithmic distribution
                let log_min = min_freq.ln();
                let log_max = max_freq.ln();
                let log_step = (log_max - log_min) / num_bars as f32;
                
                for i in 0..=num_bars {
                    let freq = (log_min + i as f32 * log_step).exp();
                    frequencies.push(freq);
                }
            }
        }
        
        frequencies
    }
    
    fn map_fft_to_bars(&self, fft_frame: &[f32], sample_rate: u32, freq_boundaries: &[f32], num_bars: usize) -> Vec<f32> {
        let mut bars = vec![0.0; num_bars];
        
        if freq_boundaries.len() < num_bars + 1 {
            log!("Warning: insufficient frequency boundaries for {} bars", num_bars);
            return bars;
        }
        
        let freq_resolution = sample_rate as f32 / 1024.0; // 1024 is FFT size
        let nyquist_bin = 512; // Only use first half of FFT (Nyquist frequency)
        
        // First pass: collect raw magnitudes
        let mut raw_magnitudes = vec![0.0; num_bars];
        for bar_idx in 0..num_bars {
            let freq_start = freq_boundaries[bar_idx];
            let freq_end = freq_boundaries[bar_idx + 1];
            
            // Convert frequencies to bin indices
            let bin_start = ((freq_start / freq_resolution) as usize).min(nyquist_bin);
            let bin_end = ((freq_end / freq_resolution) as usize).min(nyquist_bin);
            
            // Ensure bin_end is at least bin_start
            let bin_end = bin_end.max(bin_start);
            
            // Sum magnitudes in this frequency range
            let mut magnitude_sum = 0.0;
            let mut bin_count = 0;
            
            for bin_idx in bin_start..=bin_end {
                if bin_idx < nyquist_bin && bin_idx < fft_frame.len() {
                    magnitude_sum += fft_frame[bin_idx];
                    bin_count += 1;
                }
            }
            
            raw_magnitudes[bar_idx] = if bin_count > 0 {
                magnitude_sum / bin_count as f32
            } else {
                0.0
            };
        }
        
        // Apply dynamic range compression and power expansion for better variance
        self.apply_dynamic_scaling(&raw_magnitudes, &mut bars, num_bars);
        
        bars
    }
    
    fn apply_dynamic_scaling(&self, raw_magnitudes: &[f32], output_bars: &mut [f32], num_bars: usize) {
        // Use percentile-based normalization for better variance
        let mut sorted_mags = raw_magnitudes.to_vec();
        sorted_mags.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        // Find percentile thresholds
        let p25_idx = (num_bars as f32 * 0.25) as usize;
        let p75_idx = (num_bars as f32 * 0.75) as usize;
        let p90_idx = (num_bars as f32 * 0.90) as usize;
        
        let p25_val = sorted_mags.get(p25_idx).unwrap_or(&0.0);
        let p75_val = sorted_mags.get(p75_idx).unwrap_or(&0.0);
        let p90_val = sorted_mags.get(p90_idx).unwrap_or(&0.0);
        let max_val = sorted_mags.last().unwrap_or(&0.0);
        
        for i in 0..num_bars {
            let mag = raw_magnitudes[i];
            
            // Map to percentile-based ranges with dramatic scaling
            let scaled = if mag <= *p25_val {
                // Bottom 25%: Map to 0-0.2 range
                (mag / p25_val.max(0.001)) * 0.2
            } else if mag <= *p75_val {
                // 25%-75%: Map to 0.2-0.6 range with power scaling
                let normalized = (mag - p25_val) / (p75_val - p25_val).max(0.001);
                0.2 + normalized.powf(1.5) * 0.4
            } else if mag <= *p90_val {
                // 75%-90%: Map to 0.6-0.85 range with strong power scaling
                let normalized = (mag - p75_val) / (p90_val - p75_val).max(0.001);
                0.6 + normalized.powf(2.0) * 0.25
            } else {
                // Top 10%: Map to 0.85-1.0 range with extreme scaling
                let normalized = (mag - p90_val) / (max_val - p90_val).max(0.001);
                0.85 + normalized.powf(3.0) * 0.15
            };
            
            output_bars[i] = scaled.min(1.0);
        }
    }
    
    fn smooth_interpolate(&mut self, target_bars: &[f32], smoothing_factor: f32) -> Vec<f32> {
        let mut smoothed = vec![0.0; self.bin_size];
        
        // Ensure previous_bars has correct size
        if self.previous_bars.len() != self.bin_size {
            self.previous_bars = vec![0.0; self.bin_size];
        }
        
        let actual_size = self.bin_size.min(target_bars.len());
        
        for i in 0..actual_size {
            let target = target_bars.get(i).unwrap_or(&0.0);
            let previous = self.previous_bars.get(i).unwrap_or(&0.0);
            
            // Linear interpolation with smoothing
            smoothed[i] = previous * (1.0 - smoothing_factor) + target * smoothing_factor;
        }
        
        // Update previous bars for next frame
        self.previous_bars = smoothed.clone();
        
        smoothed
    }
    
    fn generate_hann_window(&self, size: usize) -> Vec<f32> {
        let mut window = Vec::with_capacity(size);
        for n in 0..size {
            let value = 0.5 * (1.0 - ((2.0 * std::f32::consts::PI * n as f32) / (size - 1) as f32).cos());
            window.push(value);
        }
        window
    }
    
    fn apply_hann_window(&self, frame: &[i16], window: &[f32]) -> Vec<f32> {
        frame.iter()
            .zip(window.iter())
            .map(|(&sample, &window_val)| {
                let normalized_sample = sample as f32 / i16::MAX as f32;
                normalized_sample * window_val
            })
            .collect()
    }
}
