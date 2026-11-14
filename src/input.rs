use std::collections::HashSet;

use glam::Vec2;
use winit::{event::MouseButton, keyboard::KeyCode};

#[derive(Default)]
pub struct Input {
    down: HashSet<KeyCode>,
    pressed: HashSet<KeyCode>,
    mouse_down: HashSet<MouseButton>,
    mouse_delta: Vec2,
}

impl Input {
    /// Клавиша удерживается.
    pub fn key_down(&self, k: KeyCode) -> bool {
        self.down.contains(&k)
    }
    /// Клавишу нажали в этом кадре.
    pub fn key_pressed(&self, k: KeyCode) -> bool {
        self.pressed.contains(&k)
    }
    pub fn mouse_down(&self, b: MouseButton) -> bool {
        self.mouse_down.contains(&b)
    }
    /// Смещение мыши за этот кадр.
    pub fn mouse_delta(&self) -> Vec2 {
        self.mouse_delta
    }

    pub(crate) fn set_key(&mut self, k: KeyCode, pressed: bool) {
        if pressed {
            if self.down.insert(k) {
                self.pressed.insert(k);
            }
        } else {
            self.down.remove(&k);
        }
    }
    pub(crate) fn set_mouse(&mut self, b: MouseButton, pressed: bool) {
        if pressed {
            self.mouse_down.insert(b);
        } else {
            self.mouse_down.remove(&b);
        }
    }
    pub(crate) fn add_mouse_delta(&mut self, d: Vec2) {
        self.mouse_delta += d;
    }
    pub(crate) fn end_frame(&mut self) {
        self.pressed.clear();
        self.mouse_delta = Vec2::ZERO;
    }
}
