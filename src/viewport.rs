//! Инструменты 3D-окна редактора: камера-луч, выбор кликом, гизмо, сетка, контуры и иконки.

use std::f32::consts::TAU;

use engine::egui::{self, Color32, Pos2, Rect, Stroke};
use engine::prelude::*;

// ======================================================================
// Камера окна просмотра
// ======================================================================

pub struct ViewCam {
    eye: Vec3,
    view_proj: Mat4,
    inv: Mat4,
    rect: Rect,
    fov_y: f32,
}

impl ViewCam {
    pub fn new(cam: &Camera, rect: Rect) -> Self {
        let aspect = rect.width() / rect.height().max(1.0);
        let vp = cam.proj(aspect) * cam.view();
        Self { eye: cam.eye, view_proj: vp, inv: vp.inverse(), rect, fov_y: cam.fov_y }
    }

    /// Мировая точка -> позиция на экране (в точках egui).
    pub fn project(&self, p: Vec3) -> Option<Pos2> {
        let c = self.view_proj * p.extend(1.0);
        if c.w <= 1e-4 {
            return None;
        }
        let ndc = c.truncate() / c.w;
        Some(Pos2::new(
            self.rect.min.x + (ndc.x * 0.5 + 0.5) * self.rect.width(),
            self.rect.min.y + (1.0 - (ndc.y * 0.5 + 0.5)) * self.rect.height(),
        ))
    }

    /// Луч из камеры через точку экрана: (начало, направление).
    pub fn ray(&self, pos: Pos2) -> (Vec3, Vec3) {
        let x = (pos.x - self.rect.min.x) / self.rect.width() * 2.0 - 1.0;
        let y = 1.0 - (pos.y - self.rect.min.y) / self.rect.height() * 2.0;
        let n = self.inv * Vec4::new(x, y, 0.0, 1.0);
        let f = self.inv * Vec4::new(x, y, 1.0, 1.0);
        let n = n.truncate() / n.w;
        let f = f.truncate() / f.w;
        (n, (f - n).normalize())
    }

    /// Какой длине в мире на расстоянии точки `p` соответствуют `px` пикселей.
    pub fn world_len(&self, p: Vec3, px: f32) -> f32 {
        let d = (p - self.eye).length();
        px * 2.0 * (self.fov_y * 0.5).tan() * d / self.rect.height().max(1.0)
    }
}

// ======================================================================
// Математика лучей
// ======================================================================

fn ray_sphere(o: Vec3, d: Vec3, c: Vec3, r: f32) -> Option<f32> {
    let oc = o - c;
    let a = d.dot(d);
    let b = 2.0 * d.dot(oc);
    let cc = oc.dot(oc) - r * r;
    let disc = b * b - 4.0 * a * cc;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let mut t = (-b - sq) / (2.0 * a);
    if t < 0.0 {
        t = (-b + sq) / (2.0 * a);
    }
    if t < 0.0 {
        None
    } else {
        Some(t)
    }
}

