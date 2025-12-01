use std::{sync::Arc, time::Instant};
use thingbuf::mpsc::blocking::Receiver;
use wgpu::include_wgsl;
use winit::{application::ApplicationHandler, dpi::PhysicalSize, event::*, window::Window};

use crate::{
    analysis::Frame,
    uniforms::{Camera, CameraUniform, DirectionKey, InputState, Time, Uniforms},
};

#[allow(private_interfaces)]
#[derive(Debug)]
pub enum App<'a> {
    Uninit(Option<Receiver<Frame>>), // Option purely so that we can `take` it
    Init(AppInner<'a>),
}

impl App<'_> {
    pub fn new(recv: Receiver<Frame>) -> Self {
        Self::Uninit(Some(recv))
    }
}

#[derive(Debug)]
struct AppInner<'a> {
    window: Arc<Window>,
    state: &'a mut State<'a>,
    last_time: Instant,
    audio_analysis_frames: Receiver<Frame>,
}

impl<'a> AppInner<'a> {
    fn new(
        state: &'a mut State<'a>,
        window: Arc<Window>,
        audio_analysis_frames: Receiver<Frame>,
    ) -> Self {
        Self {
            window,
            state,
            last_time: Instant::now(),
            audio_analysis_frames,
        }
    }
}

impl ApplicationHandler for App<'_> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Self::Uninit(recv) = self {
            let window_attributes = Window::default_attributes()
                .with_title("Volumetric Raymarch")
                .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));

            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            let state = Box::leak(Box::new(pollster::block_on(State::new(window.clone()))));

            let inner = AppInner::new(state, window, recv.take().unwrap());
            *self = Self::Init(inner);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match self {
            Self::Uninit(_) => {}
            Self::Init(inner) => {
                if inner.window.id() != window_id {
                    return;
                }

                match event {
                    WindowEvent::Resized(size) => inner.state.resize(size),
                    WindowEvent::RedrawRequested => {
                        let now = std::time::Instant::now();
                        let dt = (now - inner.last_time).as_secs_f32();

                        inner.last_time = now;

                        inner.state.update(dt);

                        if let Err(err) = inner.state.render() {
                            match err {
                                wgpu::SurfaceError::OutOfMemory => {
                                    event_loop.exit();
                                    panic!("OUT OF MEMORY");
                                }
                                wgpu::SurfaceError::Lost => {
                                    inner.state.resize(inner.state.size);
                                }
                                e => eprintln!("[ERROR]: {e}"),
                            }
                        }
                    }
                    WindowEvent::CloseRequested => event_loop.exit(),
                    #[cfg(target_os = "linux")]
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                ref logical_key,
                                ref state,
                                ..
                            },
                        ..
                    } => {
                        log::info!(
                            "{:?} -> {}",
                            logical_key,
                            if state.is_pressed() {
                                "PRESSED"
                            } else {
                                "RELEASED"
                            }
                        );

                        if let Some(dir) = DirectionKey::from_logical_key(logical_key.clone()) {
                            inner.state.input_state.keystate[dir as usize] = state.is_pressed();
                        }
                    }
                    #[cfg(target_os = "macos")]
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                ref physical_key,
                                ref state,
                                ..
                            },
                        ..
                    } => {
                        use winit::keyboard::{KeyCode, PhysicalKey};

                        log::info!(
                            "{:?} -> {}",
                            physical_key,
                            if state.is_pressed() {
                                "PRESSED"
                            } else {
                                "RELEASED"
                            }
                        );

                        if let PhysicalKey::Code(KeyCode::KeyR) = *physical_key {
                            inner.state.frame = 0;
                            inner.state.parity = false;
                        } else if let Some(dir) = DirectionKey::from_physical_key(*physical_key) {
                            inner.state.input_state.keystate[dir as usize] = state.is_pressed()
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Self::Init(inner) = self {
            inner.window.request_redraw();
        }
    }
}

#[derive(Debug)]
struct State<'window> {
    /* Config */
    surface: wgpu::Surface<'window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,

    /* Render */
    render_bind_group_front: wgpu::BindGroup,
    render_bind_group_back: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,

    /* Compute */
    compute_time_buffer: wgpu::Buffer,
    compute_pipeline: wgpu::ComputePipeline,
    compute_bind_group_front: wgpu::BindGroup,
    compute_bind_group_back: wgpu::BindGroup,

    /* Input + temporal state */
    input_state: InputState,
    camera: Camera,
    time: f32,
    frame: u32,
    parity: bool,
}

