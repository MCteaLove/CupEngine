use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

/// Пул пуль (SoA): память выделяется заранее, удаление через swap_remove.
#[derive(Clone)]
pub struct BulletPool {
    pub pos: Vec<Vec3>,
    pub vel: Vec<Vec3>,
    pub life: Vec<f32>,
    /// rgb + радиус в `w`
    pub color: Vec<[f32; 4]>,
    capacity: usize,
}

impl Default for BulletPool {
    fn default() -> Self {
        Self::new(50_000)
    }
}

impl BulletPool {
    pub fn new(capacity: usize) -> Self {
        Self {
            pos: Vec::with_capacity(capacity),
            vel: Vec::with_capacity(capacity),
            life: Vec::with_capacity(capacity),
            color: Vec::with_capacity(capacity),
            capacity,
        }
    }

    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn clear(&mut self) {
        self.pos.clear();
        self.vel.clear();
        self.life.clear();
        self.color.clear();
    }

    pub fn spawn(&mut self, pos: Vec3, vel: Vec3, life: f32, color: [f32; 4]) {
        if self.pos.len() < self.capacity {
            self.pos.push(pos);
            self.vel.push(vel);
            self.life.push(life);
            self.color.push(color);
        }
    }

    pub fn update(&mut self, dt: f32, arena_radius: f32) {
        let r2 = arena_radius * arena_radius;
        let mut i = 0;
        while i < self.pos.len() {
            self.pos[i] += self.vel[i] * dt;
            self.life[i] -= dt;
            if self.life[i] <= 0.0 || self.pos[i].length_squared() > r2 {
                self.pos.swap_remove(i);
                self.vel.swap_remove(i);
                self.life.swap_remove(i);
                self.color.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }
}

/// Компонент: спиральный эмиттер пуль.
#[derive(Clone, Serialize, Deserialize)]
pub struct BulletEmitter {
    pub arms: u32,
    pub speed: f32,
    pub interval: f32,
    pub step: f32,
    pub bullet_radius: f32,
    pub color: [f32; 3],
    #[serde(skip)]
    angle: f32,
    #[serde(skip)]
    timer: f32,
}

impl Default for BulletEmitter {
    fn default() -> Self {
        Self {
            arms: 4,
            speed: 4.0,
            interval: 0.012,
            step: 0.31,
            bullet_radius: 0.18,
            color: [1.0, 0.35, 0.5],
            angle: 0.0,
            timer: 0.0,
        }
    }
}

impl BulletEmitter {
    pub fn update(&mut self, dt: f32, origin: Vec3, pool: &mut BulletPool) {
        if self.interval < 0.0005 || self.arms == 0 {
            return;
        }
        self.timer += dt;
        let mut shots = 0;
        while self.timer >= self.interval && shots < 8 {
            self.timer -= self.interval;
            self.angle += self.step;
            shots += 1;
            for k in 0..self.arms {
                let a = self.angle + k as f32 * TAU / self.arms as f32;
                let dir = Vec3::new(a.cos(), 0.0, a.sin());
                let c = [self.color[0], self.color[1], self.color[2], self.bullet_radius];
                pool.spawn(origin, dir * self.speed, 12.0, c);
            }
        }
        if self.timer > self.interval {
            self.timer = 0.0;
        }
    }
}
