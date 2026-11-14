use std::f32::consts::TAU;
use std::path::{Path, PathBuf};

use engine::egui::{self, Color32};
use engine::prelude::*;

use crate::inspector::{inspector, Action};
use crate::project;
use crate::viewport::{self, Gizmo, GizmoMode, Ptr, ViewCam};

#[derive(Clone, Copy)]
struct Builtin {
    cube: MeshId,
    sphere: MeshId,
    plane: MeshId,
}

#[derive(Clone, Copy)]
enum Prim {
    Empty,
    Cube,
    Sphere,
    Plane,
    Light,
    Emitter,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BottomTab {
    Console,
    Assets,
}

pub struct Editor {
    scene: Scene,
    snapshot: Option<Scene>,
    playing: bool,
    selected: Option<EntityId>,
    meshes: Option<Builtin>,

    // камера и 3D-окно
    cam: FlyCamera,
    viewport_hovered: bool,
    cam_active: bool,
    prev_right: bool,
    gizmo: Gizmo,
    show_grid: bool,

    // файлы
    current_path: Option<PathBuf>,
    dirty: bool,
    autosave: bool,
    autosave_timer: f32,
    show_save_as: bool,
    save_name: String,

    // undo / redo (снимки сцены в JSON)
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
    last_committed: String,
    prev_busy: bool,

    // иерархия: перетаскивание
    drag_entity: Option<EntityId>,

