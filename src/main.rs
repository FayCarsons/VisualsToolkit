mod analysis;

use cpal::traits::StreamTrait;
use std::{sync::Arc, time::Instant};
use thingbuf::mpsc::{self, blocking::Receiver};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

use crate::analysis::Frame;

#[derive(Debug)]
enum App<'a> {
    Uninit(Option<Receiver<Frame>>), // Sucks, always `Some` but we need to be able to take it
    Init(AppInner<'a>),
}

impl App<'_> {
    fn new(recv: Receiver<Frame>) -> Self {
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
                    _ => {}
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Self::Init(inner) = self {
            inner.window.request_redraw();
        }
    }
}

#[derive(Debug)]
struct State<'window> {
    surface: wgpu::Surface<'window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    volume_tex: wgpu::Texture,
    volume_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    time: f32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, bytemuck::Zeroable, bytemuck::NoUninit)]
struct Uniforms {
    resolution: [f32; 2],
    time: f32,
    __pad: f32,
}

impl Uniforms {
    fn new(width: f32, height: f32, time: f32) -> Self {
        Self {
            resolution: [width, height],
            time,
            ..Default::default()
        }
    }
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

        const TEX_SIZE: u32 = 64;
        let volume_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Volume Texture"),
            size: wgpu::Extent3d {
                width: TEX_SIZE,
                height: TEX_SIZE,
                depth_or_array_layers: TEX_SIZE,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let volume_view = volume_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Volume Sampler"),
            ..Default::default()
        });

        let uniforms = Uniforms::new(size.width as f32, size.height as f32, 0.);

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&volume_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../main.wgsl").into()),
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
                compilation_options: Default::default(),
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
            bind_group,
            volume_tex,
            volume_view,
            sampler,
            time: 0.0,
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
        self.time += dt;

        let uniforms = Uniforms::new(self.size.width as f32, self.size.height as f32, self.time);

        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
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
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Fullscreen triangle
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

fn main() {
    env_logger::init();

    let (_sender, recv) = mpsc::blocking::channel(128);
    //let stream = analysis::make_analysis_stream(sender).unwrap();
    //stream.play().unwrap();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(recv);

    event_loop.run_app(&mut app).unwrap();
}
