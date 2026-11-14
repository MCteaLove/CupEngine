//! Работа с файлами проекта: сцены (JSON) и список ассетов.

use std::path::{Path, PathBuf};

use engine::prelude::*;

pub const SCENES_DIR: &str = "scenes";
pub const ASSETS_DIR: &str = "assets";

pub fn ensure_dirs() {
    let _ = std::fs::create_dir_all(SCENES_DIR);
    let _ = std::fs::create_dir_all(ASSETS_DIR);
}

/// Имя -> путь `scenes/<имя>.scene.json` (небезопасные символы заменяются).
pub fn scene_path(name: &str) -> PathBuf {
    let mut clean: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' { c } else { '_' })
        .collect();
    if let Some(stripped) = clean.strip_suffix(".scene.json") {
        clean = stripped.to_string();
    }
    if clean.is_empty() {
        clean = "untitled".to_string();
    }
    Path::new(SCENES_DIR).join(format!("{clean}.scene.json"))
}

/// Файлы в папке (отсортированы по имени), без подпапок.
pub fn list_files(dir: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_file()).collect(),
        Err(_) => Vec::new(),
    };
    v.sort();
    v
}

pub fn save_scene(scene: &Scene, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(scene).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub fn load_scene(path: &Path) -> Result<Scene, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut scene: Scene = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    scene.fix_ids();
    Ok(scene)
}

pub fn is_scene_file(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.ends_with(".scene.json"))
}