    // нижняя панель
    bottom_tab: BottomTab,
    show_info: bool,
    show_warn: bool,
    show_err: bool,
    scene_files: Vec<PathBuf>,
    asset_files: Vec<PathBuf>,
    selected_file: Option<PathBuf>,
}

impl Editor {
    pub fn new() -> Self {
        project::ensure_dirs();

        let mut cam = FlyCamera::new(Vec3::new(0.0, 8.0, 20.0));
        cam.pitch = -0.3;

        let main_path = project::scene_path("main");
        let (scene, path) = if main_path.exists() {
            match project::load_scene(&main_path) {
                Ok(s) => {
                    log::info!("Загружена сцена {}", main_path.display());
                    (s, Some(main_path))
                }
                Err(e) => {
                    log::error!("Не удалось загрузить {}: {e}", main_path.display());
                    (demo_scene(), None)
                }
            }
        } else {
            (demo_scene(), None)
        };

        let mut ed = Self {
            scene,
            snapshot: None,
            playing: false,
            selected: None,
            meshes: None,
            cam,
            viewport_hovered: false,
            cam_active: false,
            prev_right: false,
            gizmo: Gizmo::new(),
            show_grid: true,
            current_path: path,
            dirty: false,
            autosave: true,
            autosave_timer: 0.0,
            show_save_as: false,
            save_name: "main".to_string(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_committed: String::new(),
            prev_busy: false,
            drag_entity: None,
            bottom_tab: BottomTab::Console,
            show_info: true,
            show_warn: true,
            show_err: true,
            scene_files: Vec::new(),
            asset_files: Vec::new(),
            selected_file: None,
        };
        ed.last_committed = ed.scene_json();
        ed.refresh_assets();
        log::info!("CupEngine Editor запущен");
        ed
    }

    // ------------------------------------------------------------------
    // Play / Stop
    // ------------------------------------------------------------------

    fn play(&mut self) {
        self.snapshot = Some(self.scene.clone());
        self.playing = true;
        log::info!("Play");
    }

    fn stop(&mut self) {
        if let Some(s) = self.snapshot.take() {
            self.scene = s;
        }
        self.playing = false;
        log::info!("Stop");
    }

    // ------------------------------------------------------------------
    // Undo / Redo
    // ------------------------------------------------------------------

    fn scene_json(&self) -> String {
        serde_json::to_string(&self.scene).unwrap_or_default()
    }

    fn restore_json(&mut self, json: &str) {
        match serde_json::from_str::<Scene>(json) {
            Ok(mut s) => {
                s.fix_ids();
                self.scene = s;
            }
            Err(e) => log::error!("Undo/Redo: не удалось восстановить сцену: {e}"),
        }
    }

    /// Если сцена изменилась с прошлой фиксации — кладём прошлое состояние в историю.
    fn check_commit(&mut self) {
        let now = self.scene_json();
        if now != self.last_committed {
            let prev = std::mem::replace(&mut self.last_committed, now);
            self.undo_stack.push(prev);
            if self.undo_stack.len() > 100 {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
            self.dirty = true;
        }
    }

    fn undo(&mut self) {
        if self.playing {
            return;
        }
        if let Some(prev) = self.undo_stack.pop() {
            let current = std::mem::replace(&mut self.last_committed, prev.clone());
            self.redo_stack.push(current);
            self.restore_json(&prev);
            self.dirty = true;
        }
    }

    fn redo(&mut self) {
        if self.playing {
            return;
        }
        if let Some(next) = self.redo_stack.pop() {
            let current = std::mem::replace(&mut self.last_committed, next.clone());
            self.undo_stack.push(current);
            self.restore_json(&next);
            self.dirty = true;
        }
    }

    fn reset_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_committed = self.scene_json();
    }

    // ------------------------------------------------------------------
    // Файлы
    // ------------------------------------------------------------------

    fn refresh_assets(&mut self) {
        self.scene_files = project::list_files(project::SCENES_DIR);
        self.asset_files = project::list_files(project::ASSETS_DIR);
    }

    fn save(&mut self) {
        match self.current_path.clone() {
            Some(p) => self.save_to(&p),
            None => self.show_save_as = true,
        }
    }

    fn save_to(&mut self, path: &Path) {
        // во время Play сохраняем исходную сцену, а не игровую копию
        let scene = self.snapshot.as_ref().unwrap_or(&self.scene);
        match project::save_scene(scene, path) {
            Ok(()) => {
                self.current_path = Some(path.to_path_buf());
                self.dirty = false;
                log::info!("Сцена сохранена: {}", path.display());
                self.refresh_assets();
            }
            Err(e) => log::error!("Ошибка сохранения {}: {e}", path.display()),
        }
    }

    fn open_scene(&mut self, path: &Path) {
        if self.playing {
            self.stop();
        }
        match project::load_scene(path) {
            Ok(s) => {
                self.scene = s;
                self.selected = None;
                self.current_path = Some(path.to_path_buf());
                self.dirty = false;
                self.reset_history();
                log::info!("Открыта сцена: {}", path.display());
            }
            Err(e) => log::error!("Не удалось открыть {}: {e}", path.display()),
        }
    }

    fn new_scene(&mut self) {
        if self.playing {
            self.stop();
        }
        let mut s = Scene::new();
        let id = s.spawn("Directional Light");
        if let Some(e) = s.get_mut(id) {
            e.light = Some(Light::default());
            e.transform.translation = Vec3::new(0.0, 10.0, 0.0);
            e.transform.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::new(-0.4, -1.0, -0.3).normalize());
        }
        self.scene = s;
        self.selected = None;
        self.current_path = None;
        self.dirty = false;
        self.reset_history();
        log::info!("Новая сцена");
    }

    // ------------------------------------------------------------------
    // Добавление объектов
    // ------------------------------------------------------------------

    fn add(&mut self, p: Prim) {
        let pos = self.cam.position + self.cam.forward() * 6.0;
        let name = match p {
            Prim::Empty => "Empty",
            Prim::Cube => "Cube",
            Prim::Sphere => "Sphere",
            Prim::Plane => "Plane",
            Prim::Light => "Directional Light",
            Prim::Emitter => "Bullet Emitter",
        };
        let id = self.scene.spawn(name);
        if let Some(e) = self.scene.get_mut(id) {
            e.transform.translation = pos;
            match p {
                Prim::Empty => {}
                Prim::Cube => e.mesh = Some(MeshRenderer { kind: MeshKind::Cube, ..Default::default() }),
                Prim::Sphere => e.mesh = Some(MeshRenderer { kind: MeshKind::Sphere, ..Default::default() }),
                Prim::Plane => {
                    e.mesh = Some(MeshRenderer { kind: MeshKind::Plane, ..Default::default() });
                    e.transform.scale = Vec3::splat(5.0);
                }
                Prim::Light => {
                    e.light = Some(Light::default());
                    e.transform.rotation =
                        Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::new(-0.4, -1.0, -0.3).normalize());
                }
                Prim::Emitter => e.emitter = Some(BulletEmitter::default()),
            }
        }
        self.selected = Some(id);
    }

