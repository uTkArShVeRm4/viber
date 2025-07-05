use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use wgpu::*;
use wgpu::rwh;
use std::ptr::NonNull;

pub struct Renderer {
    device: Option<Device>,
    queue: Option<Queue>,
    surface: Option<Surface<'static>>,
    config: Option<SurfaceConfiguration>,
    render_pipeline: Option<RenderPipeline>,
    canvas: Option<HtmlCanvasElement>,
    uniform_buffer: Option<Buffer>,
    uniform_bind_group: Option<BindGroup>,
    frame_count: u32,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            device: None,
            queue: None,
            surface: None,
            config: None,
            render_pipeline: None,
            canvas: None,
            uniform_buffer: None,
            uniform_bind_group: None,
            frame_count: 0,
        }
    }

    pub async fn init(&mut self, canvas_id: &str) -> Result<(), JsValue> {
        // Get canvas element
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let canvas = document
            .get_element_by_id(canvas_id)
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();

        let width = canvas.width();
        let height = canvas.height();

        // Create WGPU instance
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::GL,
            flags: Default::default(),
            ..Default::default()
        });

        // Create surface using raw handles for canvas
        let target = SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: {
                let handle = rwh::WebDisplayHandle::new();
                rwh::RawDisplayHandle::Web(handle)
            },
            raw_window_handle: {
                let obj: NonNull<std::ffi::c_void> = NonNull::from(&canvas).cast();
                let handle = rwh::WebCanvasWindowHandle::new(obj);
                rwh::RawWindowHandle::WebCanvas(handle)
            },
        };

        let surface = unsafe { instance.create_surface_unsafe(target) }
            .map_err(|e| JsValue::from_str(&format!("Failed to create surface: {:?}", e)))?;

        // Get adapter
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        // Get device and queue
        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: None,
                    required_features: Features::empty(),
                    required_limits: Limits::downlevel_webgl2_defaults(),
                    memory_hints: Default::default(),
                    trace: Default::default(),
                },
            )
            .await
            .unwrap();

        // Configure surface
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_capabilities(&adapter).formats[0],
            width,
            height,
            present_mode: PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Create single uniform buffer (16-byte aligned)
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: (4 + 64) * 4, // (4 base floats + 64 frequency bars) * 4 bytes each = 272 bytes, aligned to 16 bytes
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout for uniforms
        let uniform_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Uniform Bind Group Layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create bind group for uniforms
        let uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Initialize uniform buffer: [time, padding, width, height]
        let uniform_data = [0.0f32, 0.0f32, width as f32, height as f32];
        queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&uniform_data));

        // Create render pipeline
        let render_pipeline = self.create_render_pipeline(&device, config.format, &uniform_bind_group_layout);

        self.device = Some(device);
        self.queue = Some(queue);
        self.surface = Some(surface);
        self.config = Some(config);
        self.render_pipeline = Some(render_pipeline);
        self.canvas = Some(canvas);
        self.uniform_buffer = Some(uniform_buffer);
        self.uniform_bind_group = Some(uniform_bind_group);

        Ok(())
    }

    fn create_render_pipeline(&self, device: &Device, format: TextureFormat, uniform_bind_group_layout: &BindGroupLayout) -> RenderPipeline {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });

        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    pub fn render(&mut self, time: f64, frequency_bars: &[f32], bin_size: usize) {
        if let (Some(device), Some(queue), Some(surface), Some(render_pipeline), Some(uniform_buffer), Some(uniform_bind_group), Some(config)) = (
            &self.device,
            &self.queue,
            &self.surface,
            &self.render_pipeline,
            &self.uniform_buffer,
            &self.uniform_bind_group,
            &self.config,
        ) {
            // Use actual elapsed time for accurate animation
            self.frame_count += 1;
            let elapsed_time = time as f32;
            
            // Create uniform data with time, bin_size, resolution, and frequency bars
            let mut uniform_data = vec![elapsed_time, bin_size as f32, config.width as f32, config.height as f32];
            
            // Add frequency bars (pad to 64 bars for shader compatibility)
            let mut bars = vec![0.0f32; 64];
            for (i, &bar) in frequency_bars.iter().take(64).enumerate() {
                bars[i] = bar;
            }
            
            // Debug logging every 120 frames (about 2 seconds)
            if self.frame_count % 120 == 0 {
                web_sys::console::log_1(&format!("frame: {}, time: {:.2}, width: {}, height: {}, bin_size: {}, bars[0]: {:.2}", self.frame_count, elapsed_time, config.width, config.height, bin_size, bars[0]).into());
            }
            
            uniform_data.extend(bars);
            
            queue.write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&uniform_data));
            let output = surface.get_current_texture().unwrap();
            let view = output
                .texture
                .create_view(&TextureViewDescriptor::default());

            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(Color {
                                r: 0.1,
                                g: 0.2,
                                b: 0.3,
                                a: 1.0,
                            }),
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(render_pipeline);
                render_pass.set_bind_group(0, uniform_bind_group, &[]);
                render_pass.draw(0..3, 0..1); // Draw a triangle
            }

            queue.submit(std::iter::once(encoder.finish()));
            output.present();
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if let (Some(surface), Some(device), Some(config)) =
            (&self.surface, &self.device, &mut self.config)
        {
            config.width = width;
            config.height = height;
            surface.configure(device, config);
        }
    }
}