mod analysis;
mod app;
mod uniforms;

use crate::app::App;
use glam::Vec3;
use thingbuf::mpsc;
use winit::event_loop::{ControlFlow, EventLoop};

pub const TEX_SIZE: u32 = 64;
pub const CAMERA_RADIUS: f32 = 6.;
pub const LOOKAT: Vec3 = Vec3::splat(0.5);

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