    // ------------------------------------------------------------------
    // Горячие клавиши
    // ------------------------------------------------------------------

    fn hotkeys(&mut self, egui: &egui::Context) {
        if self.cam_active || self.show_save_as || egui.wants_keyboard_input() {
            return;
        }
        use egui::Key;
        let (cmd, shift, z, y, d, s, del, w, e, r, f) = egui.input(|i| {
            (
                i.modifiers.command,
                i.modifiers.shift,
                i.key_pressed(Key::Z),
                i.key_pressed(Key::Y),
                i.key_pressed(Key::D),
                i.key_pressed(Key::S),
                i.key_pressed(Key::Delete),
                i.key_pressed(Key::W),
                i.key_pressed(Key::E),
                i.key_pressed(Key::R),
                i.key_pressed(Key::F),
            )
        });

        if cmd && z && !shift {
            self.undo();
        } else if cmd && (y || (z && shift)) {
            self.redo();
        } else if cmd && s {
            self.save();
        } else if cmd && d {
            if let Some(id) = self.selected {
                self.selected = self.scene.duplicate(id);
                self.check_commit();
            }
        } else if del {
            if let Some(id) = self.selected {
                self.scene.remove(id);
                self.selected = None;
                self.check_commit();
            }
        } else if !cmd {
            if w {
                self.gizmo.mode = GizmoMode::Translate;
            } else if e {
                self.gizmo.mode = GizmoMode::Rotate;
            } else if r {
                self.gizmo.mode = GizmoMode::Scale;
            } else if f {
                if let Some(id) = self.selected {
                    let p = self.scene.world_matrix(id).w_axis.truncate();
                    self.cam.position = p - self.cam.forward() * 8.0;
                }
            }
        }
    }
}

impl Game for Editor {
    fn init(&mut self, ctx: &mut Context) {
        self.meshes = Some(Builtin {
            cube: ctx.renderer.create_mesh(&MeshData::cube()),
            sphere: ctx.renderer.create_mesh(&MeshData::sphere(24, 48)),
            plane: ctx.renderer.create_mesh(&MeshData::plane()),
        });
    }

    fn fixed_update(&mut self, ctx: &mut Context) {
        if self.playing {
            self.scene.step(ctx.time.fixed_dt);
        }
    }

