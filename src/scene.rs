use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::bullets::{BulletEmitter, BulletPool};
use crate::transform::Transform;

pub type EntityId = u64;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum MeshKind {
    Cube,
    Sphere,
    Plane,
}

impl MeshKind {
    pub const ALL: [MeshKind; 3] = [MeshKind::Cube, MeshKind::Sphere, MeshKind::Plane];
    pub fn label(&self) -> &'static str {
        match self {
            MeshKind::Cube => "Cube",
            MeshKind::Sphere => "Sphere",
            MeshKind::Plane => "Plane",
        }
    }
}

// ---------------- компоненты ----------------

#[derive(Clone, Serialize, Deserialize)]
pub struct MeshRenderer {
    pub kind: MeshKind,
    pub color: [f32; 3],
}

impl Default for MeshRenderer {
    fn default() -> Self {
        Self { kind: MeshKind::Cube, color: [0.8, 0.8, 0.8] }
    }
}

/// Направленный свет. Направление = локальная ось -Z объекта.
#[derive(Clone, Serialize, Deserialize)]
pub struct Light {
    pub color: [f32; 3],
    pub intensity: f32,
}

impl Default for Light {
    fn default() -> Self {
        Self { color: [1.0, 0.96, 0.9], intensity: 1.0 }
    }
}

/// Вращение объекта (градусы в секунду), работает в режиме Play.
#[derive(Clone, Serialize, Deserialize)]
pub struct Spin {
    pub degrees_per_sec: Vec3,
}

impl Default for Spin {
    fn default() -> Self {
        Self { degrees_per_sec: Vec3::new(0.0, 45.0, 0.0) }
    }
}

// ---------------- объект сцены ----------------

