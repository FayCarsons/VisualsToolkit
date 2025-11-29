pub struct Window<'a> {
    surface: wgpu::Surface<'a>,
    surface_config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
}