    fn ui(&mut self, egui: &egui::Context, ctx: &mut Context) {
        self.hotkeys(egui);

        // ================= верхняя панель =================
        egui::TopBottomPanel::top("toolbar").show(egui, |ui| {
            ui.horizontal(|ui| {
                if !self.playing {
                    if ui.button("Play").clicked() {
                        self.play();
                    }
                } else if ui.button("Stop").clicked() {
                    self.stop();
                }

                ui.menu_button("File", |ui| {
                    if ui.button("New Scene").clicked() {
                        self.new_scene();
                        ui.close_menu();
                    }
                    if ui.button("Save (Ctrl+S)").clicked() {
                        self.save();
                        ui.close_menu();
                    }
                    if ui.button("Save As...").clicked() {
                        self.show_save_as = true;
                        ui.close_menu();
                    }
                    ui.checkbox(&mut self.autosave, "Autosave (30 c)");
                });

                ui.menu_button("Edit", |ui| {
                    let can_undo = !self.undo_stack.is_empty() && !self.playing;
                    let can_redo = !self.redo_stack.is_empty() && !self.playing;
                    if ui.add_enabled(can_undo, egui::Button::new("Undo (Ctrl+Z)")).clicked() {
                        self.undo();
                        ui.close_menu();
                    }
                    if ui.add_enabled(can_redo, egui::Button::new("Redo (Ctrl+Y)")).clicked() {
                        self.redo();
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.selected.is_some(), egui::Button::new("Duplicate (Ctrl+D)")).clicked() {
                        if let Some(id) = self.selected {
                            self.selected = self.scene.duplicate(id);
                        }
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.selected.is_some(), egui::Button::new("Delete (Del)")).clicked() {
                        if let Some(id) = self.selected {
                            self.scene.remove(id);
                            self.selected = None;
                        }
                        ui.close_menu();
                    }
                });

                ui.menu_button("Add", |ui| {
                    let items = [
                        ("Empty", Prim::Empty),
                        ("Cube", Prim::Cube),
                        ("Sphere", Prim::Sphere),
                        ("Plane", Prim::Plane),
                        ("Directional Light", Prim::Light),
                        ("Bullet Emitter", Prim::Emitter),
                    ];
                    for (label, prim) in items {
                        if ui.button(label).clicked() {
                            self.add(prim);
                            ui.close_menu();
                        }
                    }
                });

                ui.separator();
                ui.selectable_value(&mut self.gizmo.mode, GizmoMode::Translate, "Move (W)");
                ui.selectable_value(&mut self.gizmo.mode, GizmoMode::Rotate, "Rotate (E)");
                ui.selectable_value(&mut self.gizmo.mode, GizmoMode::Scale, "Scale (R)");
                ui.separator();
                ui.checkbox(&mut self.show_grid, "Grid");
                ui.separator();

                let name = self
                    .current_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("(не сохранено)");
                ui.label(format!("{}{}", name, if self.dirty { " *" } else { "" }));
                if self.playing {
                    ui.colored_label(Color32::LIGHT_GREEN, "Playing");
                }
                ui.label(format!("{:.0} FPS", ctx.time.fps));
            });
        });

        // ================= нижняя панель: Console / Assets =================
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .default_height(170.0)
            .show(egui, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.bottom_tab, BottomTab::Console, "Console");
                    ui.selectable_value(&mut self.bottom_tab, BottomTab::Assets, "Assets");
                });
                ui.separator();

                match self.bottom_tab {
                    BottomTab::Console => {
                        ui.horizontal(|ui| {
                            if ui.button("Clear").clicked() {
                                engine::console::clear();
                            }
                            ui.checkbox(&mut self.show_info, "Info");
                            ui.checkbox(&mut self.show_warn, "Warn");
                            ui.checkbox(&mut self.show_err, "Error");
                        });
                        let lines = engine::console::snapshot();
                        egui::ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for l in &lines {
                                    let (show, color) = match l.level {
                                        log::Level::Error => (self.show_err, Color32::from_rgb(240, 90, 90)),
                                        log::Level::Warn => (self.show_warn, Color32::from_rgb(240, 200, 80)),
                                        _ => (self.show_info, Color32::from_gray(190)),
                                    };
                                    if show {
                                        ui.label(
                                            egui::RichText::new(format!("[{}] {}", l.level, l.message))
                                                .color(color)
                                                .monospace(),
                                        );
                                    }
                                }
                            });
                    }
                    BottomTab::Assets => {
                        ui.horizontal(|ui| {
                            if ui.button("Refresh").clicked() {
                                self.refresh_assets();
                            }
                            ui.label("Двойной клик по сцене (.scene.json) — открыть");
                        });
                        let mut to_open: Option<PathBuf> = None;
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            ui.strong(format!("{}/", project::SCENES_DIR));
                            for p in &self.scene_files {
                                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
                                let sel = self.selected_file.as_ref() == Some(p);
                                let resp = ui.selectable_label(sel, format!("   {name}"));
                                if resp.clicked() {
                                    self.selected_file = Some(p.clone());
                                }
                                if resp.double_clicked() && project::is_scene_file(p) {
                                    to_open = Some(p.clone());
                                }
                            }
                            ui.add_space(6.0);
                            ui.strong(format!("{}/", project::ASSETS_DIR));
                            if self.asset_files.is_empty() {
                                ui.weak("   (пусто — сюда позже попадут модели и текстуры)");
                            }
                            for p in &self.asset_files {
                                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
                                ui.label(format!("   {name}"));
                            }
                        });
                        if let Some(p) = to_open {
                            self.open_scene(&p);
                        }
                    }
                }
            });

        // ================= иерархия =================
        egui::SidePanel::left("hierarchy").default_width(240.0).show(egui, |ui| {
            ui.heading("Hierarchy");
            ui.label("Перетащи объект на другой, чтобы сделать его дочерним");
            ui.separator();

            let order = self.scene.hierarchy_order();
            let panel_rect = ui.max_rect();
            let mut rows: Vec<(EntityId, egui::Rect)> = Vec::new();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (id, depth) in &order {
                    let (id, depth) = (*id, *depth);
                    let name = self.scene.get(id).map(|e| e.name.clone()).unwrap_or_default();
                    let selected = self.selected == Some(id);
                    let row = ui.horizontal(|ui| {
                        ui.add_space(depth as f32 * 16.0);
                        let text = if selected { egui::RichText::new(name).strong() } else { egui::RichText::new(name) };
                        let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click_and_drag()));
                        if selected {
                            ui.painter().rect_filled(
                                resp.rect.expand(2.0),
                                3.0,
                                Color32::from_rgba_unmultiplied(255, 160, 30, 40),
                            );
                        }
                        if resp.clicked() {
                            self.selected = Some(id);
                        }
                        if resp.drag_started() {
                            self.drag_entity = Some(id);
                            self.selected = Some(id);
                        }
                    });
                    let r = row.response.rect;
                    rows.push((
                        id,
                        egui::Rect::from_min_max(
                            egui::pos2(panel_rect.min.x, r.min.y),
                            egui::pos2(panel_rect.max.x, r.max.y),
                        ),
                    ));
                }
            });

            // обработка drag & drop
            if let Some(dragged) = self.drag_entity {
                egui.set_cursor_icon(egui::CursorIcon::Grabbing);
                let pointer = egui.input(|i| i.pointer.hover_pos());
                let target = pointer.and_then(|p| rows.iter().find(|(_, r)| r.contains(p)).map(|(id, r)| (*id, *r)));
                if let Some((tid, r)) = target {
                    if tid != dragged {
                        ui.painter().rect_stroke(r, 3.0, egui::Stroke::new(1.5, Color32::YELLOW));
                    }
                }
                if egui.input(|i| i.pointer.any_released()) {
                    if let Some(p) = pointer {
                        if panel_rect.contains(p) {
                            match target {
                                Some((tid, _)) if tid != dragged => {
                                    if !self.scene.is_descendant(tid, dragged) {
                                        self.scene.set_parent_keep_world(dragged, Some(tid));
                                    }
                                }
                                None => self.scene.set_parent_keep_world(dragged, None),
                                _ => {}
                            }
                        }
                    }
                    self.drag_entity = None;
                }
            }
        });

        // ================= инспектор =================
        egui::SidePanel::right("inspector").default_width(300.0).show(egui, |ui| {
            ui.heading("Inspector");
            ui.separator();
            match self.selected {
                None => {
                    ui.label("Выбери объект: кликни по нему в 3D-окне или в Hierarchy.");
                }
                Some(sel) => {
                    if self.scene.get(sel).is_none() {
                        self.selected = None;
                    } else {
                        let action = egui::ScrollArea::vertical()
                            .show(ui, |ui| inspector(ui, &mut self.scene, sel))
                            .inner;
                        match action {
                            Action::Delete => {
                                self.scene.remove(sel);
                                self.selected = None;
                            }
                            Action::Duplicate => {
                                self.selected = self.scene.duplicate(sel);
                            }
                            Action::None => {}
                        }
                    }
                }
            }
        });

        // ================= 3D-окно: область, выбор кликом, гизмо =================
        let rect = egui.available_rect();
        let ppp = egui.pixels_per_point();
        ctx.renderer.set_viewport(rect.min.x * ppp, rect.min.y * ppp, rect.width() * ppp, rect.height() * ppp);

        let (pos, primary_pressed, primary_down, right_down) = egui.input(|i| {
            (i.pointer.hover_pos(), i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.secondary_down())
        });
        let in_viewport = pos.map_or(false, |p| rect.contains(p)) && !self.show_save_as;
        self.viewport_hovered = in_viewport;

        if rect.width() > 20.0 && rect.height() > 20.0 {
            let vc = ViewCam::new(&self.cam.camera(), rect);
            let painter = egui
                .layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("viewport_overlay")))
                .with_clip_rect(rect);

            let mut gizmo_consumed = false;
            if let Some(sel) = self.selected.filter(|id| self.scene.get(*id).is_some()) {
                let ptr = Ptr { pos, pressed: primary_pressed && in_viewport && !right_down, down: primary_down };
                gizmo_consumed = self.gizmo.update(&vc, &mut self.scene, sel, &ptr);
                self.gizmo.draw(&painter, &vc, &self.scene, sel);
            }

            // клик по объекту
            if primary_pressed && in_viewport && !gizmo_consumed && !right_down && !self.cam_active {
                if let Some(p) = pos {
                    let (o, d) = vc.ray(p);
                    self.selected = viewport::pick(&self.scene, o, d);
                }
            }

            painter.text(
                egui::pos2(rect.min.x + 8.0, rect.min.y + 6.0),
                egui::Align2::LEFT_TOP,
                "ЛКМ — выбрать | ПКМ+WASD/QE — лететь | W/E/R — гизмо | F — фокус | Ctrl+Z/Y — undo/redo",
                egui::FontId::proportional(13.0),
                Color32::from_white_alpha(150),
            );
        }

        // ================= окно "Save As" =================
        if self.show_save_as {
            let mut open = true;
            let mut do_save = false;
            egui::Window::new("Save Scene As").collapsible(false).resizable(false).open(&mut open).show(egui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Имя:");
                    ui.text_edit_singleline(&mut self.save_name);
                });
                ui.label(format!("-> {}", project::scene_path(&self.save_name).display()));
                if ui.button("Save").clicked() {
                    do_save = true;
                }
            });
            if do_save {
                let p = project::scene_path(&self.save_name);
                self.save_to(&p);
                self.show_save_as = false;
            } else if !open {
                self.show_save_as = false;
            }
        }

        // ================= автосохранение =================
        self.autosave_timer += ctx.time.dt;
        if self.autosave_timer >= 30.0 {
            self.autosave_timer = 0.0;
            if self.autosave && self.dirty && !self.playing {
                let path = Path::new(project::SCENES_DIR).join("_autosave.scene.json");
                match project::save_scene(&self.scene, &path) {
                    Ok(()) => {
                        log::info!("Автосохранение: {}", path.display());
                        self.refresh_assets();
                    }
                    Err(e) => log::warn!("Автосохранение не удалось: {e}"),
                }
            }
        }

        // ================= фиксация истории (undo) =================
        // Изменение считается законченным, когда отпустили мышь / ушёл фокус с поля ввода.
        let busy = egui.input(|i| i.pointer.any_down()) || egui.wants_keyboard_input();
        if self.prev_busy && !busy && !self.playing {
            self.check_commit();
        }
        self.prev_busy = busy;
    }

    fn frame(&mut self, ctx: &mut Context) {
        let Some(m) = self.meshes else { return };

        // камера: управляется, только если ПКМ нажали над 3D-окном
        let right = ctx.input.mouse_down(MouseButton::Right);
        if right && !self.prev_right && self.viewport_hovered {
            self.cam_active = true;
        }
        if !right {
            self.cam_active = false;
        }
        self.prev_right = right;
        if self.cam_active {
            self.cam.update(ctx.input, ctx.time.dt);
        }
        ctx.renderer.set_camera(self.cam.camera());

        // свет: первый объект с компонентом Light
        let light = self.scene.entities().iter().find(|e| e.light.is_some());
        match light {
            Some(e) => {
                let l = e.light.as_ref().unwrap();
                let dir = self.scene.world_matrix(e.id).transform_vector3(Vec3::NEG_Z).normalize_or_zero();
                ctx.renderer.set_light(DirectionalLight {
                    direction: dir,
                    color: Vec3::from(l.color) * l.intensity,
                    ambient: Vec3::new(0.12, 0.13, 0.18),
                });
            }
            None => ctx.renderer.set_light(DirectionalLight::default()),
        }

        // меши
        for e in self.scene.entities() {
            if let Some(mr) = &e.mesh {
                let mesh = match mr.kind {
                    MeshKind::Cube => m.cube,
                    MeshKind::Sphere => m.sphere,
                    MeshKind::Plane => m.plane,
                };
                let c = Vec4::new(mr.color[0], mr.color[1], mr.color[2], 1.0);
                ctx.renderer.draw_mesh(mesh, self.scene.world_matrix(e.id), c);
            }
        }

        // пули
        let b = &self.scene.bullets;
        for i in 0..b.len() {
            let c = b.color[i];
            ctx.renderer.draw_billboard(b.pos[i], c[3], Vec4::new(c[0], c[1], c[2], 1.0));
        }

        // сетка, иконки света/эмиттеров, контур выбранного
        if self.show_grid {
            viewport::draw_grid(ctx.renderer, 20);
        }
        viewport::draw_scene_overlays(ctx.renderer, &self.scene, self.selected);

        let name = self
            .current_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("untitled");
        ctx.window.set_title(&format!(
            "CupEngine Editor - {}{} | objects: {} | bullets: {}",
            name,
            if self.dirty { "*" } else { "" },
            self.scene.entities().len(),
            self.scene.bullets.len(),
        ));
    }
}

