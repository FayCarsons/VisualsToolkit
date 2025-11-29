use std::{
    convert::identity,
    f32::consts::{PI, TAU},
};

use glam::{Vec2, Vec3};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DirectionKey {
    Up = 0,
    Left = 1,
    Down = 2,
    Right = 3,
}

impl DirectionKey {
    pub fn from_logical_key(key: winit::keyboard::Key) -> Option<Self> {
        use winit::keyboard::Key::Character;

        match key {
            Character(c) if c == "w" => Some(Self::Up),
            Character(c) if c == "a" => Some(Self::Left),
            Character(c) if c == "s" => Some(Self::Down),
            Character(c) if c == "d" => Some(Self::Right),
            _ => None,
        }
    }

    pub fn from_physical_key(key: winit::keyboard::PhysicalKey) -> Option<Self> {
        use winit::keyboard::{KeyCode::*, PhysicalKey::Code};

        if let Code(k) = key {
            match k {
                KeyW | KeyK => Some(Self::Up),
                KeyS | KeyJ => Some(Self::Down),
                KeyA | KeyH => Some(Self::Left),
                KeyD | KeyL => Some(Self::Right),
                _ => None,
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Default)]
pub struct InputState {
    pub keystate: [bool; 4],
    movement_delta: Vec2,
}

impl InputState {
    pub fn move_camera(&mut self) {
        use DirectionKey::*;

        const DELTA_KEY: f32 = 0.05;

        if self.keystate[Up as usize] {
            self.movement_delta.y += DELTA_KEY;
        }

        if self.keystate[Down as usize] {
            self.movement_delta.y -= DELTA_KEY;
        }

        if self.keystate[Left as usize] {
            self.movement_delta.x -= DELTA_KEY;
        }

        if self.keystate[Right as usize] {
            self.movement_delta.x += DELTA_KEY;
        }
    }

    pub fn any_movement(&self) -> bool {
        self.keystate.iter().copied().any(identity)
    }

    pub fn pos(&self) -> &Vec2 {
        &self.movement_delta
    }

    pub fn clear(&mut self) {
        self.keystate = [false; 4];
        self.movement_delta = Vec2::ZERO;
    }
}

#[derive(Debug)]
pub struct Camera {
    theta: f32,
    phi: f32,
    radius: f32,
    lookat: Vec3,
}

// This padding is awful, can't we get rid of this?
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, bytemuck::Zeroable, bytemuck::NoUninit)]
pub struct CameraUniform {
    pos: Vec3,
    __p1: f32,
    forward: Vec3,
    __p2: f32,
    right: Vec3,
    __p3: f32,
    up: Vec3,
    __p4: f32,
}

impl Camera {
    pub fn new(radius: f32, lookat: Vec3) -> Self {
        Self {
            theta: 0.,
            phi: PI / 4.,
            radius,
            lookat,
        }
    }

    pub fn update(&mut self, Vec2 { x, y }: &Vec2) {
        self.theta += x;
        self.theta %= TAU; // wrap to prevent fp errors

        fn wrap(n: f32, min: f32, max: f32) -> f32 {
            let range = max - min;
            let normalized = (n - min) % range;
            min + if normalized < 0. {
                normalized + range
            } else {
                normalized
            }
        }

        self.phi += y;
        self.phi = wrap(self.phi, 0.1, PI - 0.1)
    }

    fn position(&self) -> Vec3 {
        Vec3::new(
            self.lookat.x + self.radius * self.phi.sin() * self.theta.cos(),
            self.lookat.y + self.radius * self.phi.cos(),
            self.lookat.z + self.radius * self.phi.sin() * self.theta.sin(),
        )
    }

    pub fn as_uniform(&self) -> CameraUniform {
        let pos = self.position();
        let forward = (self.lookat - pos).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();

        CameraUniform {
            pos,
            forward,
            right,
            up,

            ..Default::default()
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, bytemuck::Zeroable, bytemuck::NoUninit)]
pub struct Uniforms {
    resolution: [f32; 2],
    time: f32,
    __pad: f32,
}

impl Uniforms {
    pub fn new(width: f32, height: f32, time: f32) -> Self {
        Self {
            resolution: [width, height],
            time,
            ..Default::default()
        }
    }
}
