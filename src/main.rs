mod analysis;
mod app;
mod uniforms;
mod wiggle;

use crate::app::App;
use glam::Vec3;
use thingbuf::mpsc;
use winit::event_loop::{ControlFlow, EventLoop};

pub const TEX_SIZE: u32 = 32;
pub const CAMERA_RADIUS: f32 = 5.;
pub const LOOKAT: Vec3 = Vec3::splat(0.5);

fn main() {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!("[{}]: {}", record.level(), message))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()
        .unwrap();

    log::info!("Initializing ...");

    let (_sender, recv) = mpsc::blocking::channel(128);
    //let stream = analysis::make_analysis_stream(sender).unwrap();
    //stream.play().unwrap();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    log::info!("Start event loop");

    let mut app = App::new(recv);

    log::info!("Have GPU access, running app");

    event_loop.run_app(&mut app).unwrap();
}