fn demo_scene() -> Scene {
    let mut s = Scene::new();

    let light = s.spawn("Directional Light");
    if let Some(e) = s.get_mut(light) {
        e.light = Some(Light::default());
        e.transform.translation = Vec3::new(0.0, 10.0, 0.0);
        e.transform.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::new(-0.4, -1.0, -0.3).normalize());
    }

    let floor = s.spawn("Floor");
    if let Some(e) = s.get_mut(floor) {
        e.mesh = Some(MeshRenderer { kind: MeshKind::Plane, color: [0.22, 0.24, 0.28] });
        e.transform.scale = Vec3::new(20.0, 1.0, 20.0);
    }

    let core = s.spawn("Core");
    if let Some(e) = s.get_mut(core) {
        e.mesh = Some(MeshRenderer { kind: MeshKind::Sphere, color: [0.9, 0.9, 0.95] });
        e.transform.translation = Vec3::new(0.0, 1.5, 0.0);
        e.transform.scale = Vec3::splat(2.0);
        e.emitter = Some(BulletEmitter::default());
    }

    let ring = s.spawn("Cube Ring");
    if let Some(e) = s.get_mut(ring) {
        e.spin = Some(Spin { degrees_per_sec: Vec3::new(0.0, 20.0, 0.0) });
    }

    for i in 0..12 {
        let a = i as f32 / 12.0 * TAU;
        let id = s.spawn(&format!("Cube {i}"));
        if let Some(e) = s.get_mut(id) {
            e.parent = Some(ring);
            e.transform.translation = Vec3::new(a.cos() * 9.0, 0.75, a.sin() * 9.0);
            e.transform.scale = Vec3::splat(1.5);
            e.mesh = Some(MeshRenderer {
                kind: MeshKind::Cube,
                color: [
                    0.5 + 0.5 * a.cos(),
                    0.5 + 0.5 * (a + 2.1).cos(),
                    0.5 + 0.5 * (a + 4.2).cos(),
                ],
            });
            e.spin = Some(Spin { degrees_per_sec: Vec3::new(0.0, 60.0, 0.0) });
        }
    }
    s
}
