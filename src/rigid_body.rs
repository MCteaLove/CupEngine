
use glam::{Quat, Vec3};

use crate::transform::Transform;

#[derive(Debug, Clone, Copy)]
pub struct RigidBody {
    // Скорость перемещения в мировых координатах.
    pub linear_velocity: Vec3,

    // Угловая скорость относительно локальных осей.
    pub angular_velocity: Vec3,

    // Сила, приложенная к объекту.
    pub force: Vec3,

    // Крутящий момент относительно локальных осей.
    pub torque: Vec3,

    // Масса объекта.
    pub mass: f32,

    // Момент инерции вокруг локальных осей X, Y, Z.
    pub angular_inertia: Vec3,
}

impl RigidBody {
    pub fn new(mass: f32) -> Self {
        Self {
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            force: Vec3::ZERO,
            torque: Vec3::ZERO,
            mass: mass.max(0.0001),
            angular_inertia: Vec3::ONE,
        }
    }

    pub fn apply_force(&mut self, force: Vec3) {
        self.force += force;
    }

    pub fn apply_torque(&mut self, torque: Vec3) {
        self.torque += torque;
    }

    pub fn update(&mut self, transform: &mut Transform, delta_time: f32) {
        if delta_time <= 0.0 {
            return;
        }

        // Линейное движение.
        let acceleration = self.force / self.mass;

        self.linear_velocity += acceleration * delta_time;
        transform.position += self.linear_velocity * delta_time;

        // Угловое движение.
        let inertia = self.angular_inertia.max(Vec3::splat(0.0001));

        let angular_acceleration = self.torque / inertia;

        self.angular_velocity += angular_acceleration * delta_time;

        // Применяем вращение через кватернион.
        let angular_speed = self.angular_velocity.length();

        if angular_speed > 0.000001 {
            let axis = self.angular_velocity / angular_speed;
            let angle = angular_speed * delta_time;

            let rotation = Quat::from_axis_angle(axis, angle);

            transform.rotate_local(rotation);
        }

        // Сбрасываем силы и моменты после шага симуляции.
        self.force = Vec3::ZERO;
        self.torque = Vec3::ZERO;
    }
}

impl Default for RigidBody {
    fn default() -> Self {
        Self::new(1.0)
    }
}
