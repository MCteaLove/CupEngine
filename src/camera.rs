use glam::{Mat4, Vec3};
use winit::{event::MouseButton, keyboard::KeyCode};

use crate::input::Input;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov_y: f32, // радианы
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, 5.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov_y: 60f32.to_radians(),
            near: 0.1,
            far: 500.0,
        }
    }
}

impl Camera {
    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye, self.target, self.up)
    }
    pub fn proj(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, aspect, self.near, self.far)
    }
}

/// Свободная камера: ПКМ + мышь — поворот, WASD — движение, Q/E — вниз/вверх, Shift — ускорение.
pub struct FlyCamera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
    pub sensitivity: f32,
}

impl FlyCamera {
    pub fn new(position: Vec3) -> Self {
        Self { position, yaw: 0.0, pitch: 0.0, speed: 8.0, sensitivity: 0.003 }
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }

    pub fn update(&mut self, input: &Input, dt: f32) {
        if input.mouse_down(MouseButton::Right) {
            let d = input.mouse_delta();
            self.yaw += d.x * self.sensitivity;
            self.pitch = (self.pitch - d.y * self.sensitivity).clamp(-1.55, 1.55);
        }
        let fwd = self.forward();
        let right = fwd.cross(Vec3::Y).normalize_or_zero();
        let mut dir = Vec3::ZERO;
        if input.key_down(KeyCode::KeyW) { dir += fwd; }
        if input.key_down(KeyCode::KeyS) { dir -= fwd; }
        if input.key_down(KeyCode::KeyD) { dir += right; }
        if input.key_down(KeyCode::KeyA) { dir -= right; }
        if input.key_down(KeyCode::KeyE) { dir += Vec3::Y; }
        if input.key_down(KeyCode::KeyQ) { dir -= Vec3::Y; }
        let boost = if input.key_down(KeyCode::ShiftLeft) { 3.0 } else { 1.0 };
        self.position += dir.normalize_or_zero() * self.speed * boost * dt;
    }

    pub fn camera(&self) -> Camera {
        Camera {
            eye: self.position,
            target: self.position + self.forward(),
            ..Default::default()
        }
    }
}
