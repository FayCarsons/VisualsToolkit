pub mod input;
pub mod pingpong;
pub mod window;

pub struct Wiggle<'a> {
    surface: wgpu::Surface<'a>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
}
