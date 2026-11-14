//! Панель Inspector: редактирование выбранного объекта.

use engine::egui;
use engine::prelude::*;

pub enum Action {
    None,
    Delete,
    Duplicate,
}

fn vec3_edit(ui: &mut egui::Ui, label: &str, v: &mut Vec3, speed: f64) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        changed |= ui.add(egui::DragValue::new(&mut v.x).speed(speed).prefix("x ")).changed();
        changed |= ui.add(egui::DragValue::new(&mut v.y).speed(speed).prefix("y ")).changed();
        changed |= ui.add(egui::DragValue::new(&mut v.z).speed(speed).prefix("z ")).changed();
    });
    changed
}

fn drag_f32(ui: &mut egui::Ui, label: &str, v: &mut f32, speed: f64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(v).speed(speed));
    });
}

pub fn inspector(ui: &mut egui::Ui, scene: &mut Scene, id: EntityId) -> Action {
    let mut action = Action::None;

    // кандидаты в родители: все, кроме самого объекта и его потомков
    let candidates: Vec<(EntityId, String)> = scene
        .entities()
        .iter()
        .filter(|c| c.id != id && !scene.is_descendant(c.id, id))
        .map(|c| (c.id, c.name.clone()))
        .collect();
    let mut new_parent: Option<Option<EntityId>> = None;

    {
        let Some(e) = scene.get_mut(id) else { return action };

        ui.horizontal(|ui| {
            if ui.button("Duplicate").clicked() {
                action = Action::Duplicate;
            }
            if ui.button("Delete").clicked() {
                action = Action::Delete;
            }
        });
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut e.name);
        });

        let mut parent = e.parent;
        let parent_name = parent
            .and_then(|p| candidates.iter().find(|c| c.0 == p).map(|c| c.1.clone()))
            .unwrap_or_else(|| "None".to_string());
        egui::ComboBox::from_label("Parent").selected_text(parent_name).show_ui(ui, |ui| {
            ui.selectable_value(&mut parent, None, "None");
            for (cid, cname) in &candidates {
                ui.selectable_value(&mut parent, Some(*cid), cname.as_str());
            }
        });
        if parent != e.parent {
            new_parent = Some(parent);
        }

        // ---- Transform ----
        ui.separator();
        ui.strong("Transform");
        vec3_edit(ui, "Position", &mut e.transform.translation, 0.05);
        let (ry, rx, rz) = e.transform.rotation.to_euler(EulerRot::YXZ);
        let mut euler = Vec3::new(rx.to_degrees(), ry.to_degrees(), rz.to_degrees());
        if vec3_edit(ui, "Rotation", &mut euler, 0.5) {
            e.transform.rotation =
                Quat::from_euler(EulerRot::YXZ, euler.y.to_radians(), euler.x.to_radians(), euler.z.to_radians());
        }
        vec3_edit(ui, "Scale", &mut e.transform.scale, 0.02);

        // ---- Mesh Renderer ----
        let mut remove_mesh = false;
        if let Some(m) = &mut e.mesh {
            ui.separator();
            ui.horizontal(|ui| {
                ui.strong("Mesh Renderer");
                if ui.small_button("X").clicked() {
                    remove_mesh = true;
                }
            });
            egui::ComboBox::from_label("Mesh").selected_text(m.kind.label()).show_ui(ui, |ui| {
                for k in MeshKind::ALL {
                    ui.selectable_value(&mut m.kind, k, k.label());
                }
            });
            ui.horizontal(|ui| {
                ui.label("Color");
                ui.color_edit_button_rgb(&mut m.color);
            });
        }
        if remove_mesh {
            e.mesh = None;
        }

        // ---- Light ----
        let mut remove_light = false;
        if let Some(l) = &mut e.light {
            ui.separator();
            ui.horizontal(|ui| {
                ui.strong("Directional Light");
                if ui.small_button("X").clicked() {
                    remove_light = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Color");
                ui.color_edit_button_rgb(&mut l.color);
            });
            drag_f32(ui, "Intensity", &mut l.intensity, 0.02);
        }
        if remove_light {
            e.light = None;
        }

        // ---- Spin ----
        let mut remove_spin = false;
        if let Some(s) = &mut e.spin {
            ui.separator();
            ui.horizontal(|ui| {
                ui.strong("Spin");
                if ui.small_button("X").clicked() {
                    remove_spin = true;
                }
            });
            vec3_edit(ui, "Deg/sec", &mut s.degrees_per_sec, 1.0);
        }
        if remove_spin {
            e.spin = None;
        }

        // ---- Bullet Emitter ----
        let mut remove_emitter = false;
        if let Some(em) = &mut e.emitter {
            ui.separator();
            ui.horizontal(|ui| {
                ui.strong("Bullet Emitter");
                if ui.small_button("X").clicked() {
                    remove_emitter = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Arms");
                ui.add(egui::DragValue::new(&mut em.arms).clamp_range(1..=64));
            });
            drag_f32(ui, "Speed", &mut em.speed, 0.05);
            drag_f32(ui, "Interval", &mut em.interval, 0.001);
            drag_f32(ui, "Spiral step", &mut em.step, 0.005);
            drag_f32(ui, "Bullet radius", &mut em.bullet_radius, 0.01);
            ui.horizontal(|ui| {
                ui.label("Color");
                ui.color_edit_button_rgb(&mut em.color);
            });
        }
        if remove_emitter {
            e.emitter = None;
        }

        // ---- Add Component ----
        ui.separator();
        ui.menu_button("Add Component", |ui| {
            if e.mesh.is_none() && ui.button("Mesh Renderer").clicked() {
                e.mesh = Some(MeshRenderer::default());
                ui.close_menu();
            }
            if e.light.is_none() && ui.button("Directional Light").clicked() {
                e.light = Some(Light::default());
                ui.close_menu();
            }
            if e.spin.is_none() && ui.button("Spin").clicked() {
                e.spin = Some(Spin::default());
                ui.close_menu();
            }
            if e.emitter.is_none() && ui.button("Bullet Emitter").clicked() {
                e.emitter = Some(BulletEmitter::default());
                ui.close_menu();
            }
        });
    }

    if let Some(p) = new_parent {
        scene.set_parent_keep_world(id, p);
    }
    action
}
