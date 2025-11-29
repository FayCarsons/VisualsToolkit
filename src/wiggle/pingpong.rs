#[derive(Debug, Clone)]
pub struct PingPongBuffer {
    texs: [wgpu::Texture; 2],
    idx: bool,
}

impl PingPongBuffer {
    pub fn new<'a>(device: &wgpu::Device, desc: &wgpu::TextureDescriptor<'a>) -> Self {
        let front = device.create_texture(desc);
        let back = device.create_texture(desc);

        Self {
            texs: [front, back],
            idx: false,
        }
    }

    pub fn front(&self) -> &wgpu::Texture {
        &self.texs[self.idx as usize]
    }

    pub fn back(&self) -> &wgpu::Texture {
        &self.texs[!self.idx as usize]
    }

    pub fn update(&mut self) {
        self.idx = !self.idx
    }
}

pub struct PingPongBufferView([wgpu::TextureView; 2]);
