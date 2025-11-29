use glam::Vec2;

pub struct KeyPos<const MODE: u8>(Vec2);

// So we can do `key_layout: Wasd | Hjkl | Arrows`
#[repr(u8)]
pub enum KeyLayout {
    Wasd = 0u8,
    Hjkl = 1u8 << 1u8,
    Arrows = 1u8 << 2u8,
}

impl<const Flag: u8> KeyPos<Flag> {
    pub fn new<const N: u8>(flag: KeyLayout) -> KeyPos<N> {
        KeyPos(Vec2::ZERO)
    }
}

pub struct Input {
    mouse_pos: Vec2,
    key_pos: Vec2,
}
