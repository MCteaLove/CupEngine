
use glam::{Quat, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Transform {
    /// Положение объекта в мировом пространстве.
    pub position: Vec3,

    /// Ориентация объекта в пространстве.
    pub rotation: Quat,

    /// Масштаб объекта.
    pub scale: Vec3,
}

impl Transform {
    /// Создаёт трансформацию без смещения, поворота и изменения масштаба.
    pub fn new() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    /// Направление, в котором объект смотрит.
    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::NEG_Z
    }

    /// Направление вправо относительно объекта.
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    /// Направление вверх относительно объекта.
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    /// Перемещение относительно локальных осей объекта.
    pub fn translate_local(&mut self, movement: Vec3) {
        self.position += self.rotation * movement;
    }

    /// Поворот относительно локальных осей объекта.
    pub fn rotate_local(&mut self, rotation: Quat) {
        self.rotation = (self.rotation * rotation).normalize();
    }

    /// Поворот относительно мировых осей.
    pub fn rotate_world(&mut self, rotation: Quat) {
        self.rotation = (rotation * self.rotation).normalize();
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}