impl<'window> State<'window> {
    async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window).expect("CREATE SURFACE");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("FIND ADAPTER");

        let (device, queue) = adapter
            .request_device(&Default::default())
            .await
            .expect("CREATE DEVICE");

        // let surface_caps = surface.get_capabilities(&adapter);
        // let surface_format = surface_caps
        //     .formats
        //     .iter()
        //     .find(|f| f.is_srgb())
        //     .copied()
        //     .unwrap_or(surface_caps.formats[0]);

        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .expect("GET SURFACE CONFIG");

        surface.configure(&device, &config);

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../main.wgsl").into()),
        });

        let compute_shader = device.create_shader_module(include_wgsl!("../compute.wgsl"));

        /* Render uniforms buffers */
        let uniforms = Uniforms::new(size.width as f32, size.height as f32, 0., 0);

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        let camera = Camera::new(super::CAMERA_RADIUS, super::LOOKAT);

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        queue.write_buffer(
            &camera_buffer,
            0,
            bytemuck::cast_slice(&[camera.as_uniform()]),
        );

        /* Compute uniforms buffer */
        let compute_time_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Compute time buffer"),
            size: std::mem::size_of::<Time>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let volume_tex_a = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Volume Texture A"),
            size: wgpu::Extent3d {
                width: super::TEX_SIZE,
                height: super::TEX_SIZE,
                depth_or_array_layers: super::TEX_SIZE,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        let volume_tex_b = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Volume Texture B"),
            size: wgpu::Extent3d {
                width: super::TEX_SIZE,
                height: super::TEX_SIZE,
                depth_or_array_layers: super::TEX_SIZE,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        let volume_view_a = volume_tex_a.create_view(&wgpu::TextureViewDescriptor::default());
        let volume_view_b = volume_tex_b.create_view(&wgpu::TextureViewDescriptor::default());

        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let compute_bind_group_front = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group (Front)"),
            layout: &compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&volume_view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&volume_view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: compute_time_buffer.as_entire_binding(),
                },
            ],
        });

        let compute_bind_group_back = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group (Back)"),
            layout: &compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&volume_view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&volume_view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: compute_time_buffer.as_entire_binding(),
                },
            ],
        });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compute Pipline Layout"),
                bind_group_layouts: &[&compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("TexMain"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Volume Sampler"),
            ..Default::default()
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        let render_bind_group_front = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&volume_view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let render_bind_group_back = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&volume_view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("trivial"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("DDAMain"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[("TexSize", super::TEX_SIZE as f64)],
                    ..Default::default()
                },
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            uniform_buffer,
            camera_buffer,
            render_bind_group_front,
            render_bind_group_back,

            compute_time_buffer,
            compute_pipeline,
            compute_bind_group_front,
            compute_bind_group_back,

            /* Uniform + pipeline state */
            time: 0.0,
            camera,
            frame: 0,
            parity: false,
            input_state: Default::default(),
        }
    }

    fn resize(&mut self, size @ PhysicalSize { width, height }: winit::dpi::PhysicalSize<u32>) {
        (width > 0 && height > 0).then(|| {
            self.size = size;
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        });
    }

    fn update(&mut self, dt: f32) {
        self.input_state.move_camera();
        if self.input_state.any_movement() {
            self.camera.update(self.input_state.pos());
            self.input_state.clear();
        }

        let time = Time::new(self.time, self.frame);

        self.queue
            .write_buffer(&self.compute_time_buffer, 0, bytemuck::cast_slice(&[time]));

        let uniforms = Uniforms::new(
            self.size.width as f32,
            self.size.height as f32,
            self.time,
            self.frame,
        );

        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        let camera_uniform = self.camera.as_uniform();

        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        self.time += dt;
        self.frame += 1;
        self.parity = !self.parity;
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(
                0,
                if self.parity {
                    &self.compute_bind_group_front
                } else {
                    &self.compute_bind_group_back
                },
                &[],
            );
            let workgroups = super::TEX_SIZE.div_ceil(4);
            compute_pass.dispatch_workgroups(workgroups, workgroups, workgroups);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(
                0,
                if self.parity {
                    &self.render_bind_group_back
                } else {
                    &self.render_bind_group_front
                },
                &[],
            );
            render_pass.draw(0..3, 0..1); // Fullscreen triangle
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
