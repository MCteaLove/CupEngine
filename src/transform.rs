use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self { translation: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::ONE }
    }
}

impl Transform {
    pub fn from_translation(t: Vec3) -> Self {
        Self { translation: t, ..Default::default() }
    }
    pub fn with_rotation(mut self, r: Quat) -> Self {
        self.rotation = r;
        self
    }
    pub fn with_scale(mut self, s: Vec3) -> Self {
        self.scale = s;
        self
    }
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}
