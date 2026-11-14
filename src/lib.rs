//! CupEngine — 3D-движок на wgpu + winit + egui.

pub mod app;
pub mod bullets;
pub mod camera;
pub mod console;
pub mod input;
pub mod mesh;
pub mod renderer;
pub mod scene;
pub mod transform;

pub use egui;
pub use glam;
pub use winit;

pub mod prelude {
    pub use crate::app::{run, AppConfig, Context, Game, Time};
    pub use crate::bullets::{BulletEmitter, BulletPool};
    pub use crate::camera::{Camera, FlyCamera};
    pub use crate::input::Input;
    pub use crate::mesh::{MeshData, Vertex};
    pub use crate::renderer::{DirectionalLight, MeshId, RenderStats, Renderer};
    pub use crate::scene::{Entity, EntityId, Light, MeshKind, MeshRenderer, Scene, Spin};
    pub use crate::transform::Transform;
    pub use glam::{EulerRot, Mat4, Quat, Vec2, Vec3, Vec4};
    pub use winit::{event::MouseButton, keyboard::KeyCode};
}