fn ray_aabb(o: Vec3, d: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let mut tmin = 0.0f32;
    let mut tmax = f32::INFINITY;
    for i in 0..3 {
        if d[i].abs() < 1e-8 {
            if o[i] < min[i] || o[i] > max[i] {
                return None;
            }
        } else {
            let mut t1 = (min[i] - o[i]) / d[i];
            let mut t2 = (max[i] - o[i]) / d[i];
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
    }
    Some(tmin)
}

fn ray_plane(o: Vec3, d: Vec3, p: Vec3, n: Vec3) -> Option<Vec3> {
    let denom = n.dot(d);
    if denom.abs() < 1e-5 {
        return None;
    }
    let t = (p - o).dot(n) / denom;
    if t < 0.0 {
        None
    } else {
        Some(o + d * t)
    }
}

/// Параметр точки на прямой (p0 + t*a), ближайшей к лучу.
fn axis_param(ray_o: Vec3, ray_d: Vec3, p0: Vec3, a: Vec3) -> Option<f32> {
    let w0 = p0 - ray_o;
    let b = a.dot(ray_d);
    let denom = 1.0 - b * b;
    if denom.abs() < 1e-4 {
        return None;
    }
    let d = a.dot(w0);
    let e = ray_d.dot(w0);
    Some((b * e - d) / denom)
}

fn ray_vs_mesh(m: Mat4, kind: MeshKind, ray_o: Vec3, ray_d: Vec3) -> Option<f32> {
    if m.determinant().abs() < 1e-9 {
        return None;
    }
    let inv = m.inverse();
    let o = inv.transform_point3(ray_o);
    let d = inv.transform_vector3(ray_d);
    match kind {
        MeshKind::Sphere => ray_sphere(o, d, Vec3::ZERO, 0.5),
        MeshKind::Cube => ray_aabb(o, d, Vec3::splat(-0.5), Vec3::splat(0.5)),
        MeshKind::Plane => {
            if d.y.abs() < 1e-6 {
                return None;
            }
            let t = -o.y / d.y;
            if t < 0.0 {
                return None;
            }
            let p = o + d * t;
            if p.x.abs() <= 1.0 && p.z.abs() <= 1.0 {
                Some(t)
            } else {
                None
            }
        }
    }
}

/// Выбор объекта лучом: ближайший.
pub fn pick(scene: &Scene, ray_o: Vec3, ray_d: Vec3) -> Option<EntityId> {
    let mut best: Option<(f32, EntityId)> = None;
    for e in scene.entities() {
        let m = scene.world_matrix(e.id);
        let t = match &e.mesh {
            Some(mr) => ray_vs_mesh(m, mr.kind, ray_o, ray_d),
            None => ray_sphere(ray_o, ray_d, m.w_axis.truncate(), 0.4),
        };
        if let Some(t) = t {
            if best.map_or(true, |b| t < b.0) {
                best = Some((t, e.id));
            }
        }
    }
    best.map(|b| b.1)
}

// ======================================================================
// Гизмо
// ======================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

pub struct Ptr {
    pub pos: Option<Pos2>,
    pub pressed: bool,
    pub down: bool,
}

#[derive(Clone, Copy)]
struct Drag {
    axis: usize,
    start_pos: Vec3,
    start_rot: Quat,
    start_scale: Vec3,
    t0: f32,
    hit0: Vec3,
    len: f32,
    dir: Vec3,
}

pub struct Gizmo {
    pub mode: GizmoMode,
    drag: Option<Drag>,
    hover: Option<usize>,
}

pub fn perp(axis: Vec3) -> (Vec3, Vec3) {
    let helper = if axis.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let u = axis.cross(helper).normalize();
    let v = axis.cross(u);
    (u, v)
}

fn ring(center: Vec3, axis: Vec3, radius: f32, segs: usize) -> Vec<Vec3> {
    let (u, v) = perp(axis);
    (0..=segs)
        .map(|i| {
            let a = i as f32 / segs as f32 * TAU;
            center + (u * a.cos() + v * a.sin()) * radius
        })
        .collect()
}

fn dist_seg(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let l2 = ab.length_sq().max(1e-6);
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    (a + ab * t - p).length()
}

fn axis_color(i: usize, active: bool) -> Color32 {
    if active {
        return Color32::from_rgb(255, 220, 60);
    }
    match i {
        0 => Color32::from_rgb(230, 60, 60),
        1 => Color32::from_rgb(80, 200, 80),
        _ => Color32::from_rgb(70, 110, 240),
    }
}

impl Gizmo {
    pub fn new() -> Self {
        Self { mode: GizmoMode::Translate, drag: None, hover: None }
    }

    fn axes(&self, wrot: Quat) -> [Vec3; 3] {
        match self.mode {
            // масштаб идёт по локальным осям объекта
            GizmoMode::Scale => [wrot * Vec3::X, wrot * Vec3::Y, wrot * Vec3::Z],
            _ => [Vec3::X, Vec3::Y, Vec3::Z],
        }
    }

    fn hit_test(&self, vc: &ViewCam, wpos: Vec3, len: f32, dirs: &[Vec3; 3], p: Pos2) -> Option<usize> {
        let mut best: Option<(f32, usize)> = None;
        for i in 0..3 {
            let d = match self.mode {
                GizmoMode::Rotate => {
                    let pts: Vec<Option<Pos2>> =
                        ring(wpos, dirs[i], len * 0.9, 48).into_iter().map(|q| vc.project(q)).collect();
                    let mut m = f32::MAX;
                    for w in pts.windows(2) {
                        if let (Some(a), Some(b)) = (w[0], w[1]) {
                            m = m.min(dist_seg(p, a, b));
                        }
                    }
                    m
                }
                _ => match (vc.project(wpos), vc.project(wpos + dirs[i] * len)) {
                    (Some(a), Some(b)) => dist_seg(p, a, b),
                    _ => f32::MAX,
                },
            };
            if d < 9.0 && best.map_or(true, |b| d < b.0) {
                best = Some((d, i));
            }
        }
        best.map(|b| b.1)
    }

    /// Обновляет взаимодействие. Возвращает true, если клик/перетаскивание "съело" гизмо.
    pub fn update(&mut self, vc: &ViewCam, scene: &mut Scene, id: EntityId, ptr: &Ptr) -> bool {
        let world = scene.world_matrix(id);
        let (_, wrot, wpos) = world.to_scale_rotation_translation();
        let len = vc.world_len(wpos, 100.0);
        let dirs = self.axes(wrot);
        let mut consumed = false;

        if self.drag.is_none() {
            self.hover = None;
            if let Some(p) = ptr.pos {
                self.hover = self.hit_test(vc, wpos, len, &dirs, p);
            }
            if ptr.pressed {
                if let (Some(axis), Some(p)) = (self.hover, ptr.pos) {
                    let (o, d) = vc.ray(p);
                    let dir = dirs[axis];
                    let start_scale = scene.get(id).map(|e| e.transform.scale).unwrap_or(Vec3::ONE);
                    let start = match self.mode {
                        GizmoMode::Rotate => ray_plane(o, d, wpos, dir).map(|h| (0.0, h)),
                        _ => axis_param(o, d, wpos, dir).map(|t| (t, Vec3::ZERO)),
                    };
                    if let Some((t0, hit0)) = start {
                        self.drag = Some(Drag {
                            axis,
                            start_pos: wpos,
                            start_rot: wrot,
                            start_scale,
                            t0,
                            hit0,
                            len,
                            dir,
                        });
                        consumed = true;
                    }
                }
            }
        } else if let Some(d) = self.drag {
            consumed = true;
            if ptr.down {
                if let Some(p) = ptr.pos {
                    self.apply(vc, scene, id, d, p);
                }
            } else {
                self.drag = None;
            }
        }
        consumed
    }

    fn apply(&self, vc: &ViewCam, scene: &mut Scene, id: EntityId, d: Drag, p: Pos2) {
        let (o, r) = vc.ray(p);
        match self.mode {
            GizmoMode::Translate => {
                if let Some(t) = axis_param(o, r, d.start_pos, d.dir) {
                    scene.set_world_translation(id, d.start_pos + d.dir * (t - d.t0));
                }
            }
            GizmoMode::Scale => {
                if let Some(t) = axis_param(o, r, d.start_pos, d.dir) {
                    let f = (1.0 + (t - d.t0) / d.len).max(0.01);
                    if let Some(e) = scene.get_mut(id) {
                        let mut s = d.start_scale;
                        s[d.axis] = d.start_scale[d.axis] * f;
                        e.transform.scale = s;
                    }
                }
            }
            GizmoMode::Rotate => {
                if let Some(h) = ray_plane(o, r, d.start_pos, d.dir) {
                    let v0 = d.hit0 - d.start_pos;
                    let v1 = h - d.start_pos;
                    if v0.length() > 1e-4 && v1.length() > 1e-4 {
                        let ang = d.dir.dot(v0.cross(v1)).atan2(v0.dot(v1));
                        let q = (Quat::from_axis_angle(d.dir, ang) * d.start_rot).normalize();
                        scene.set_world_rotation(id, q);
                    }
                }
            }
        }
    }

    pub fn draw(&self, painter: &egui::Painter, vc: &ViewCam, scene: &Scene, id: EntityId) {
        let world = scene.world_matrix(id);
        let (_, wrot, wpos) = world.to_scale_rotation_translation();
        let len = vc.world_len(wpos, 100.0);
        let dirs = self.axes(wrot);
        let active = self.drag.map(|d| d.axis).or(self.hover);
        let Some(o2) = vc.project(wpos) else { return };

        for i in 0..3 {
            let col = axis_color(i, active == Some(i));
            match self.mode {
                GizmoMode::Rotate => {
                    let pts: Vec<Pos2> =
                        ring(wpos, dirs[i], len * 0.9, 48).into_iter().filter_map(|q| vc.project(q)).collect();
                    painter.add(egui::Shape::line(pts, Stroke::new(2.5, col)));
                }
                _ => {
                    let Some(e2) = vc.project(wpos + dirs[i] * len) else { continue };
                    painter.line_segment([o2, e2], Stroke::new(3.0, col));
                    if self.mode == GizmoMode::Translate {
                        let dir = (e2 - o2).normalized();
                        if dir.is_finite() {
                            let n = egui::vec2(-dir.y, dir.x);
                            let tip = e2 + dir * 14.0;
                            painter.add(egui::Shape::convex_polygon(
                                vec![tip, e2 + n * 6.0, e2 - n * 6.0],
                                col,
                                Stroke::NONE,
                            ));
                        }
                    } else {
                        painter.rect_filled(Rect::from_center_size(e2, egui::vec2(10.0, 10.0)), 1.0, col);
                    }
                }
            }
        }
        painter.circle_filled(o2, 4.0, Color32::WHITE);
    }
}

// ======================================================================
// 3D-линии: сетка, контур выбранного, иконки невидимых объектов
// ======================================================================

pub fn draw_grid(r: &mut Renderer, half: i32) {
    let y = 0.01;
    let h = half as f32;
    for i in -half..=half {
        let f = i as f32;
        let major = i % 5 == 0;
        let base = if major { Vec4::new(0.45, 0.47, 0.55, 0.85) } else { Vec4::new(0.3, 0.32, 0.4, 0.45) };
        let cx = if i == 0 { Vec4::new(0.85, 0.25, 0.25, 0.95) } else { base }; // ось X
        let cz = if i == 0 { Vec4::new(0.25, 0.4, 0.95, 0.95) } else { base }; // ось Z
        r.draw_line(Vec3::new(-h, y, f), Vec3::new(h, y, f), cx);
        r.draw_line(Vec3::new(f, y, -h), Vec3::new(f, y, h), cz);
    }
}

fn draw_wire_box(r: &mut Renderer, m: Mat4, min: Vec3, max: Vec3, color: Vec4) {
    let c = |x: f32, y: f32, z: f32| m.transform_point3(Vec3::new(x, y, z));
    let p = [
        c(min.x, min.y, min.z),
        c(max.x, min.y, min.z),
        c(max.x, max.y, min.z),
        c(min.x, max.y, min.z),
        c(min.x, min.y, max.z),
        c(max.x, min.y, max.z),
        c(max.x, max.y, max.z),
        c(min.x, max.y, max.z),
    ];
    const EDGES: [(usize, usize); 12] =
        [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    for (a, b) in EDGES {
        r.draw_line(p[a], p[b], color);
    }
}

fn draw_circle(r: &mut Renderer, center: Vec3, u: Vec3, v: Vec3, radius: f32, color: Vec4, segs: usize) {
    let mut prev = center + u * radius;
    for i in 1..=segs {
        let a = i as f32 / segs as f32 * TAU;
        let p = center + (u * a.cos() + v * a.sin()) * radius;
        r.draw_line(prev, p, color);
        prev = p;
    }
}

fn bounds(kind: MeshKind) -> (Vec3, Vec3) {
    match kind {
        MeshKind::Cube | MeshKind::Sphere => (Vec3::splat(-0.52), Vec3::splat(0.52)),
        MeshKind::Plane => (Vec3::new(-1.0, -0.03, -1.0), Vec3::new(1.0, 0.03, 1.0)),
    }
}

pub fn draw_scene_overlays(r: &mut Renderer, scene: &Scene, selected: Option<EntityId>) {
    let orange = Vec4::new(1.0, 0.62, 0.12, 1.0);
    let yellow = Vec4::new(1.0, 0.9, 0.3, 1.0);
    let pink = Vec4::new(1.0, 0.4, 0.6, 1.0);

    for e in scene.entities() {
        let m = scene.world_matrix(e.id);
        let pos = m.w_axis.truncate();

        // источник света: "солнце" + стрелка направления
        if e.light.is_some() {
            draw_circle(r, pos, Vec3::X, Vec3::Y, 0.3, yellow, 16);
            draw_circle(r, pos, Vec3::X, Vec3::Z, 0.3, yellow, 16);
            draw_circle(r, pos, Vec3::Y, Vec3::Z, 0.3, yellow, 16);
            let dir = m.transform_vector3(Vec3::NEG_Z).normalize_or_zero();
            if dir != Vec3::ZERO {
                let tip = pos + dir * 2.5;
                r.draw_line(pos + dir * 0.3, tip, yellow);
                let (u, v) = perp(dir);
                for w in [u, -u, v, -v] {
                    r.draw_line(tip, tip - dir * 0.35 + w * 0.15, yellow);
                }
            }
        }

        // эмиттер пуль: ромб
        if e.emitter.is_some() {
            let pts = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z]
                .map(|d| pos + m.transform_vector3(d).normalize_or_zero() * 0.3);
            for i in 0..6 {
                for j in (i + 1)..6 {
                    if i / 2 != j / 2 {
                        r.draw_line(pts[i], pts[j], pink);
                    }
                }
            }
        }

        // пустой объект: маленькие оси
        if e.mesh.is_none() && e.light.is_none() && e.emitter.is_none() {
            let axes = [
                (Vec3::X, Vec4::new(0.9, 0.3, 0.3, 1.0)),
                (Vec3::Y, Vec4::new(0.3, 0.85, 0.3, 1.0)),
                (Vec3::Z, Vec4::new(0.3, 0.45, 0.95, 1.0)),
            ];
            for (d, col) in axes {
                let w = m.transform_vector3(d).normalize_or_zero() * 0.4;
                r.draw_line(pos - w, pos + w, col);
            }
        }

        // контур выбранного объекта
        if selected == Some(e.id) {
            match &e.mesh {
                Some(mr) => {
                    let (mn, mx) = bounds(mr.kind);
                    draw_wire_box(r, m, mn, mx, orange);
                }
                None => {
                    draw_circle(r, pos, Vec3::X, Vec3::Y, 0.55, orange, 24);
                    draw_circle(r, pos, Vec3::X, Vec3::Z, 0.55, orange, 24);
                    draw_circle(r, pos, Vec3::Y, Vec3::Z, 0.55, orange, 24);
                }
            }
        }
    }
}
