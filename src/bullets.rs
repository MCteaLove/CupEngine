use engine::prelude::*;
use std::f32::consts::TAU;

/// Пул пуль (SoA). Это код игры, а не движка: движок лишь рисует биллборды.
pub struct BulletPool {
    pub pos: Vec<Vec3>,
    pub vel: Vec<Vec3>,
    pub life: Vec<f32>,
    capacity: usize,
}

impl BulletPool {
    pub fn new(capacity: usize) -> Self {
        Self {
            pos: Vec::with_capacity(capacity),
            vel: Vec::with_capacity(capacity),
            life: Vec::with_capacity(capacity),
            capacity,
        }
    }

    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn spawn(&mut self, pos: Vec3, vel: Vec3, life: f32) {
        if self.pos.len() < self.capacity {
            self.pos.push(pos);
            self.vel.push(vel);
            self.life.push(life);
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
            } else {
                i += 1;
            }
        }
    }
}

/// Спиральный эмиттер.
pub struct Emitter {
    pub pos: Vec3,
    pub angle: f32,
    pub step: f32,
    pub interval: f32,
    pub arms: u32,
    pub speed: f32,
    pub timer: f32,
}

impl Emitter {
    pub fn update(&mut self, dt: f32, pool: &mut BulletPool) {
        self.timer += dt;
        while self.timer >= self.interval {
            self.timer -= self.interval;
            self.angle += self.step;
            for k in 0..self.arms {
                let a = self.angle + k as f32 * TAU / self.arms as f32;
                let dir = Vec3::new(a.cos(), 0.0, a.sin());
                pool.spawn(self.pos, dir * self.speed, 12.0);
            }
        }
    }
}