#[derive(Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub transform: Transform,
    pub parent: Option<EntityId>,
    #[serde(default)]
    pub mesh: Option<MeshRenderer>,
    #[serde(default)]
    pub light: Option<Light>,
    #[serde(default)]
    pub spin: Option<Spin>,
    #[serde(default)]
    pub emitter: Option<BulletEmitter>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Scene {
    entities: Vec<Entity>,
    next_id: EntityId,
    /// Рантайм-состояние (пули). Не сохраняется, сбрасывается при Stop.
    #[serde(skip)]
    pub bullets: BulletPool,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    pub fn new() -> Self {
        Self { entities: Vec::new(), next_id: 1, bullets: BulletPool::new(50_000) }
    }

    pub fn spawn(&mut self, name: &str) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        self.entities.push(Entity {
            id,
            name: name.to_string(),
            transform: Transform::default(),
            parent: None,
            mesh: None,
            light: None,
            spin: None,
            emitter: None,
        });
        id
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    fn index_of(&self, id: EntityId) -> Option<usize> {
        self.entities.iter().position(|e| e.id == id)
    }

    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.entities.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.entities.iter_mut().find(|e| e.id == id)
    }

    /// Пересчитать счётчик id после загрузки из файла.
    pub fn fix_ids(&mut self) {
        self.next_id = self.entities.iter().map(|e| e.id).max().unwrap_or(0) + 1;
    }

    /// Мировая матрица родителя (или единичная, если родителя нет).
    pub fn parent_world(&self, id: EntityId) -> Mat4 {
        match self.get(id).and_then(|e| e.parent) {
            Some(p) => self.world_matrix(p),
            None => Mat4::IDENTITY,
        }
    }

    /// Выставить мировую позицию (пересчитывается в локальную).
    pub fn set_world_translation(&mut self, id: EntityId, pos: Vec3) {
        let local = self.parent_world(id).inverse().transform_point3(pos);
        if let Some(e) = self.get_mut(id) {
            e.transform.translation = local;
        }
    }

    /// Выставить мировой поворот (пересчитывается в локальный).
    pub fn set_world_rotation(&mut self, id: EntityId, rot: Quat) {
        let (_, parent_rot, _) = self.parent_world(id).to_scale_rotation_translation();
        let local = (parent_rot.inverse() * rot).normalize();
        if let Some(e) = self.get_mut(id) {
            e.transform.rotation = local;
        }
    }

    /// Мировая матрица с учётом родителей.
    pub fn world_matrix(&self, id: EntityId) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        let mut cur = Some(id);
        let mut guard = 0;
        while let Some(c) = cur {
            let Some(e) = self.get(c) else { break };
            m = e.transform.matrix() * m;
            cur = e.parent;
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        m
    }

    /// Является ли `id` потомком `ancestor`.
    pub fn is_descendant(&self, id: EntityId, ancestor: EntityId) -> bool {
        let mut cur = self.get(id).and_then(|e| e.parent);
        let mut guard = 0;
        while let Some(c) = cur {
            if c == ancestor {
                return true;
            }
            cur = self.get(c).and_then(|e| e.parent);
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        false
    }

    /// Объекты в порядке дерева: (id, глубина).
    pub fn hierarchy_order(&self) -> Vec<(EntityId, usize)> {
        let mut out = Vec::with_capacity(self.entities.len());
        for e in &self.entities {
            let is_root = match e.parent {
                None => true,
                Some(p) => self.get(p).is_none(),
            };
            if is_root {
                self.push_tree(e.id, 0, &mut out);
            }
        }
        out
    }

    fn push_tree(&self, id: EntityId, depth: usize, out: &mut Vec<(EntityId, usize)>) {
        out.push((id, depth));
        if depth > 32 {
            return;
        }
        for c in self.entities.iter().filter(|c| c.parent == Some(id)) {
            self.push_tree(c.id, depth + 1, out);
        }
    }

    /// Удаляет объект вместе со всеми потомками.
    pub fn remove(&mut self, id: EntityId) {
        let mut doomed = vec![id];
        let mut i = 0;
        while i < doomed.len() {
            let cur = doomed[i];
            for e in &self.entities {
                if e.parent == Some(cur) && !doomed.contains(&e.id) {
                    doomed.push(e.id);
                }
            }
            i += 1;
        }
        self.entities.retain(|e| !doomed.contains(&e.id));
    }

    pub fn duplicate(&mut self, id: EntityId) -> Option<EntityId> {
        let mut copy = self.get(id)?.clone();
        copy.id = self.next_id;
        self.next_id += 1;
        copy.name.push_str(" (copy)");
        let new_id = copy.id;
        self.entities.push(copy);
        Some(new_id)
    }

    /// Меняет родителя, сохраняя мировую позицию объекта.
    pub fn set_parent_keep_world(&mut self, id: EntityId, new_parent: Option<EntityId>) {
        let world = self.world_matrix(id);
        let parent_world = match new_parent {
            Some(p) => self.world_matrix(p),
            None => Mat4::IDENTITY,
        };
        let local = parent_world.inverse() * world;
        let (s, r, t) = local.to_scale_rotation_translation();
        if let Some(e) = self.get_mut(id) {
            e.parent = new_parent;
            e.transform = Transform { translation: t, rotation: r, scale: s };
        }
    }

    /// Один шаг симуляции (режим Play).
    pub fn step(&mut self, dt: f32) {
        for e in &mut self.entities {
            if let Some(s) = &e.spin {
                let delta = Quat::from_scaled_axis(s.degrees_per_sec * (std::f32::consts::PI / 180.0) * dt);
                e.transform.rotation = (e.transform.rotation * delta).normalize();
            }
        }

        let ids: Vec<EntityId> =
            self.entities.iter().filter(|e| e.emitter.is_some()).map(|e| e.id).collect();
        for id in ids {
            let pos = self.world_matrix(id).w_axis.truncate();
            let Some(idx) = self.index_of(id) else { continue };
            if let Some(mut em) = self.entities[idx].emitter.take() {
                em.update(dt, pos, &mut self.bullets);
                self.entities[idx].emitter = Some(em);
            }
        }
        self.bullets.update(dt, 60.0);
    }
}
